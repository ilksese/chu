//! Git 准备快照、兼容安装 API 与旧版来源迁移。
use crate::{
    App,
    fs::*,
    git_runner::{self, GitRunner},
    models::*,
    operation::{self, Phase},
    skill_preview,
    sources::*,
};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

pub(crate) struct PreparedCheckout {
    pub directory: TempDir,
    pub path: PathBuf,
    pub revision: String,
}

pub(crate) fn cache_path(app: &App, source: &str, reference: &str) -> PathBuf {
    let key = format!(
        "{:x}",
        Sha256::digest(format!("{source}\n{reference}").as_bytes())
    );
    app.root.join("sources").join(key)
}

fn validate_reference(reference: &str) -> Result<&str> {
    let reference = if reference.is_empty() {
        "main"
    } else {
        reference
    };
    if reference.starts_with(['-', '/'])
        || reference.ends_with(['/', '.'])
        || reference.contains("..")
        || reference.contains("@{")
        || reference.contains("//")
        || reference
            .split('/')
            .any(|s| s.ends_with(".lock") || s.starts_with('.'))
        || reference
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "~^:?*[\\".contains(c))
    {
        bail!("非法 Git 分支或版本");
    }
    Ok(reference)
}

pub(crate) fn validate_git_source(source: &str) -> Result<()> {
    if Path::new(source).is_absolute() && !source.contains('\0') {
        return Ok(());
    }
    let url = url::Url::parse(source).map_err(|_| anyhow::anyhow!("非法 Git 来源"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("Git 来源必须是不含认证信息的公开 HTTPS 仓库");
    }
    Ok(())
}

/// 绝对本地仓库供自动化/隔离测试使用，公开 HTTPS 来源语法保持不变。
pub(crate) fn preview_source(repository: &str) -> Result<Source> {
    let input = repository.trim();
    let (base, fragment) = input.split_once('#').unwrap_or((input, ""));
    let source = if Path::new(base).is_absolute() {
        let (reference, filter) = fragment.split_once('@').unwrap_or((fragment, ""));
        Source {
            kind: "git".into(),
            url: base.into(),
            reference: if reference.is_empty() {
                "main".into()
            } else {
                reference.into()
            },
            subpath: String::new(),
            filter: filter.into(),
        }
    } else {
        // parse_source 会规范化 GitHub URL，必须先拒绝可能被丢弃的认证/查询信息。
        if let Ok(url) = url::Url::parse(base)
            && (!url.username().is_empty() || url.password().is_some() || url.query().is_some())
        {
            bail!("Git 来源不能包含认证信息或查询参数");
        }
        parse_source(input)?
    };
    validate_git_source(&source.url)?;
    validate_reference(&source.reference)?;
    Ok(source)
}

fn run(runner: &GitRunner, args: Vec<OsString>, label: &str) -> Result<Vec<u8>> {
    runner.run(args, label)
}

pub(crate) fn revision(runner: &GitRunner, path: &Path) -> Result<String> {
    let output = run(
        runner,
        vec![
            "-C".into(),
            path.into(),
            "rev-parse".into(),
            "--verify".into(),
            "HEAD^{commit}".into(),
        ],
        "正在校验 Git 版本",
    )?;
    let revision = std::str::from_utf8(&output).unwrap_or("").trim();
    if !matches!(revision.len(), 40 | 64) || !revision.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Git 未返回有效的提交版本");
    }
    Ok(revision.to_ascii_lowercase())
}

impl App {
    pub(crate) fn prepare_checkout(
        &self,
        source: &str,
        reference: &str,
    ) -> Result<PreparedCheckout> {
        operation::check()?;
        validate_git_source(source)?;
        if Path::new(source).is_absolute() && !Path::new(source).is_dir() {
            bail!("本地 Git 来源不可用");
        }
        let reference = validate_reference(reference)?;
        let cache = cache_path(self, source, reference);
        let parent = cache.parent().unwrap();
        if exists(parent) && fs::symlink_metadata(parent)?.file_type().is_symlink() {
            bail!("来源缓存目录不可用");
        }
        mkdir(parent)?;
        let directory = tempfile::Builder::new()
            .prefix(".chu-git-prepare-")
            .tempdir_in(parent)?;
        let path = directory.path().join("repository");
        let runner = GitRunner::new(&self.home);
        let origin = if cache.join(".git").is_dir() {
            if fs::symlink_metadata(&cache)?.file_type().is_symlink()
                || fs::symlink_metadata(cache.join(".git"))?
                    .file_type()
                    .is_symlink()
            {
                bail!("来源缓存不是可信的普通目录");
            }
            let output = run(
                &runner,
                vec![
                    "-C".into(),
                    cache.as_os_str().into(),
                    "config".into(),
                    "--local".into(),
                    "--no-includes".into(),
                    "--get".into(),
                    "remote.origin.url".into(),
                ],
                "正在准备已缓存来源",
            )?;
            let origin = std::str::from_utf8(&output).unwrap_or("").trim().to_owned();
            validate_git_source(&origin)?;
            // 仅复制对象，不复制缓存配置、钩子或工作树；原缓存不做修改。
            run(
                &runner,
                vec![
                    "clone".into(),
                    "--no-local".into(),
                    "--no-checkout".into(),
                    "--template=".into(),
                    "--".into(),
                    cache.as_os_str().into(),
                    path.as_os_str().into(),
                ],
                "正在准备缓存快照",
            )?;
            origin
        } else {
            if exists(&cache) {
                bail!("来源缓存已存在但不是有效 Git 仓库，未覆盖原目录");
            }
            run(
                &runner,
                vec![
                    "init".into(),
                    "--template=".into(),
                    "--".into(),
                    path.as_os_str().into(),
                ],
                "正在准备来源目录",
            )?;
            run(
                &runner,
                vec![
                    "-C".into(),
                    path.as_os_str().into(),
                    "remote".into(),
                    "add".into(),
                    "origin".into(),
                    source.into(),
                ],
                "正在准备 Git 来源",
            )?;
            source.into()
        };
        run(
            &runner,
            vec![
                "-C".into(),
                path.as_os_str().into(),
                "remote".into(),
                "set-url".into(),
                "origin".into(),
                origin.into(),
            ],
            "正在准备 Git 来源",
        )?;
        run(
            &runner,
            vec![
                "-C".into(),
                path.as_os_str().into(),
                "fetch".into(),
                "--depth".into(),
                "1".into(),
                "--no-tags".into(),
                "origin".into(),
                reference.into(),
            ],
            "正在获取来源版本",
        )?;
        run(
            &runner,
            vec![
                "-C".into(),
                path.as_os_str().into(),
                "checkout".into(),
                "--detach".into(),
                "--force".into(),
                "FETCH_HEAD".into(),
            ],
            "正在准备固定版本快照",
        )?;
        let revision = revision(&runner, &path)?;
        operation::check()?;
        operation::progress("正在校验来源快照", Phase::Preparing);
        Ok(PreparedCheckout {
            directory,
            path,
            revision,
        })
    }

    /// 兼容 checkout 仅发布完整准备的仓库，失败保留旧缓存。
    pub fn checkout(&self, source: &str, reference: &str) -> Result<PathBuf> {
        git_runner::task(|| {
            let prepared = self.prepare_checkout(source, reference)?;
            let reference = validate_reference(reference)?;
            let cache = cache_path(self, source, reference);
            let old = prepared.directory.path().join("previous");
            operation::begin_commit()?;
            if exists(&cache) {
                fs::rename(&cache, &old)?;
            }
            if fs::rename(&prepared.path, &cache).is_err() {
                if exists(&old) && fs::rename(&old, &cache).is_err() {
                    // 恢复失败也必须保留旧版本，不能让准备目录的 Drop 删除唯一备份。
                    let _retained_backup = prepared.directory.keep();
                    bail!("缓存发布和恢复均失败，旧缓存已保留在来源准备目录，请检查权限");
                }
                bail!("来源缓存发布失败，已保留上一版本");
            }
            Ok(cache)
        })
    }

    pub fn preview_skills(&self, repository: &str) -> Result<Vec<SkillCandidate>> {
        git_runner::task(|| {
            let source = parse_source(repository)?;
            let prepared = self.prepare_checkout(&source.url, &source.reference)?;
            let found = self.source_candidates(&source, &prepared.path)?;
            operation::check()?;
            Ok(found)
        })
    }

    pub(crate) fn source_candidates(
        &self,
        source: &Source,
        cache: &Path,
    ) -> Result<Vec<SkillCandidate>> {
        let scan = if source.subpath.is_empty() {
            cache.to_path_buf()
        } else {
            skill_preview::snapshot_path(cache, &source.subpath)?
        };
        let mut found = discover(&scan);
        for candidate in &mut found {
            operation::check()?;
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
        git_runner::task(|| {
            let source = parse_source(repository)?;
            if paths.is_empty() {
                bail!("请至少选择一个 skill");
            }
            let prepared = self.prepare_checkout(&source.url, &source.reference)?;
            skill_preview::install_snapshot(
                self,
                &source,
                &prepared.path,
                &paths,
                &prepared.revision,
                None,
            )
        })
    }

    pub fn install_from_cache(
        &mut self,
        source: &Source,
        cache: &Path,
        paths: &[String],
    ) -> Result<()> {
        git_runner::task(|| {
            let commit = if cache.join(".git").is_dir() {
                revision(&GitRunner::new(&self.home), cache)?
            } else {
                String::new()
            };
            skill_preview::install_snapshot(self, source, cache, paths, &commit, None)
        })
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
                        resolved_commit: String::new(),
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
