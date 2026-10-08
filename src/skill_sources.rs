//! Git checkout, source installation and legacy origin migration.
use crate::{App, fs::*, models::*, sources::*};
use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
impl App {
    pub fn checkout(&self, source: &str, reference: &str) -> Result<PathBuf> {
        let reference = if reference.is_empty() {
            "main"
        } else {
            reference
        };
        if reference.starts_with('-') || source.starts_with('-') {
            bail!("非法 Git 来源或分支");
        }
        let key = format!(
            "{:x}",
            Sha256::digest(format!("{source}\n{reference}").as_bytes())
        );
        let cache = self.root.join("sources").join(key);
        let git = |args: Vec<String>| -> Result<()> {
            let output = Command::new("git")
                .args(args)
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .context("Git 未安装或无法启动")?;
            if !output.status.success() {
                bail!(
                    "Git 操作失败: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Ok(())
        };
        let c = cache.to_string_lossy().into_owned();
        if cache.join(".git").exists() {
            git(vec![
                "-C".into(),
                c.clone(),
                "fetch".into(),
                "--depth".into(),
                "1".into(),
                "origin".into(),
                reference.into(),
            ])?;
            git(vec![
                "-C".into(),
                c,
                "checkout".into(),
                "--detach".into(),
                "FETCH_HEAD".into(),
            ])?;
        } else {
            remove(&cache)?;
            mkdir(cache.parent().unwrap())?;
            if let Err(e) = git(vec![
                "clone".into(),
                "--depth".into(),
                "1".into(),
                "--branch".into(),
                reference.into(),
                "--".into(),
                source.into(),
                c,
            ]) {
                let _ = remove(&cache);
                return Err(e);
            }
        }
        Ok(cache)
    }
    pub fn preview_skills(&self, repository: &str) -> Result<Vec<SkillCandidate>> {
        let source = parse_source(repository)?;
        let cache = self.checkout(&source.url, &source.reference)?;
        let scan = if source.subpath.is_empty() {
            cache
        } else {
            safe_relative(&cache, &source.subpath)?
        };
        let mut found = discover(&scan);
        // Preview paths are repository-relative, including GitHub/GitLab tree subpaths.
        for candidate in &mut found {
            if !source.subpath.is_empty() {
                candidate.path = if candidate.path == "." {
                    source.subpath.clone()
                } else {
                    format!("{}/{}", source.subpath, candidate.path)
                };
            }
            candidate.installed = self.state.skills.iter().any(|s| s.name == candidate.name);
        }
        found.retain(|c| {
            source.filter.is_empty()
                || source.filter == "*"
                || c.name.eq_ignore_ascii_case(&source.filter)
        });
        Ok(found)
    }
    pub fn install_skills(&mut self, repository: &str, paths: Vec<String>) -> Result<()> {
        let source = parse_source(repository)?;
        if paths.is_empty() {
            bail!("请至少选择一个 skill");
        }
        let cache = self.checkout(&source.url, &source.reference)?;
        self.install_from_cache(&source, &cache, &paths)
    }
    pub fn install_from_cache(
        &mut self,
        source: &Source,
        cache: &Path,
        paths: &[String],
    ) -> Result<()> {
        if paths.is_empty() {
            bail!("请至少选择一个 skill");
        }
        let mut lock = self.read_lock()?;
        let mut errors = Vec::new();
        for relative in paths {
            let result = (|| -> Result<()> {
                let origin = safe_relative(cache, relative)?;
                let (name, description) = frontmatter(&origin).context("不是合法 skill")?;
                safe_name(&name)?;
                if self.state.skills.iter().any(|s| s.name == name) {
                    bail!("skill {name} 已存在");
                }
                let id = identifier(&name, "skill");
                let target = self.root.join("skills").join(&id);
                if exists(&target) {
                    bail!("中央 skill 目录已存在");
                }
                copy_skill_dir(&origin, &target)?;
                self.state.skills.push(StoredSkill {
                    id,
                    name: name.clone(),
                    description,
                    source_dir: target.to_string_lossy().into(),
                    deployments: Map::new(),
                });
                let time = now();
                lock.skills.insert(
                    name,
                    SkillLock {
                        source: source.url.clone(),
                        source_type: source.kind.clone(),
                        r#ref: source.reference.clone(),
                        skill_path: relative.clone(),
                        content_hash: hash_path(&target),
                        installed_at: time.clone(),
                        updated_at: time,
                    },
                );
                Ok(())
            })();
            if let Err(e) = result {
                errors.push(format!("{relative}: {e}"));
            }
        }
        self.save()?;
        self.write_lock(&lock)?;
        if !errors.is_empty() {
            bail!("部分 skill 未安装: {}", errors.join("; "));
        }
        Ok(())
    }
    pub(crate) fn migrate_sources(&mut self, raw: &[u8]) -> Result<()> {
        let value: serde_json::Value = serde_json::from_slice(raw)?;
        let mut lock = self.read_lock()?;
        let mut changed = false;
        for item in value
            .get("skills")
            .and_then(|s| s.as_array())
            .into_iter()
            .flatten()
        {
            let name = item["name"].as_str().unwrap_or("");
            let repo = item["repository"].as_str().unwrap_or("");
            if repo.is_empty() || lock.skills.contains_key(name) {
                continue;
            }
            if let Ok(source) = parse_source(repo) {
                let time = now();
                let path = if name == "grill-me" && source.url.contains("mattpocock/skills") {
                    "skills/productivity/grill-me"
                } else {
                    "."
                };
                lock.skills.insert(
                    name.into(),
                    SkillLock {
                        source: source.url,
                        source_type: source.kind,
                        r#ref: item["version"]
                            .as_str()
                            .filter(|v| !v.is_empty())
                            .unwrap_or(&source.reference)
                            .into(),
                        skill_path: path.into(),
                        content_hash: hash_path(Path::new(
                            item["sourceDir"].as_str().unwrap_or(""),
                        )),
                        installed_at: time.clone(),
                        updated_at: time,
                    },
                );
                changed = true;
            }
        }
        if changed {
            self.write_lock(&lock)?;
            self.save()?;
        }
        Ok(())
    }
}
