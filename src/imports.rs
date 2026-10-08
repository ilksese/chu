//! Batch import and immediate apply. Preflight all conflicts, then commit one journalled unit.
use crate::{
    App,
    fs::*,
    import_scan::{Entry, Resource},
    import_transaction::Journal,
    mcp::{PathBufExt, read_config, root_key, write_config},
    mcp_native,
    models::*,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
struct SkillPlan {
    item: StoredSkill,
    source: PathBuf,
    hash: String,
}
struct ConfigPlan {
    path: PathBuf,
    format: String,
    root: Value,
    original: Value,
    hash: String,
}
impl App {
    pub fn import_host_resources(
        &mut self,
        kind: &str,
        ids: Vec<String>,
        host_ids: Vec<String>,
    ) -> Result<()> {
        if ids.is_empty() {
            bail!("请至少勾选一项需要导入的资源");
        }
        let (_, mut scan) = self.scan_imports(kind)?;
        let mut selected = Vec::new();
        let mut seen = BTreeSet::new();
        for id in ids.into_iter().collect::<BTreeSet<_>>() {
            let index = scan
                .iter()
                .position(|e| e.view.id == id)
                .context("候选资源已变化，请重新扫描后再导入")?;
            let entry = scan.remove(index);
            if !entry.view.available {
                bail!("{}：{}", entry.view.name, entry.view.reason);
            }
            if !seen.insert(entry.view.name.clone()) {
                bail!("存在同名资源“{}”，请仅选择一个来源", entry.view.name);
            }
            selected.push(entry);
        }
        let hosts: Vec<Host> = host_ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .iter()
            .map(|h| self.host(h))
            .collect::<Result<_>>()?;
        let old = self.state.clone();
        let mut journal = Journal::new();
        let result = self.commit_imports(selected, &hosts, &mut journal);
        if let Err(error) = result {
            self.state = old;
            journal.rollback()?;
            return Err(error);
        }
        journal.commit();
        Ok(())
    }
    fn commit_imports(
        &mut self,
        entries: Vec<Entry>,
        hosts: &[Host],
        journal: &mut Journal,
    ) -> Result<()> {
        let mut state = self.state.clone();
        let mut skills = Vec::new();
        let mut mcps = Vec::new();
        let mut ids = BTreeSet::new();
        for entry in entries {
            match entry.resource.context("此资源暂不支持导入")? {
                Resource::Skill { source, hash } => {
                    let id = identifier(&entry.view.name, "skill");
                    if !ids.insert(id.clone()) || state.skills.iter().any(|s| s.id == id) {
                        bail!("Skill 标识冲突，请更改来源名称后重试");
                    }
                    let central = self.root.join("skills").join(&id);
                    if exists(&central) {
                        bail!("中央 Skill 目录已存在: {}", central.display());
                    }
                    let item = StoredSkill {
                        id,
                        name: entry.view.name,
                        description: entry.view.description,
                        source_dir: central.to_string_lossy().into(),
                        deployments: Map::new(),
                    };
                    for host in hosts {
                        validate_skill_target(
                            &Path::new(&host.skill_path).join(&item.name),
                            &hash,
                        )?;
                    }
                    skills.push(SkillPlan { item, source, hash });
                }
                Resource::Mcp(item) => {
                    if !ids.insert(item.id.clone()) || state.mcps.iter().any(|m| m.id == item.id) {
                        bail!("MCP 标识冲突，请更改来源名称后重试");
                    }
                    mcps.push(*item);
                }
            }
        }
        let mut configs = BTreeMap::<PathBuf, ConfigPlan>::new();
        for host in hosts {
            if !mcps.is_empty() {
                let path = PathBuf::from(&host.config_path);
                let hash = hash_path(&path);
                if self
                    .state
                    .mcps
                    .iter()
                    .filter_map(|m| m.deployments.get(&host.id))
                    .any(|d| d.enabled && !d.last_hash.is_empty() && d.last_hash != hash)
                {
                    bail!("{} 配置在 Chu 外发生变化，已阻止写入", host.name);
                }
                if !configs.contains_key(&path) {
                    let root = read_config(&path, &host.format).context("无法读取目标宿主配置")?;
                    configs.insert(
                        path.clone(),
                        ConfigPlan {
                            path: path.clone(),
                            format: host.format.clone(),
                            original: root.clone(),
                            root,
                            hash,
                        },
                    );
                }
                let plan = configs.get_mut(&path).unwrap();
                let key = root_key(&host.id, &plan.root);
                if plan.root.get(key).is_none() {
                    plan.root[key] = json!({});
                }
                let servers = plan.root[key]
                    .as_object_mut()
                    .context("目标 MCP 节点不是对象，未覆盖原配置")?;
                for item in &mcps {
                    let config = mcp_native::for_host(item, &host.id)?;
                    if let Some(existing) = servers.get_mut(&item.name) {
                        if !mcp_native::parse(&item.name, existing)
                            .is_ok_and(|v| mcp_native::equivalent(&v, item))
                        {
                            bail!(
                                "{} 中已有不同内容的同名 MCP“{}”，已阻止覆盖",
                                host.name,
                                item.name
                            );
                        }
                        // Take ownership without rewriting unrelated native options; explicit apply enables it.
                        if existing.get("enabled").is_some() {
                            existing["enabled"] = json!(true);
                        }
                    } else {
                        servers.insert(item.name.clone(), config);
                    }
                }
            }
        }
        // Stage every Skill before touching any host. Source changes during copying invalidate the batch.
        let staging = tempfile::tempdir()?;
        for (index, skill) in skills.iter().enumerate() {
            let staged = staging.path().join(index.to_string());
            copy_skill_dir(&skill.source, &staged)?;
            if hash_skill_path(&staged) != skill.hash {
                bail!("Skill 来源在导入时发生变化，请重新扫描");
            }
        }
        for (index, mut skill) in skills.into_iter().enumerate() {
            let central = Path::new(&skill.item.source_dir);
            journal.watch(central)?;
            mkdir(central.parent().unwrap())?;
            copy_skill_dir(&staging.path().join(index.to_string()), central)?;
            for host in hosts {
                let target = Path::new(&host.skill_path).join(&skill.item.name);
                let deployment = apply_skill_target(central, &target, journal)?;
                skill.item.deployments.insert(host.id.clone(), deployment);
            }
            state.skills.push(skill.item);
        }
        for plan in configs.values() {
            if hash_path(&plan.path) != plan.hash {
                bail!("宿主配置在导入确认后发生变化，已阻止写入");
            }
            if plan.root != plan.original {
                journal.watch(&plan.path)?;
                journal.watch(&PathBufExt::suffix(&plan.path, ".chu.bak"))?;
                write_config(&plan.path, &plan.format, &plan.root)?;
            }
        }
        for mut item in mcps {
            for host in hosts {
                item.deployments.insert(
                    host.id.clone(),
                    Deployment {
                        enabled: true,
                        mode: "native".into(),
                        target: host.config_path.clone(),
                        last_hash: hash_path(Path::new(&host.config_path)),
                    },
                );
            }
            state.mcps.push(item);
        }
        // Existing managed MCPs share the same configuration fingerprint after the single merged write.
        for item in &mut state.mcps {
            for dep in item.deployments.values_mut() {
                if dep.enabled && configs.contains_key(Path::new(&dep.target)) {
                    dep.last_hash = hash_path(Path::new(&dep.target));
                }
            }
        }
        journal.watch(&self.root.join("state.json"))?;
        self.state = state;
        self.save()
    }
}
fn validate_skill_target(target: &Path, expected: &str) -> Result<()> {
    if !exists(target) {
        return Ok(());
    }
    if !target.is_dir()
        || !target.join("SKILL.md").is_file()
        || target
            .canonicalize()
            .map(|p| hash_skill_path(&p))
            .unwrap_or_default()
            != expected
    {
        bail!(
            "目标 Skill 已存在且内容不同，已阻止覆盖: {}",
            target.display()
        );
    }
    Ok(())
}

pub(crate) fn apply_skill_target(
    source: &Path,
    target: &Path,
    journal: &mut Journal,
) -> Result<Deployment> {
    validate_skill_target(target, &hash_skill_path(source))?;
    let mode = if exists(target) && fs::symlink_metadata(target)?.is_dir() {
        "copy"
    } else if exists(target) && target.canonicalize().ok() == source.canonicalize().ok() {
        "link"
    } else {
        journal.watch(target)?;
        remove(target)?;
        mkdir(target.parent().unwrap())?;
        if symlink(source, target).is_ok() {
            "link"
        } else {
            copy_dir(source, target)?;
            "copy"
        }
    };
    Ok(Deployment {
        enabled: true,
        mode: mode.into(),
        target: target.to_string_lossy().into(),
        last_hash: hash_path(target),
    })
}
