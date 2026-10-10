//! 与工作线程共享的任务取消边界，不持有应用状态。
use anyhow::{Result, bail};
use std::{
    cell::RefCell,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Preparing,
    Network,
    Commit,
    Finished,
}

#[derive(Debug)]
struct State {
    phase: Phase,
    label: String,
    cancelled: bool,
    deadline: Option<Instant>,
    timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct Operation {
    inner: Arc<(Mutex<State>, Condvar)>,
}

impl Default for Operation {
    fn default() -> Self {
        Self::new()
    }
}

impl Operation {
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(120))
    }

    /// 隔离后端测试可缩短预算；正式任务使用 `new()`。
    #[doc(hidden)]
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(State {
                    phase: Phase::Preparing,
                    label: "正在准备".into(),
                    cancelled: false,
                    deadline: None,
                    timeout,
                }),
                Condvar::new(),
            )),
        }
    }

    /// 隔离测试在阶段屏障处缩短已启动的网络时限；不得延长正式任务预算。
    #[doc(hidden)]
    pub fn shorten_network_deadline(&self, remaining: Duration) -> bool {
        let mut state = self.state();
        if matches!(state.phase, Phase::Commit | Phase::Finished) {
            return false;
        }
        let Some(deadline) = state.deadline else {
            return false;
        };
        state.deadline = Some(deadline.min(Instant::now() + remaining));
        self.inner.1.notify_all();
        true
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn phase(&self) -> Phase {
        self.state().phase
    }

    pub fn label(&self) -> String {
        self.state().label.clone()
    }

    pub fn is_cancelled(&self) -> bool {
        self.state().cancelled
    }

    /// 取消和进入提交必须在同一把锁下裁决。
    pub fn cancel(&self) -> bool {
        let mut state = self.state();
        if matches!(state.phase, Phase::Commit | Phase::Finished) {
            return false;
        }
        state.cancelled = true;
        state.label = "正在取消".into();
        self.inner.1.notify_all();
        true
    }

    fn check_state(state: &State) -> Result<()> {
        if state.cancelled {
            bail!("操作已取消");
        }
        if !matches!(state.phase, Phase::Commit | Phase::Finished)
            && state.deadline.is_some_and(|d| Instant::now() >= d)
        {
            bail!("Git 网络操作超时，请稍后重试");
        }
        Ok(())
    }

    pub fn check(&self) -> Result<()> {
        Self::check_state(&self.state())
    }

    pub fn begin_commit(&self) -> Result<()> {
        let mut state = self.state();
        Self::check_state(&state)?;
        if state.phase == Phase::Finished {
            bail!("操作已经结束");
        }
        state.phase = Phase::Commit;
        state.label = "正在提交更改，暂不能取消".into();
        Ok(())
    }

    pub fn set_phase(&self, label: impl Into<String>, phase: Phase) {
        let mut state = self.state();
        // 提交不可逆；进度更新不得重新开放取消。
        if matches!(state.phase, Phase::Commit | Phase::Finished) || state.cancelled {
            return;
        }
        if phase == Phase::Network && state.deadline.is_none() {
            state.deadline = Instant::now().checked_add(state.timeout);
        }
        // 只有 begin_commit() 可以越过原子的取消/提交边界。
        if phase != Phase::Commit {
            state.phase = phase;
            state.label = label.into();
        }
    }

    /// 只读准备完成也必须原子裁决取消，防止迟到取消遗留预览令牌。
    pub(crate) fn finish_preparation(&self) -> Result<()> {
        let mut state = self.state();
        Self::check_state(&state)?;
        state.phase = Phase::Finished;
        state.label = "准备完成".into();
        self.inner.1.notify_all();
        Ok(())
    }

    pub fn finish(&self) {
        let mut state = self.state();
        state.phase = Phase::Finished;
        if state.cancelled {
            state.label = "已取消".into();
        }
        self.inner.1.notify_all();
    }

    pub(crate) fn wait(&self, duration: Duration) {
        let state = self.state();
        if state.cancelled {
            return;
        }
        let duration = state
            .deadline
            .map(|d| duration.min(d.saturating_duration_since(Instant::now())))
            .unwrap_or(duration);
        drop(self.inner.1.wait_timeout(state, duration));
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Operation>> = const { RefCell::new(None) };
}

struct Scope(Option<Operation>);
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

pub fn with_operation<T>(operation: &Operation, f: impl FnOnce() -> T) -> T {
    let _scope = Scope(CURRENT.with(|slot| slot.replace(Some(operation.clone()))));
    f()
}

pub fn current() -> Option<Operation> {
    CURRENT.with(|slot| slot.borrow().clone())
}

/// 原有同步/CLI 资源操作在无任务上下文时保持原行为。
pub fn check() -> Result<()> {
    current().map_or(Ok(()), |op| op.check())
}

pub fn begin_commit() -> Result<()> {
    current().map_or(Ok(()), |op| op.begin_commit())
}

pub(crate) fn progress(label: &str, phase: Phase) {
    if let Some(op) = current() {
        op.set_phase(label, phase);
    }
}
