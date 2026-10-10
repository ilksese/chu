//! 界面中的活跃任务只保存安全元数据，不保留含凭证的请求参数。
use crate::{
    operation::{Operation, Phase},
    worker::{Completion, Origin},
};
use std::collections::BTreeMap;

pub(crate) struct Pending {
    pub completion: Completion,
    pub origin: Origin,
    pub operation: Operation,
}
#[derive(Default)]
pub(crate) struct Tasks {
    next: u64,
    pub pending: BTreeMap<u64, Pending>,
    pub close_after_cancel: Option<u64>,
}
impl Tasks {
    pub fn allocate(&mut self, pending: Pending) -> u64 {
        self.next += 1;
        self.pending.insert(self.next, pending);
        self.next
    }
    pub fn blocking(&self) -> bool {
        self.pending.values().any(|p| p.completion.blocking())
    }
    pub fn active(&self) -> Option<&Pending> {
        self.pending.values().find(|p| p.completion.blocking())
    }
    pub fn dialog_pending(&self, dialog: u64) -> Option<&Pending> {
        self.pending
            .values()
            .find(|p| p.origin.dialog == dialog && p.completion.dialog_result())
    }
    pub fn detail_pending(&self, page: i32, id: &str) -> bool {
        self.pending.values().any(|p| matches!(&p.completion, Completion::Detail { page: p, id: i } if *p == page && i == id))
    }
    pub fn can_cancel(p: &Pending) -> bool {
        !matches!(p.operation.phase(), Phase::Commit | Phase::Finished)
    }
}
