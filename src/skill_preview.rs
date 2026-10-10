//! 进程内预览能力绑定提交、候选集合及独立准备快照，不持久化令牌。
use crate::{
    App,
    fs::*,
    git_runner,
    import_transaction::Journal,
    models::*,
    operation::{self, Phase},
    preview_store::{self, Preview},
    skill_sources,
    sources::{Source, frontmatter},
};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};
use tempfile::TempDir;

thread_local! {
    static FAIL_AFTER_STATE_SAVE: Cell<bool> = const { Cell::new(false) };
    static BEFORE_COMMIT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
}
struct CommitScope(Option<Box<dyn FnOnce()>>);
impl Drop for CommitScope {
    fn drop(&mut self) {
        BEFORE_COMMIT.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}
/// 隔离测试在预检完成后设置屏障，验证取消及外部目标竞争，不改全局环境。
#[doc(hidden)]
pub fn with_before_install_commit<T>(barrier: impl FnOnce() + 'static, f: impl FnOnce() -> T) -> T {
    let _scope = CommitScope(BEFORE_COMMIT.with(|slot| slot.replace(Some(Box::new(barrier)))));
    f()
}
struct FailureScope(bool);
impl Drop for FailureScope {
    fn drop(&mut self) {
        FAIL_AFTER_STATE_SAVE.with(|flag| flag.set(self.0));
    }
}
/// 隔离测试注入状态保存后的失败，验证整批目录及 state/lock 回滚，不修改全局环境。
#[doc(hidden)]
pub fn with_install_failure<T>(f: impl FnOnce() -> T) -> T {
    let _scope = FailureScope(FAIL_AFTER_STATE_SAVE.with(|flag| flag.replace(true)));
    f()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SkillPreview {
    pub token: String,
    pub source: String,
    pub revision: String,
    pub candidates: Vec<SkillCandidate>,
}

fn display(source: &Source) -> String {
    let mut value = source.url.clone();
    if !source.subpath.is_empty() {
        value.push('/');
        value.push_str(&source.subpath);
    }
    if source.reference != "main" {
        value.push('#');
        value.push_str(&source.reference);
    }
    if !source.filter.is_empty() {
        value.push('@');
        value.push_str(&source.filter);
    }
    value
}

impl App {
    pub fn preview_skills_bound(&self, repository: &str) -> Result<SkillPreview> {
        git_runner::task(|| {
            let source = skill_sources::preview_source(repository)?;
            let checkout = self.prepare_checkout(&source.url, &source.reference)?;
            // 先校验快照，再允许发现流程读取 SKILL.md 或插件清单。
            let fingerprint = snapshot_fingerprint(&checkout.path)?;
            let candidates = self.source_candidates(&source, &checkout.path)?;
            let mut hashes = BTreeMap::new();
            for candidate in &candidates {
                let path = snapshot_path(&checkout.path, &candidate.path)?;
                safe_name(&candidate.name)?;
                hashes.insert(candidate.path.clone(), snapshot_fingerprint(&path)?);
            }
            operation::check()?;
            let result = SkillPreview {
                token: uuid::Uuid::new_v4().to_string(),
                source: display(&source),
                revision: checkout.revision.clone(),
                candidates,
            };
            preview_store::register(
                &self.preview_store,
                result.token.clone(),
                Preview {
                    source,
                    checkout,
                    fingerprint,
                    candidates: hashes,
                },
            )?;
            // 若取消先胜出，只清理本任务刚注册的令牌及 TempDir。
            if let Some(op) = operation::current()
                && let Err(error) = op.finish_preparation()
            {
                self.drop_skill_preview(&result.token)?;
                return Err(error);
            }
            Ok(result)
        })
    }

    /// 关闭/换来源时幂等清理，其他域 App 的令牌不能删除它的快照。
    pub fn drop_skill_preview(&self, token: &str) -> Result<()> {
        preview_store::drop_preview(&self.preview_store, token)
    }

    pub fn install_skills_from_preview(&mut self, token: &str, paths: Vec<String>) -> Result<()> {
        operation::check()?;
        // 暂取能力以固定 TempDir 生命周期，文件操作不持有全局或 Store 锁。
        let preview = preview_store::take(&self.preview_store, token)?;
        let result = (|| {
            if snapshot_fingerprint(&preview.checkout.path)? != preview.fingerprint {
                bail!("预览快照已被修改，请重新预览");
            }
            install_snapshot(
                self,
                &preview.source,
                &preview.checkout.path,
                &paths,
                &preview.checkout.revision,
                Some(&preview.candidates),
            )
        })();
        // 失败保留未撤销的能力供修正重试；成功只消费当前令牌。
        if result.is_err() {
            preview_store::finish(&self.preview_store, token, Some(preview))?;
        } else {
            preview_store::finish(&self.preview_store, token, None)?;
        }
        result
    }
}

/// 只接受规范的仓库相对候选路径，所有父级目录不得是链接。
pub(crate) fn snapshot_path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.contains(['\\', '\0']) {
        bail!("非法预览路径");
    }
    let path = Path::new(relative);
    if relative != "."
        && (path.is_absolute()
            || relative
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_))))
    {
        bail!("非法预览路径");
    }
    let mut target = root.to_path_buf();
    if fs::symlink_metadata(root)?.file_type().is_symlink() {
        bail!("预览根目录不能是链接");
    }
    if relative != "." {
        for part in path.components() {
            target.push(part);
            if fs::symlink_metadata(&target)?.file_type().is_symlink() {
                bail!("预览路径不能经过链接");
            }
        }
    }
    if !target.is_dir() || !target.canonicalize()?.starts_with(root.canonicalize()?) {
        bail!("预览路径不是仓库内目录");
    }
    Ok(target)
}

fn field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value);
}

/// 严格失败关闭的指纹，独立于已有部署 content_hash 兼容语义。
/// 允许内部相对链接；拒绝绝对、失效、越界链接及特殊文件。
pub(crate) fn snapshot_fingerprint(root: &Path) -> Result<String> {
    let info = fs::symlink_metadata(root)?;
    if !info.is_dir() || info.file_type().is_symlink() {
        bail!("预览快照不可用");
    }
    let canonical = root.canonicalize()?;
    let mut hash = Sha256::new();
    for entry in walkdir::WalkDir::new(root)
        .min_depth(1)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() != 1 || e.file_name() != ".git")
    {
        operation::check()?;
        let entry = entry.map_err(|_| anyhow::anyhow!("无法完整读取预览快照"))?;
        let relative = entry.path().strip_prefix(root)?;
        field(
            &mut hash,
            relative
                .to_str()
                .context("预览包含无效路径编码")?
                .as_bytes(),
        );
        let kind = entry.file_type();
        if kind.is_symlink() {
            let link = fs::read_link(entry.path())?;
            let destination = entry
                .path()
                .canonicalize()
                .map_err(|_| anyhow::anyhow!("预览包含失效链接"))?;
            if link.is_absolute()
                || !destination.starts_with(&canonical)
                || destination.starts_with(canonical.join(".git"))
            {
                bail!("预览包含不安全的链接");
            }
            hash.update(b"link");
            field(
                &mut hash,
                link.to_str().context("预览链接编码无效")?.as_bytes(),
            );
        } else if kind.is_dir() {
            hash.update(b"dir");
        } else if kind.is_file() {
            hash.update(b"file");
            let metadata = entry.metadata()?;
            hash.update(metadata.len().to_le_bytes());
            let mut file = fs::File::open(entry.path())?;
            let mut buffer = [0; 64 * 1024];
            loop {
                operation::check()?;
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hash.update(&buffer[..n]);
            }
        } else {
            bail!("预览包含不支持的特殊文件");
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

struct PlannedSkill {
    relative: String,
    name: String,
    description: String,
    id: String,
    target: PathBuf,
    staged: PathBuf,
    fingerprint: String,
}

pub(crate) fn install_snapshot(
    app: &mut App,
    source: &Source,
    snapshot: &Path,
    paths: &[String],
    revision: &str,
    bound: Option<&BTreeMap<String, String>>,
) -> Result<()> {
    operation::check()?;
    if paths.is_empty() {
        bail!("请至少选择一个 skill");
    }
    skill_sources::validate_git_source(&source.url)?;
    let mut lock = app.read_lock()?;
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let skills = app.root.join("skills");
    if exists(&skills)
        && (!skills.is_dir() || fs::symlink_metadata(&skills)?.file_type().is_symlink())
    {
        bail!("中央 skill 目录不可用，未修改原路径");
    }
    mkdir(&app.root)?;
    let staging: TempDir = tempfile::Builder::new()
        .prefix(".chu-install-prepare-")
        .tempdir_in(&app.root)?;
    let mut plan = Vec::new();
    for relative in paths {
        operation::check()?;
        if !selected.insert(relative.clone()) {
            bail!("不能重复选择同一个 skill");
        }
        if let Some(bound) = bound
            && !bound.contains_key(relative)
        {
            bail!("所选路径不属于此预览，请重新选择");
        }
        let origin = snapshot_path(snapshot, relative)?;
        let fingerprint = snapshot_fingerprint(&origin)?;
        if bound.is_some_and(|b| b.get(relative) != Some(&fingerprint)) {
            bail!("预览 skill 已被修改，请重新预览");
        }
        let (name, description) = frontmatter(&origin).context("不是合法 skill")?;
        safe_name(&name)?;
        let id = identifier(&name, "skill");
        if app
            .state
            .skills
            .iter()
            .any(|s| s.name == name || s.id == id)
            || lock.skills.contains_key(&name)
            || lock.skills.contains_key(&id)
            || !names.insert(name.clone())
            || !ids.insert(id.clone())
        {
            bail!("所选 skill 与已安装资源或同批资源冲突，未安装任何项");
        }
        let target = skills.join(&id);
        if exists(&target) {
            bail!("中央 skill 路径已存在，未覆盖原目录");
        }
        let staged = staging.path().join(&id);
        copy_skill_dir(&origin, &staged)?;
        if snapshot_fingerprint(&staged)? != fingerprint
            || snapshot_fingerprint(&origin)? != fingerprint
        {
            bail!("复制期间预览内容发生变化，未安装任何项");
        }
        plan.push(PlannedSkill {
            relative: relative.clone(),
            name,
            description,
            id,
            target,
            staged,
            fingerprint,
        });
    }
    operation::progress("正在完成安装预检", Phase::Preparing);
    let mut journal = Journal::new();
    let mut targets = Vec::<Journal>::new();
    let prepared = (|| -> Result<()> {
        // 提交前只观察元数据，不纳入尚未取得所有权的目标目录。
        journal.watch(&app.root.join("state.json"))?;
        journal.watch(&app.root.join("chu-lock.json"))?;
        for item in &plan {
            if exists(&item.target)
                || snapshot_fingerprint(&snapshot_path(snapshot, &item.relative)?)?
                    != item.fingerprint
            {
                bail!("安装预检已失效，未修改现有资源");
            }
        }
        if let Some(barrier) = BEFORE_COMMIT.with(|slot| slot.borrow_mut().take()) {
            barrier();
        }
        operation::begin_commit()
    })();
    if let Err(error) = prepared {
        // 尚未修改资源，解除空日志，避免取消反而触发 Drop 写回原文件。
        journal.commit();
        return Err(error);
    }
    let old = app.state.clone();
    let mut created_parent = false;
    let result = (|| -> Result<()> {
        if !exists(&skills) {
            match fs::create_dir(&skills) {
                Ok(()) => created_parent = true,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        if !skills.is_dir() || fs::symlink_metadata(&skills)?.file_type().is_symlink() {
            bail!("中央 skill 目录已变化，未覆盖原路径");
        }
        for item in plan {
            // create_dir 原子取得新路径所有权，若外部目录先出现则不进入回滚列表。
            let mut target_journal = Journal::new();
            if exists(&item.target) {
                bail!("安装目标已出现，未覆盖现有资源");
            }
            target_journal.watch(&item.target)?;
            if fs::create_dir(&item.target).is_err() {
                target_journal.commit();
                bail!("安装目标已出现或不可创建，未覆盖现有资源");
            }
            copy_skill_dir(&item.staged, &item.target)?;
            if snapshot_fingerprint(&item.target)? != item.fingerprint {
                bail!("安装内容复制校验失败");
            }
            // 保留 Missing 日志以供整批失败回滚，每项只包含本任务创建的新目录。
            targets.push(target_journal);
            app.state.skills.push(StoredSkill {
                id: item.id,
                name: item.name.clone(),
                description: item.description,
                source_dir: item.target.to_string_lossy().into(),
                deployments: Map::new(),
            });
            let time = now();
            lock.skills.insert(
                item.name,
                SkillLock {
                    source: source.url.clone(),
                    source_type: source.kind.clone(),
                    r#ref: source.reference.clone(),
                    skill_path: item.relative,
                    content_hash: hash_path(&item.target),
                    installed_at: time.clone(),
                    updated_at: time,
                    resolved_commit: revision.into(),
                },
            );
        }
        app.save()?;
        if FAIL_AFTER_STATE_SAVE.with(Cell::get) {
            bail!("隔离测试注入安装失败");
        }
        app.write_lock(&lock)
    })();
    if let Err(error) = result {
        app.state = old;
        let mut rollback_failed = false;
        for target in targets.iter_mut().rev() {
            rollback_failed |= target.rollback().is_err();
        }
        if created_parent {
            // 只删自己创建且已空的父目录，不递归删除外部并发出现的资源。
            let _ = fs::remove_dir(&skills);
        }
        rollback_failed |= journal.rollback().is_err();
        if rollback_failed {
            bail!("安装失败，回滚未全部完成，请检查隔离目录权限");
        }
        return Err(error);
    }
    for target in &mut targets {
        target.commit();
    }
    journal.commit();
    Ok(())
}
