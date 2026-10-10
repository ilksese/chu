//! 请求通道与线程拥有同一个生命周期，关闭时先取消准备任务，再等待提交结束。
use crate::{operation::Operation, worker::Work};
use anyhow::{Result, bail};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc::Sender},
    thread::JoinHandle,
};
pub(crate) type Operations = Arc<Mutex<BTreeMap<u64, Operation>>>;
#[cfg(test)]
#[path = "worker_handle_tests.rs"]
mod tests;
pub(crate) struct Worker {
    sender: Option<Sender<Work>>,
    thread: Option<JoinHandle<()>>,
    operations: Operations,
    failed: bool,
}
impl Worker {
    pub fn new(sender: Sender<Work>, thread: JoinHandle<()>, operations: Operations) -> Self {
        Self {
            sender: Some(sender),
            thread: Some(thread),
            operations,
            failed: false,
        }
    }
    pub fn send(&self, work: Work) -> Result<()> {
        let Some(sender) = &self.sender else {
            bail!("后台线程正在关闭");
        };
        let job = work.job;
        self.operations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(job, work.operation.clone());
        if sender.send(work).is_err() {
            self.operations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&job);
            bail!("后台线程已停止");
        }
        Ok(())
    }
    pub fn begin_shutdown(&mut self) {
        for operation in self
            .operations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            // Operation 在同一门闩内裁决取消与提交，已开始的提交或回滚不会被打断。
            operation.cancel();
        }
        self.sender.take();
    }
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn join(&mut self) -> Result<()> {
        self.begin_shutdown();
        if let Some(thread) = self.thread.take() {
            self.failed |= thread.join().is_err();
        }
        if self.failed {
            bail!("后台线程异常结束，请检查资源状态");
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        // 事件循环错误返回或 Controller 提前释放也不能让线程脱离进程生命周期。
        let _ = self.join();
    }
}
