use crate::{fs::*, hosts, models::*};
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct App {
    pub root: PathBuf,
    pub home: PathBuf,
    pub state: State,
    pub preferences: Preferences,
    pub(crate) environment: bool,
}
impl App {
    /// Tests and automation must provide an isolated home. No process HOME mutation.
    pub fn open(root: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Result<Self> {
        Self::open_inner(root.into(), home.into(), false)
    }
    pub fn system() -> Result<Self> {
        let home = dirs::home_dir().context("无法确定用户目录")?;
        Self::open_inner(home.join(".chu"), home, true)
    }
    fn open_inner(root: PathBuf, home: PathBuf, environment: bool) -> Result<Self> {
        let root = std::path::absolute(root)?;
        let home = std::path::absolute(home)?;
        let raw = match fs::read(root.join("state.json")) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        let state = if raw.is_empty() {
            State {
                version: 1,
                ..State::default()
            }
        } else {
            serde_json::from_slice(&raw).context("中央状态损坏，未覆盖原文件")?
        };
        let preferences = match fs::read(root.join("preferences.json")) {
            Ok(data) => serde_json::from_slice(&data).context("界面偏好损坏")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
            Err(e) => return Err(e.into()),
        };
        let mut app = Self {
            root,
            home,
            state,
            preferences,
            environment,
        };
        if !raw.is_empty() {
            app.migrate_sources(&raw)?;
        }
        Ok(app)
    }
    pub fn save(&mut self) -> Result<()> {
        self.state.version = 1;
        write_json(&self.root.join("state.json"), &self.state)
    }
    pub fn read_lock(&self) -> Result<LockFile> {
        match fs::read(self.root.join("chu-lock.json")) {
            Ok(data) => serde_json::from_slice(&data).context("锁文件损坏，未覆盖原文件"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LockFile {
                version: 1,
                ..LockFile::default()
            }),
            Err(e) => Err(e.into()),
        }
    }
    pub fn write_lock(&self, lock: &LockFile) -> Result<()> {
        let mut lock = lock.clone();
        lock.version = 1;
        write_json(&self.root.join("chu-lock.json"), &lock)
    }
    pub fn hosts(&self) -> Vec<Host> {
        hosts::hosts(&self.home, &self.state, self.environment)
    }
    pub fn host(&self, id: &str) -> Result<Host> {
        self.hosts()
            .into_iter()
            .find(|h| h.id == id)
            .context("未知的 Agent 宿主")
    }
    pub fn skill(&self, id: &str) -> Result<StoredSkill> {
        self.state
            .skills
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .context("未知的 skill")
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let lock = self.reconcile_prompts()?;
        let hosts = self.hosts();
        let mut skills: Vec<_> = self
            .state
            .skills
            .iter()
            .map(|s| {
                let (enabled_on, mode_by_host) = hosts::deployment_views(&s.deployments);
                let source = lock.skills.get(&s.name).or_else(|| lock.skills.get(&s.id));
                SkillView {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    description: if s.description.is_empty() {
                        crate::sources::description(Path::new(&s.source_dir))
                    } else {
                        s.description.clone()
                    },
                    tracked: source.is_some(),
                    repository: source.map(crate::sources::repository).unwrap_or_default(),
                    source: s.source_dir.clone(),
                    managed: true,
                    enabled_on,
                    mode_by_host,
                }
            })
            .collect();
        skills.extend(self.discovered_skills(&hosts));
        skills.sort_by(|a, b| a.name.cmp(&b.name));
        let mut mcps: Vec<_> = self
            .state
            .mcps
            .iter()
            .map(|s| McpView {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                r#type: s.r#type.clone(),
                endpoint: s.endpoint.clone(),
                command: s.command.clone(),
                has_credentials: !s.secret.is_empty() || !s.env.is_empty() || !s.headers.is_empty(),
                managed: true,
                enabled_on: hosts::deployment_views(&s.deployments).0,
            })
            .collect();
        mcps.extend(self.discovered_mcps(&hosts));
        mcps.sort_by(|a, b| a.name.cmp(&b.name));
        let agents = self
            .state
            .agents
            .iter()
            .map(|s| AgentView {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                model: s.model.clone(),
                source: s.source.clone(),
                managed: true,
                enabled_on: hosts::deployment_views(&s.deployments).0,
            })
            .collect();
        Ok(Snapshot {
            root: self.root.to_string_lossy().into(),
            projects: self.project_views(&hosts),
            hosts,
            skills,
            mcps,
            agents,
            prompts: self.prompt_views(&lock),
            references: self.reference_views(),
            providers: self.state.providers.clone(),
            last_scan: now(),
        })
    }
    pub fn update_host_paths(
        &mut self,
        id: &str,
        config: &str,
        skills: &str,
        agents: &str,
    ) -> Result<()> {
        self.host(id)?;
        if [config, skills, agents].iter().any(|p| p.trim().is_empty()) {
            bail!("宿主路径不能为空");
        }
        let expand = |s: &str| -> String {
            if s == "~" {
                self.home.to_string_lossy().into()
            } else if let Some(p) = s.strip_prefix("~/") {
                self.home.join(p).to_string_lossy().into()
            } else {
                s.into()
            }
        };
        self.state.paths.insert(
            id.into(),
            HostPaths {
                config: expand(config),
                skills: expand(skills),
                agents: expand(agents),
            },
        );
        self.save()
    }
    pub fn set_host_visible(&mut self, id: &str, visible: bool) -> Result<()> {
        self.host(id)?;
        self.preferences.hidden_hosts.retain(|h| h != id);
        if !visible {
            self.preferences.hidden_hosts.push(id.into());
        }
        write_json(&self.root.join("preferences.json"), &self.preferences)
    }
    /// Shared native/UI/headless boundary, with the same operations as the former App binding.
    pub fn execute(&mut self, method: &str, args: Value) -> Result<Value> {
        let old = self.state.clone();
        let result = self.dispatch(method, args);
        if result.is_err() {
            self.state = fs::read(self.root.join("state.json"))
                .ok()
                .and_then(|d| serde_json::from_slice(&d).ok())
                .unwrap_or(old);
        }
        result
    }
    fn dispatch(&mut self, method: &str, args: Value) -> Result<Value> {
        let string = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("");
        let boolean = |i: usize| args.get(i).and_then(Value::as_bool).unwrap_or(false);
        match method {
            "GetSnapshot" | "Refresh" => {}
            "ListImportCandidates" => {
                return Ok(serde_json::to_value(
                    self.list_import_candidates(string(0))?,
                )?);
            }
            "ImportHostResources" => self.import_host_resources(
                string(0),
                serde_json::from_value(args.get(1).cloned().unwrap_or_default())?,
                serde_json::from_value(args.get(2).cloned().unwrap_or_default())?,
            )?,
            "PreviewSkills" => return Ok(serde_json::to_value(self.preview_skills(string(0))?)?),
            "InstallSkills" => self.install_skills(
                string(0),
                serde_json::from_value(args.get(1).cloned().unwrap_or_default())?,
            )?,
            "CheckSkillUpdates" => return Ok(serde_json::to_value(self.check_skill_updates()?)?),
            "UpdateSkill" => self.update_skill(string(0))?,
            "RemoveSkill" | "DeleteSkill" => self.delete_skill(string(0))?,
            "ImportSkill" => self.import_skill(string(0), string(1))?,
            "ToggleSkill" => self.toggle_skill(string(0), string(1), boolean(2))?,
            "AddMCP" => self.add_mcp(serde_json::from_value(
                args.get(0).cloned().unwrap_or_default(),
            )?)?,
            "ToggleMCP" => self.toggle_mcp(string(0), string(1), boolean(2))?,
            "TestMCP" => return Ok(Value::String(self.test_mcp(string(0))?)),
            "AddAgent" => self.add_agent(serde_json::from_value(
                args.get(0).cloned().unwrap_or_default(),
            )?)?,
            "ToggleAgent" => self.toggle_agent(string(0), string(1), boolean(2))?,
            "RestoreBackup" => self.restore_backup(string(0))?,
            "UpdateHostPaths" => {
                self.update_host_paths(string(0), string(1), string(2), string(3))?
            }
            "SetHostVisible" => self.set_host_visible(string(0), boolean(1))?,
            "CreatePrompt" => self.create_prompt(string(0), string(1))?,
            "ReadSkill" => {
                return Ok(Value::String(self.read_skill_content(string(0))?));
            }
            "ReadAgent" => {
                let agent = self
                    .state
                    .agents
                    .iter()
                    .find(|a| a.id == string(0))
                    .context("未知的 Agent")?;
                return Ok(Value::String(fs::read_to_string(
                    self.root.join("agents").join(format!("{}.md", agent.id)),
                )?));
            }
            "ReadPrompt" => return Ok(Value::String(self.read_prompt(string(0))?)),
            "UpdatePrompt" => self.update_prompt(string(0), string(1), string(2))?,
            "DeletePrompt" => self.delete_prompt(string(0))?,
            "TogglePrompt" => self.toggle_prompt(string(0), string(1), boolean(2))?,
            "CreateReference" => self.create_reference(string(0), string(1))?,
            "ReadReference" => return Ok(Value::String(self.read_reference(string(0))?)),
            "UpdateReference" => self.update_reference(string(0), string(1), string(2))?,
            "DeleteReference" => self.delete_reference(string(0))?,
            "CreateProvider" => self.save_provider(
                "",
                serde_json::from_value(args.get(0).cloned().unwrap_or_default())?,
            )?,
            "UpdateProvider" => {
                if string(0).is_empty() {
                    bail!("未知的供应商");
                }
                self.save_provider(
                    string(0),
                    serde_json::from_value(args.get(1).cloned().unwrap_or_default())?,
                )?;
            }
            "DeleteProvider" => self.delete_provider(string(0))?,
            "RefreshProviderModels" => self.refresh_provider_models(string(0))?,
            "AddProject" => self.add_project(string(0))?,
            "RenameProject" => self.rename_project(string(0), string(1))?,
            "RelocateProject" => self.relocate_project(string(0), string(1))?,
            "ToggleProjectSkill" => {
                self.toggle_project_skill(string(0), string(1), string(2), boolean(3))?
            }
            "ResetProjectSkill" => self.reset_project_skill(string(0), string(1), string(2))?,
            "DeleteProject" => {
                let retained = self.delete_project(string(0), boolean(1))?;
                return Ok(serde_json::json!({"snapshot": self.snapshot()?, "retained": retained}));
            }
            _ => bail!("未知操作: {method}"),
        }
        Ok(serde_json::to_value(self.snapshot()?)?)
    }
}
