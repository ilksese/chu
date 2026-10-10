use crate::{
    App, fs::*, git_runner, models::*, operation, skill_preview, skill_sources::PreparedCheckout,
    sources::*,
};
use anyhow::{Context, Result, bail};
use std::{fs, path::Path};

impl App {
    pub(crate) fn discovered_skills(&self, hosts: &[Host]) -> Vec<SkillView> {
        let mut found = Vec::new();
        for host in hosts {
            if let Ok(entries) = fs::read_dir(&host.skill_path) {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if !e.path().is_dir()
                        || !e.path().join("SKILL.md").is_file()
                        || self.state.skills.iter().any(|s| s.name == name)
                    {
                        continue;
                    }
                    found.push(SkillView {
                        id: format!("discovered-skill-{}-{}", host.id, slug(&name)),
                        name,
                        description: description(&e.path()),
                        source: e.path().to_string_lossy().into(),
                        enabled_on: [(host.id.clone(), true)].into(),
                        mode_by_host: [(host.id.clone(), "external".into())].into(),
                        ..SkillView::default()
                    });
                }
            }
        }
        found
    }
    pub fn read_skill_content(&self, id: &str) -> Result<String> {
        let source = if let Ok(skill) = self.skill(id) {
            skill.source_dir
        } else {
            self.discovered_skills(&self.hosts())
                .into_iter()
                .find(|s| s.id == id)
                .context("未知的 skill")?
                .source
        };
        Ok(fs::read_to_string(Path::new(&source).join("SKILL.md"))?)
    }
    pub fn import_skill(&mut self, host: &str, name: &str) -> Result<()> {
        let name = safe_name(name)?;
        let host = self.host(host)?;
        let source = Path::new(&host.skill_path).join(&name);
        if !source.join("SKILL.md").is_file() {
            bail!("未找到可导入的 SKILL.md");
        }
        if self.state.skills.iter().any(|s| s.name == name) {
            bail!("skill 已存在");
        }
        let id = identifier(&name, "skill");
        let target = self.root.join("skills").join(&id);
        if exists(&target) {
            bail!("中央 skill 目录已存在");
        }
        mkdir(target.parent().unwrap())?;
        fs::rename(&source, &target).context("移动 skill 失败")?;
        let mode = if symlink(&target, &source).is_ok() {
            "link"
        } else {
            if let Err(e) = copy_dir(&target, &source) {
                let _ = remove(&source);
                let _ = fs::rename(&target, &source);
                return Err(e);
            }
            "copy"
        };
        self.state.skills.push(StoredSkill {
            id,
            name,
            description: description(&target),
            source_dir: target.to_string_lossy().into(),
            deployments: [(
                host.id,
                Deployment {
                    enabled: true,
                    mode: mode.into(),
                    target: source.to_string_lossy().into(),
                    last_hash: hash_path(&source),
                },
            )]
            .into(),
        });
        self.save()
    }
    pub fn toggle_skill(&mut self, id: &str, host: &str, enabled: bool) -> Result<()> {
        let host = self.host(host)?;
        let skill = self.skill(id)?;
        let dep = skill.deployments.get(&host.id).filter(|d| d.enabled);
        safe_name(&skill.name)?;
        let target = Path::new(&host.skill_path).join(&skill.name);
        let source = Path::new(&skill.source_dir);
        if enabled {
            if !source.join("SKILL.md").is_file() {
                bail!("中央 skill 源目录不可用");
            }
            if dep.is_some() {
                return Ok(());
            }
            let mut journal = crate::import_transaction::Journal::new();
            journal.watch(&self.root.join("state.json"))?;
            let deployment = crate::imports::apply_skill_target(source, &target, &mut journal)?;
            let old = self.state.clone();
            self.state
                .skills
                .iter_mut()
                .find(|s| s.id == id)
                .unwrap()
                .deployments
                .insert(host.id, deployment);
            if let Err(error) = self.save() {
                self.state = old;
                journal.rollback()?;
                return Err(error);
            }
            journal.commit();
            return Ok(());
        }
        let Some(dep) = dep else {
            return Ok(());
        };
        verify_deployment(source, dep)?;
        remove(Path::new(&dep.target))?;
        self.state
            .skills
            .iter_mut()
            .find(|s| s.id == id)
            .unwrap()
            .deployments
            .remove(&host.id);
        self.save()
    }

    pub fn delete_skill(&mut self, id: &str) -> Result<()> {
        let skill = self.skill(id)?;
        let projects: Vec<_> = self
            .state
            .projects
            .iter()
            .filter(|p| p.deployments.get(id).is_some_and(|d| !d.is_empty()))
            .map(|p| p.name.clone())
            .collect();
        if !projects.is_empty() {
            bail!("该 skill 仍被项目使用: {}", projects.join("、"));
        }
        for dep in skill.deployments.values() {
            verify_deployment(Path::new(&skill.source_dir), dep)?;
        }
        let mut lock = self.read_lock()?;
        for dep in skill.deployments.values() {
            if !dep.target.is_empty() {
                remove(Path::new(&dep.target))?;
            }
        }
        if !skill.source_dir.is_empty() {
            remove(Path::new(&skill.source_dir))?;
        }
        self.state.skills.retain(|s| s.id != id);
        lock.skills.remove(&skill.name);
        lock.skills.remove(id);
        self.save()?;
        self.write_lock(&lock)
    }
    pub fn check_skill_updates(&self) -> Result<Vec<SkillUpdate>> {
        git_runner::task(|| {
            let lock = self.read_lock()?;
            let mut caches = Map::<PreparedCheckout>::new();
            let mut updates = Vec::new();
            for (name, item) in lock.skills {
                operation::check()?;
                let key = format!("{}\n{}", item.source, item.r#ref);
                if !caches.contains_key(&key) {
                    let prepared = self.prepare_checkout(&item.source, &item.r#ref)?;
                    skill_preview::snapshot_fingerprint(&prepared.path)?;
                    caches.insert(key.clone(), prepared);
                }
                let cache = &caches[&key].path;
                let found = discover(cache);
                let stored = self
                    .state
                    .skills
                    .iter()
                    .find(|s| s.name == name || s.id == name);
                let installed_name = stored
                    .and_then(|s| frontmatter(Path::new(&s.source_dir)))
                    .map(|(n, _)| n)
                    .unwrap_or_else(|| name.clone());
                let matching: Vec<_> = found
                    .iter()
                    .filter(|c| c.name == name || c.name == installed_name)
                    .collect();
                let at_path = found.iter().find(|c| c.path == item.skill_path);
                let status = if let Some(c) = at_path {
                    if hash_skill_path(&skill_preview::snapshot_path(cache, &c.path)?)
                        == item.content_hash
                    {
                        ""
                    } else {
                        "update"
                    }
                } else if matching.len() > 1 {
                    "ambiguous"
                } else if matching.len() == 1 {
                    "update"
                } else {
                    "deleted"
                };
                if !status.is_empty() {
                    updates.push(SkillUpdate {
                        id: stored.map(|s| s.id.clone()).unwrap_or(name.clone()),
                        name,
                        status: status.into(),
                    });
                }
            }
            updates.sort_by(|a, b| a.name.cmp(&b.name));
            operation::check()?;
            Ok(updates)
        })
    }
    pub fn update_skill(&mut self, id: &str) -> Result<()> {
        git_runner::task(|| {
            let mut skill = self.skill(id)?;
            let mut lock = self.read_lock()?;
            let key = if lock.skills.contains_key(&skill.name) {
                skill.name.clone()
            } else {
                skill.id.clone()
            };
            let mut item = lock
                .skills
                .get(&key)
                .cloned()
                .context("该 skill 没有来源记录，需要重新安装后才能更新")?;
            let prepared = self.prepare_checkout(&item.source, &item.r#ref)?;
            skill_preview::snapshot_fingerprint(&prepared.path)?;
            let relative = resolve(
                &item,
                &skill.name,
                Path::new(&skill.source_dir),
                &discover(&prepared.path),
            )
            .context("来源仓库里找不到这个 skill，或名称存在歧义")?;
            let source = skill_preview::snapshot_path(&prepared.path, &relative)?;
            if let Some((name, description)) = frontmatter(&source) {
                safe_name(&name)?;
                if self
                    .state
                    .skills
                    .iter()
                    .any(|s| s.id != id && s.name == name)
                {
                    bail!("更新后的 Skill 名称与现有资源冲突");
                }
                skill.name = name;
                skill.description = description;
            }
            let staging = tempfile::Builder::new()
                .prefix(".chu-update-prepare-")
                .tempdir_in(&self.root)?;
            let staged = staging.path().join("skill");
            let fingerprint = skill_preview::snapshot_fingerprint(&source)?;
            copy_skill_dir(&source, &staged)?;
            if skill_preview::snapshot_fingerprint(&staged)? != fingerprint {
                bail!("更新快照复制校验失败，未修改已安装资源");
            }
            operation::begin_commit()?;
            replace_dir(&staged, Path::new(&skill.source_dir))?;
            item.skill_path = relative;
            item.content_hash = hash_path(Path::new(&skill.source_dir));
            item.updated_at = now();
            item.resolved_commit = prepared.revision.clone();
            for dep in skill
                .deployments
                .values_mut()
                .filter(|d| d.mode == "copy" && !d.target.is_empty())
            {
                if hash_path(Path::new(&dep.target)) == dep.last_hash {
                    replace_dir(Path::new(&skill.source_dir), Path::new(&dep.target))?;
                    dep.last_hash = hash_path(Path::new(&dep.target));
                }
            }
            self.sync_project_copies(&skill)?;
            *self.state.skills.iter_mut().find(|s| s.id == id).unwrap() = skill.clone();
            lock.skills.remove(&key);
            lock.skills.insert(skill.name, item);
            self.save()?;
            self.write_lock(&lock)
        })
    }
}
pub fn verify_deployment(source: &Path, dep: &Deployment) -> Result<()> {
    let target = Path::new(&dep.target);
    if dep.mode == "copy" {
        if hash_path(target) != dep.last_hash {
            bail!("目标 copy 已被修改，已保护用户改动");
        }
    } else if exists(target) {
        let info = fs::symlink_metadata(target)?;
        if !info.file_type().is_symlink()
            || target.canonicalize().ok() != source.canonicalize().ok()
        {
            bail!("目标 link 已被修改，已保护用户改动");
        }
    }
    Ok(())
}
