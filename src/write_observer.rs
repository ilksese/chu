//! 隔离回归可在真实原子写入后设置屏障，不修改全局环境或资源路径。
use anyhow::Result;
use std::{cell::RefCell, path::Path, sync::Arc};
pub type WriteObserver = Arc<dyn Fn(&Path) -> Result<()> + Send + Sync>;
thread_local! {
    static OBSERVER: RefCell<Option<WriteObserver>> = const { RefCell::new(None) };
}
struct Scope(Option<WriteObserver>);
impl Drop for Scope {
    fn drop(&mut self) {
        OBSERVER.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
#[doc(hidden)]
pub fn with_write_observer<T>(observer: WriteObserver, run: impl FnOnce() -> T) -> T {
    with_optional_observer(Some(observer), run)
}
pub(crate) fn with_optional_observer<T>(
    observer: Option<WriteObserver>,
    run: impl FnOnce() -> T,
) -> T {
    let _scope = Scope(OBSERVER.with(|slot| slot.replace(observer)));
    run()
}
pub(crate) fn current() -> Option<WriteObserver> {
    OBSERVER.with(|slot| slot.borrow().clone())
}
pub(crate) fn after_write(path: &Path) -> Result<()> {
    current().map_or(Ok(()), |observer| observer(path))
}
