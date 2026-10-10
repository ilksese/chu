//! 串行线程独占 App；请求身份和取消句柄与界面生命周期分离。
use crate::{
    App,
    feedback::{self, Feedback, Severity},
    models::Snapshot,
    operation::{self, Operation, Phase},
    worker_handle::{Operations, Worker},
};
use anyhow::Result;
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver},
};

#[derive(Clone)]
pub(crate) enum Completion {
    Snapshot { close: bool },
    Detail { page: i32, id: String },
    Edit { page: i32, id: String },
    Preview,
    ImportScan,
    Updates,
    ReleasePreview,
}
impl Completion {
    pub fn blocking(&self) -> bool {
        !matches!(self, Self::Detail { .. } | Self::ReleasePreview)
    }
    pub fn dialog_result(&self) -> bool {
        matches!(
            self,
            Self::Edit { .. } | Self::Preview | Self::ImportScan | Self::Snapshot { close: true }
        )
    }
}
#[derive(Clone, Default)]
pub(crate) struct Origin {
    pub page: i32,
    pub selection: u64,
    pub dialog: u64,
}
pub(crate) struct Work {
    pub job: u64,
    pub method: String,
    pub args: Value,
    pub completion: Completion,
    pub origin: Origin,
    pub operation: Operation,
}
pub(crate) struct Reply {
    pub job: u64,
    pub completion: Completion,
    pub origin: Origin,
    pub value: Option<Value>,
    pub snapshot: Option<Snapshot>,
    pub hidden: Vec<String>,
    pub error: Option<String>,
    pub feedback: Feedback,
    pub method: String,
}
pub(crate) fn read_only(method: &str) -> bool {
    method.starts_with("Read")
        || matches!(
            method,
            "PreviewSkills"
                | "PreviewSkillsBound"
                | "DropSkillPreview"
                | "ListImportCandidates"
                | "CheckSkillUpdates"
                | "TestMCP"
                | "TestMCPResult"
        )
}
fn prepares_before_commit(method: &str) -> bool {
    matches!(
        method,
        "InstallSkills"
            | "InstallSkillsFromPreview"
            | "UpdateSkill"
            | "UpdateMCP"
            | "DeleteMCP"
            | "UpdateAgent"
            | "DeleteAgent"
    )
}
pub(crate) fn spawn(mut app: App) -> Result<(Worker, Receiver<Reply>)> {
    let (sender, requests) = mpsc::channel::<Work>();
    let (replies, receiver) = mpsc::channel();
    let operations: Operations = Arc::new(Mutex::new(std::collections::BTreeMap::new()));
    let active = operations.clone();
    let observer = crate::write_observer::current();
    let thread = std::thread::Builder::new()
        .name("chu-resource-worker".into())
        .spawn(move || {
            crate::write_observer::with_optional_observer(observer, || {
                for work in requests {
                    let result = operation::with_operation(&work.operation, || {
                        work.operation.check()?;
                        if !read_only(&work.method) && !prepares_before_commit(&work.method) {
                            work.operation.begin_commit()?;
                        } else {
                            work.operation
                                .set_phase(feedback::label(&work.method), Phase::Preparing);
                        }
                        let value = app.execute(&work.method, work.args.clone())?;
                        if read_only(&work.method) {
                            work.operation.finish_preparation()?;
                        }
                        Ok::<_, anyhow::Error>(value)
                    });
                    let cancelled = work.operation.is_cancelled();
                    if cancelled
                        && work.method == "PreviewSkillsBound"
                        && let Some(token) = result
                            .as_ref()
                            .ok()
                            .and_then(|v| v.get("token"))
                            .and_then(Value::as_str)
                    {
                        let _ = app.drop_skill_preview(token);
                    }
                    let value = result.as_ref().ok();
                    let snapshot = if !read_only(&work.method) {
                        value.and_then(|v| {
                            serde_json::from_value::<Snapshot>(
                                v.get("snapshot").unwrap_or(v).clone(),
                            )
                            .ok()
                        })
                    } else {
                        None
                    };
                    let error = result.as_ref().err().map(|e| e.to_string());
                    let mut outcome = if cancelled {
                        Feedback::cancelled()
                    } else if let Some(error) = &error {
                        Feedback::error(error)
                    } else {
                        feedback::completed(
                            &work.method,
                            &work.args,
                            value.unwrap_or(&Value::Null),
                            snapshot.as_ref(),
                        )
                    };
                    if work.method == "TestMCPResult"
                        && !cancelled
                        && let Some(v) = value
                    {
                        let (text, severity) =
                            match v.get("status").and_then(Value::as_str).unwrap_or("") {
                                "commandAvailable" => (
                                    "启动命令可用；此检查不启动服务或验证 MCP 协议。".into(),
                                    Severity::Success,
                                ),
                                "commandMissing" => (
                                    "连接测试失败：找不到启动命令，请编辑配置或安装对应程序。"
                                        .into(),
                                    Severity::Error,
                                ),
                                "reachable" => (
                                    "远程地址可达；此检查不验证 MCP 协议握手。".into(),
                                    Severity::Success,
                                ),
                                "httpError" => (
                                    format!(
                                        "连接测试失败：HTTP {}。",
                                        v.get("httpStatus")
                                            .and_then(Value::as_u64)
                                            .map_or_else(|| "错误".into(), |s| s.to_string())
                                    ),
                                    Severity::Error,
                                ),
                                "connectionFailed" => (
                                    "连接测试失败，请检查地址、凭证及网络后重试。".into(),
                                    Severity::Error,
                                ),
                                _ => ("连接测试未返回有效结果。".into(), Severity::Error),
                            };
                        outcome.text = text;
                        outcome.severity = severity;
                    }
                    work.operation.finish();
                    let reply = Reply {
                        job: work.job,
                        completion: work.completion,
                        origin: work.origin,
                        value: result.ok(),
                        snapshot,
                        hidden: app.preferences.hidden_hosts.clone(),
                        error: if cancelled { None } else { error },
                        feedback: outcome,
                        method: work.method,
                    };
                    active
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&work.job);
                    if let Err(error) = replies.send(reply) {
                        if error.0.method == "PreviewSkillsBound"
                            && let Some(token) = error
                                .0
                                .value
                                .as_ref()
                                .and_then(|v| v.get("token"))
                                .and_then(Value::as_str)
                        {
                            let _ = app.drop_skill_preview(token);
                        }
                        break;
                    }
                }
            })
        })?;
    Ok((Worker::new(sender, thread, operations), receiver))
}
