use crate::{App, fs::*, hosts::HOST_IDS, mcp::PathBufExt, models::*};
use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn still_managed(source: &Path, dep: &PromptDeployment) -> bool {
    let target = Path::new(&dep.target);
    let Ok(info) = fs::symlink_metadata(target) else {
        return false;
    };
    if info.file_type().is_symlink() {
        let a = target.canonicalize();
        let b = source.canonicalize();
        a.is_ok() && b.is_ok() && a.ok() == b.ok()
    } else {
        dep.mode == "copy" && md5_file(target) == dep.last_hash
    }
}
impl App {
    pub(crate) fn reconcile_prompts(&mut self) -> Result<LockFile> {
        let mut lock = self.read_lock()?;
        let old = lock.clone();
        let dir = self.root.join("prompts");
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let file = e.file_name().to_string_lossy().into_owned();
                if !file.ends_with(".md")
                    || !e.file_type().is_ok_and(|t| t.is_file())
                    || lock.prompts.values().any(|p| p.file == file)
                {
                    continue;
                }
                let name = file.strip_suffix(".md").unwrap_or(&file);
                let id = identifier(name, "prompt");
                lock.prompts.entry(id).or_insert_with(|| PromptLock {
                    name: name.into(),
                    file: file.clone(),
                    content_hash: md5_file(&e.path()),
                    updated_at: now(),
                    deployments: Map::new(),
                });
            }
        }
        for item in lock.prompts.values_mut() {
            safe_name(&item.file)?;
            let source = dir.join(&item.file);
            let hash = md5_file(&source);
            if hash.is_empty() {
                item.deployments.clear();
                continue;
            }
            if item.content_hash != hash {
                item.content_hash = hash.clone();
                item.updated_at = now();
            }
            let mut discarded = Vec::new();
            for (host, dep) in &mut item.deployments {
                if still_managed(&source, dep) {
                    if dep.mode == "copy" && md5_file(Path::new(&dep.target)) != hash {
                        copy_file(&source, Path::new(&dep.target))?;
                    }
                    dep.last_hash = hash.clone();
                } else {
                    discarded.push(host.clone());
                }
            }
            for host in discarded {
                item.deployments.remove(&host);
            }
        }
        if lock != old {
            self.write_lock(&lock)?;
        }
        Ok(lock)
    }
    pub fn prompt_target(&self, host: &str) -> Result<PathBuf> {
        let host = self.host(host)?;
        let mut dir = Path::new(&host.config_path)
            .parent()
            .context("宿主配置缺少父目录")?
            .to_owned();
        let name = match host.id.as_str() {
            "claude" => "CLAUDE.md",
            "jcode" => "prompt-overlay.md",
            "opencode" => {
                if dir.file_name().is_none_or(|n| n != "opencode") {
                    dir.push("opencode");
                }
                "AGENTS.md"
            }
            _ => "AGENTS.md",
        };
        Ok(dir.join(name))
    }
    pub fn create_prompt(&mut self, name: &str, content: &str) -> Result<()> {
        let name = markdown_name(name)?;
        let id = identifier(&name, "prompt");
        let mut lock = self.reconcile_prompts()?;
        if lock.prompts.contains_key(&id) {
            bail!("提示词已存在");
        }
        let file = format!("{name}.md");
        let path = self.root.join("prompts").join(&file);
        if exists(&path) {
            bail!("提示词文件已存在");
        }
        atomic_write(&path, content.as_bytes())?;
        lock.prompts.insert(
            id,
            PromptLock {
                name,
                file,
                content_hash: md5_file(&path),
                updated_at: now(),
                deployments: Map::new(),
            },
        );
        self.write_lock(&lock)
    }
    pub fn read_prompt(&self, id: &str) -> Result<String> {
        let lock = self.read_lock()?;
        let item = lock.prompts.get(id).context("未知的提示词")?;
        safe_name(&item.file)?;
        Ok(fs::read_to_string(
            self.root.join("prompts").join(&item.file),
        )?)
    }
    pub fn update_prompt(&mut self, id: &str, name: &str, content: &str) -> Result<()> {
        let name = markdown_name(name)?;
        let next_id = identifier(&name, "prompt");
        let mut lock = self.reconcile_prompts()?;
        let mut item = lock.prompts.get(id).cloned().context("未知的提示词")?;
        if next_id != id && lock.prompts.contains_key(&next_id) {
            bail!("提示词已存在");
        }
        let old = self.root.join("prompts").join(&item.file);
        let file = format!("{name}.md");
        let next = self.root.join("prompts").join(&file);
        if next != old && exists(&next) {
            bail!("提示词文件已存在");
        }
        // Only copies still matching the recorded hash are overwritten.
        atomic_write(&next, content.as_bytes())?;
        for dep in item.deployments.values_mut() {
            if dep.mode == "link" && next != old {
                remove(Path::new(&dep.target))?;
                if symlink(&next, Path::new(&dep.target)).is_err() {
                    copy_file(&next, Path::new(&dep.target))?;
                    dep.mode = "copy".into();
                }
            } else if dep.mode == "copy" {
                copy_file(&next, Path::new(&dep.target))?;
            }
            dep.last_hash = md5_file(&next);
        }
        if next != old {
            remove(&old)?;
        }
        item.name = name;
        item.file = file;
        item.content_hash = md5_file(&next);
        item.updated_at = now();
        lock.prompts.remove(id);
        lock.prompts.insert(next_id, item);
        self.write_lock(&lock)
    }
    fn detach_prompt(&self, lock: &mut LockFile, id: &str, host: &str) -> Result<()> {
        let item = lock.prompts.get_mut(id).context("未知的提示词")?;
        let Some(dep) = item.deployments.get(host).cloned() else {
            return Ok(());
        };
        let source = self.root.join("prompts").join(&item.file);
        if exists(&dep.target) && !still_managed(&source, &dep) {
            bail!("宿主提示词已被修改，已保护用户改动");
        }
        remove(Path::new(&dep.target))?;
        let backup = PathBufExt::suffix(Path::new(&dep.target), ".chubak");
        if dep.backup && exists(&backup) {
            fs::rename(backup, &dep.target).context("恢复宿主提示词失败")?;
        }
        item.deployments.remove(host);
        Ok(())
    }
    pub fn toggle_prompt(&mut self, id: &str, host: &str, enabled: bool) -> Result<()> {
        let target = self.prompt_target(host)?;
        if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir()) {
            bail!("宿主提示词路径是目录，已保护用户文件");
        }
        let mut lock = self.reconcile_prompts()?;
        let mut item = lock.prompts.get(id).cloned().context("未知的提示词")?;
        if !enabled {
            self.detach_prompt(&mut lock, id, host)?;
            return self.write_lock(&lock);
        }
        let source = self.root.join("prompts").join(&item.file);
        if md5_file(&source).is_empty() {
            bail!("提示词文件不可用");
        }
        if item
            .deployments
            .get(host)
            .is_some_and(|d| still_managed(&source, d))
        {
            return Ok(());
        }
        let mut backup = exists(PathBufExt::suffix(&target, ".chubak"));
        let mut managed = false;
        for (other_id, other) in &mut lock.prompts {
            if other_id == id {
                continue;
            }
            if let Some(dep) = other.deployments.remove(host) {
                managed = true;
                backup |= dep.backup;
                remove(Path::new(&dep.target))?;
            }
        }
        if exists(&target) && !managed && !backup {
            fs::rename(&target, PathBufExt::suffix(&target, ".chubak"))
                .context("备份宿主提示词失败")?;
            backup = true;
        } else {
            remove(&target)?;
        }
        mkdir(target.parent().unwrap())?;
        let mode = if symlink(&source, &target).is_ok() {
            "link"
        } else {
            copy_file(&source, &target)?;
            "copy"
        };
        item.content_hash = md5_file(&source);
        item.deployments.insert(
            host.into(),
            PromptDeployment {
                mode: mode.into(),
                target: target.to_string_lossy().into(),
                last_hash: item.content_hash.clone(),
                backup,
            },
        );
        lock.prompts.insert(id.into(), item);
        self.write_lock(&lock)
    }
    pub fn delete_prompt(&mut self, id: &str) -> Result<()> {
        let mut lock = self.reconcile_prompts()?;
        let item = lock.prompts.get(id).cloned().context("未知的提示词")?;
        for host in item.deployments.keys() {
            self.detach_prompt(&mut lock, id, host)?;
        }
        remove(&self.root.join("prompts").join(&item.file))?;
        lock.prompts.remove(id);
        self.write_lock(&lock)
    }
    pub(crate) fn prompt_views(&self, lock: &LockFile) -> Vec<PromptView> {
        let mut views: Vec<_> = lock
            .prompts
            .iter()
            .map(|(id, item)| {
                let source = self.root.join("prompts").join(&item.file);
                PromptView {
                    id: id.clone(),
                    name: item.name.clone(),
                    source: source.to_string_lossy().into(),
                    preview: preview(&source),
                    enabled_on: HOST_IDS
                        .iter()
                        .map(|h| (h.to_string(), item.deployments.contains_key(*h)))
                        .collect(),
                    mode_by_host: item
                        .deployments
                        .iter()
                        .map(|(h, d)| (h.clone(), d.mode.clone()))
                        .collect(),
                }
            })
            .collect();
        views.sort_by(|a, b| a.name.cmp(&b.name));
        views
    }
}
