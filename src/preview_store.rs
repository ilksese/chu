//! App 共同持有预览快照；全局只保存域和令牌的弱索引，不延长临时目录生命周期。
use crate::{skill_sources::PreparedCheckout, sources::Source};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock, Weak},
};

pub(crate) struct Preview {
    pub source: Source,
    pub checkout: PreparedCheckout,
    pub fingerprint: String,
    pub candidates: BTreeMap<String, String>,
}

#[derive(Default)]
pub(crate) struct PreviewStore {
    // 安装暂取快照时保留空槽；并发关闭删除空槽，失败恢复不能复活已撤销令牌。
    entries: BTreeMap<String, Option<Preview>>,
}

type Store = Arc<Mutex<PreviewStore>>;
type WeakStore = Weak<Mutex<PreviewStore>>;

#[derive(Default)]
struct Registry {
    domains: BTreeMap<(PathBuf, PathBuf), WeakStore>,
    tokens: BTreeMap<String, WeakStore>,
}

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> Result<MutexGuard<'static, Registry>> {
    REGISTRY
        .get_or_init(|| Mutex::new(Registry::default()))
        .lock()
        .map_err(|_| anyhow::anyhow!("预览状态不可用，请重新预览"))
}

fn lock_store(store: &Store) -> Result<MutexGuard<'_, PreviewStore>> {
    store
        .lock()
        .map_err(|_| anyhow::anyhow!("预览状态不可用，请重新预览"))
}

// 尚未创建 root 时也用现有祖先的规范路径作为域，不创建目录或扫描来源缓存。
fn domain_path(path: &Path) -> Result<PathBuf> {
    let mut ancestor = std::path::absolute(path)?;
    let mut suffix = Vec::new();
    let mut normalized = loop {
        match fs::canonicalize(&ancestor) {
            Ok(path) => break path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let component = ancestor
                    .components()
                    .next_back()
                    .context("预览域路径不可用")?;
                match component {
                    Component::Normal(part) => suffix.push(part.to_os_string()),
                    Component::ParentDir => suffix.push("..".into()),
                    Component::CurDir => {}
                    _ => return Err(error.into()),
                }
                if !ancestor.pop() {
                    return Err(error.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
    };
    for part in suffix.into_iter().rev() {
        if part == ".." {
            normalized.pop();
        } else {
            normalized.push(part);
        }
    }
    Ok(normalized)
}

pub(crate) fn new_store(root: &Path, home: &Path) -> Result<Store> {
    // 路径解析可能访问磁盘，必须在取得全局锁之前完成。
    let domain = (domain_path(root)?, domain_path(home)?);
    let mut registry = registry()?;
    registry.domains.retain(|_, owner| owner.strong_count() > 0);
    registry.tokens.retain(|_, owner| owner.strong_count() > 0);
    if let Some(store) = registry.domains.get(&domain).and_then(Weak::upgrade) {
        return Ok(store);
    }
    let store = Arc::new(Mutex::new(PreviewStore::default()));
    registry.domains.insert(domain, Arc::downgrade(&store));
    Ok(store)
}

fn check_owner(registry: &Registry, store: &Store, token: &str) -> Result<()> {
    if let Some(owner) = registry.tokens.get(token)
        && owner.strong_count() > 0
        && !owner.ptr_eq(&Arc::downgrade(store))
    {
        bail!("预览不属于当前应用");
    }
    Ok(())
}

pub(crate) fn register(store: &Store, token: String, preview: Preview) -> Result<()> {
    let mut registry = registry()?;
    let mut previews = lock_store(store)?;
    if registry.tokens.contains_key(&token) || previews.entries.contains_key(&token) {
        bail!("预览令牌冲突，请重新预览");
    }
    registry.tokens.insert(token.clone(), Arc::downgrade(store));
    previews.entries.insert(token, Some(preview));
    Ok(())
}

pub(crate) fn take(store: &Store, token: &str) -> Result<Preview> {
    let registry = registry()?;
    check_owner(&registry, store, token)?;
    let mut previews = lock_store(store)?;
    previews
        .entries
        .get_mut(token)
        .and_then(Option::take)
        .context("预览已失效，请重新预览来源")
}

pub(crate) fn finish(store: &Store, token: &str, preview: Option<Preview>) -> Result<()> {
    let mut registry = registry()?;
    let mut previews = lock_store(store)?;
    if let Some(slot) = previews.entries.get_mut(token) {
        if slot.is_some() {
            bail!("预览状态不可用，请重新预览");
        }
        *slot = preview;
        if slot.is_none() {
            previews.entries.remove(token);
            registry.tokens.remove(token);
        }
    }
    // 若空槽已撤销，传入的快照在两个锁释放后才 Drop 和清理临时目录。
    Ok(())
}

pub(crate) fn drop_preview(store: &Store, token: &str) -> Result<()> {
    let mut registry = registry()?;
    check_owner(&registry, store, token)?;
    let mut previews = lock_store(store)?;
    let removed = previews.entries.remove(token);
    registry.tokens.remove(token);
    drop(previews);
    drop(registry);
    // 仅 Drop 当前令牌拥有的 TempDir，不扫描或清理其他缓存。
    drop(removed);
    Ok(())
}
