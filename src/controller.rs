//! One serialized background worker owns filesystem/network state; GUI callbacks never block on IO.
use crate::{
    App,
    editor::Dialog,
    models::*,
    presenter::{self, ViewState},
    ui::{AppWindow, CandidateRow},
};
use anyhow::Result;
use serde_json::{Value, json};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};
#[derive(Clone)]
pub(crate) enum Completion {
    Snapshot { close: bool },
    Detail { page: i32, id: String },
    Edit { page: i32, id: String },
    Preview,
    ImportScan,
    Updates,
}
impl Completion {
    fn blocking(&self) -> bool {
        !matches!(self, Self::Detail { .. })
    }
}
struct Work {
    method: String,
    args: Value,
    completion: Completion,
}
struct Reply {
    completion: Completion,
    value: Option<Value>,
    snapshot: Option<Snapshot>,
    hidden: Vec<String>,
    error: Option<String>,
    method: String,
}
pub struct Controller {
    pub(crate) window: slint::Weak<AppWindow>,
    pub(crate) state: RefCell<ViewState>,
    pub(crate) dialog: RefCell<Option<Dialog>>,
    sender: Sender<Work>,
    receiver: RefCell<Receiver<Reply>>,
    timer: Timer,
}
impl Controller {
    pub fn attach(ui: &AppWindow, mut app: App) -> Result<Rc<Self>> {
        let snapshot = app.snapshot()?;
        let hidden = app.preferences.hidden_hosts.clone();
        let (sender, requests) = mpsc::channel::<Work>();
        let (replies, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("chu-resource-worker".into())
            .spawn(move || {
                for work in requests {
                    let result = app.execute(&work.method, work.args);
                    let error = result.as_ref().err().map(|e| e.to_string());
                    let snapshot = app.snapshot().ok();
                    let reply = Reply {
                        completion: work.completion,
                        value: result.ok(),
                        snapshot,
                        hidden: app.preferences.hidden_hosts.clone(),
                        error,
                        method: work.method,
                    };
                    if replies.send(reply).is_err() {
                        break;
                    }
                }
            })?;
        let controller = Rc::new(Self {
            window: ui.as_weak(),
            state: RefCell::new(ViewState {
                snapshot,
                hidden,
                ..ViewState::default()
            }),
            dialog: RefCell::new(None),
            sender,
            receiver: RefCell::new(receiver),
            timer: Timer::default(),
        });
        presenter::render(ui, &mut controller.state.borrow_mut());
        crate::callbacks::wire(ui, &controller);
        let weak = Rc::downgrade(&controller);
        controller
            .timer
            .start(TimerMode::Repeated, Duration::from_millis(40), move || {
                if let Some(c) = weak.upgrade() {
                    c.poll();
                }
            });
        Ok(controller)
    }
    pub(crate) fn post(&self, method: &str, args: Value, completion: Completion) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if completion.blocking() {
            if ui.get_busy() {
                return;
            }
            ui.set_busy(true);
            ui.set_notice("".into());
            ui.set_dialog_error("".into());
        }
        if self
            .sender
            .send(Work {
                method: method.into(),
                args,
                completion,
            })
            .is_err()
        {
            ui.set_busy(false);
            self.notice("后台线程已停止", true);
        }
    }
    pub(crate) fn notice(&self, text: &str, error: bool) {
        if let Some(ui) = self.window.upgrade() {
            ui.set_notice(text.into());
            ui.set_notice_error(error);
        }
    }
    pub(crate) fn render(&self) {
        if let Some(ui) = self.window.upgrade() {
            presenter::render(&ui, &mut self.state.borrow_mut());
        }
    }
    pub fn navigate(&self, page: i32) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_busy() || ui.get_dialog_open() {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            s.page = page.clamp(0, 8);
            s.query.clear();
            s.selected.clear();
        }
        ui.set_query("".into());
        self.render();
        self.load_detail();
    }
    pub(crate) fn load_detail(&self) {
        let state = self.state.borrow();
        if state.selected.is_empty()
            || state
                .details
                .contains_key(&presenter::detail_key(state.page, &state.selected))
        {
            return;
        }
        let method = match state.page {
            2 => "ReadSkill",
            5 => "ReadAgent",
            6 => "ReadPrompt",
            7 => "ReadReference",
            _ => return,
        };
        self.post(
            method,
            json!([state.selected]),
            Completion::Detail {
                page: state.page,
                id: state.selected.clone(),
            },
        );
    }
    pub(crate) fn open(&self, method: &str, id: &str, args: Value) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let dialog = crate::editor::open(&ui, &self.state.borrow(), method, id, args);
        *self.dialog.borrow_mut() = Some(dialog);
        if matches!(method, "UpdatePrompt" | "UpdateReference") {
            let page = self.state.borrow().page;
            if !self
                .state
                .borrow()
                .details
                .contains_key(&presenter::detail_key(page, id))
            {
                let read = if method == "UpdatePrompt" {
                    "ReadPrompt"
                } else {
                    "ReadReference"
                };
                self.post(
                    read,
                    json!([id]),
                    Completion::Edit {
                        page,
                        id: id.into(),
                    },
                );
            }
        }
    }
    pub(crate) fn close_dialog(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_busy() {
            return;
        }
        *self.dialog.borrow_mut() = None;
        ui.set_dialog_open(false);
        ui.set_fields(presenter::model(Vec::new()));
        ui.set_candidates(presenter::model(Vec::new()));
        ui.set_import_rows(presenter::model(Vec::new()));
        ui.set_apply_hosts(presenter::model(Vec::new()));
        ui.set_dialog_importer(false);
    }
    fn poll(&self) {
        while let Ok(reply) = self.receiver.borrow_mut().try_recv() {
            let Some(ui) = self.window.upgrade() else {
                return;
            };
            if reply.completion.blocking() {
                ui.set_busy(false);
            }
            if let Some(snapshot) = reply.snapshot {
                let mut s = self.state.borrow_mut();
                let ids: Vec<String> = match reply.method.as_str() {
                    "CreateProvider" => snapshot
                        .providers
                        .iter()
                        .filter(|r| !s.snapshot.providers.iter().any(|old| old.id == r.id))
                        .map(|r| r.id.clone())
                        .collect(),
                    "CreatePrompt" | "UpdatePrompt" => snapshot
                        .prompts
                        .iter()
                        .filter(|r| !s.snapshot.prompts.iter().any(|old| old.id == r.id))
                        .map(|r| r.id.clone())
                        .collect(),
                    "CreateReference" | "UpdateReference" => snapshot
                        .references
                        .iter()
                        .filter(|r| !s.snapshot.references.iter().any(|old| old.id == r.id))
                        .map(|r| r.id.clone())
                        .collect(),
                    "AddAgent" => snapshot
                        .agents
                        .iter()
                        .filter(|r| !s.snapshot.agents.iter().any(|old| old.id == r.id))
                        .map(|r| r.id.clone())
                        .collect(),
                    "ImportHostResources" => {
                        if s.page == 2 {
                            snapshot
                                .skills
                                .iter()
                                .filter(|r| {
                                    r.managed
                                        && !s
                                            .snapshot
                                            .skills
                                            .iter()
                                            .any(|old| old.managed && old.id == r.id)
                                })
                                .map(|r| r.id.clone())
                                .collect()
                        } else {
                            snapshot
                                .mcps
                                .iter()
                                .filter(|r| {
                                    r.managed
                                        && !s
                                            .snapshot
                                            .mcps
                                            .iter()
                                            .any(|old| old.managed && old.id == r.id)
                                })
                                .map(|r| r.id.clone())
                                .collect()
                        }
                    }
                    "AddMCP" => snapshot
                        .mcps
                        .iter()
                        .filter(|r| !s.snapshot.mcps.iter().any(|old| old.id == r.id))
                        .map(|r| r.id.clone())
                        .collect(),
                    _ => Vec::new(),
                };
                if let Some(id) = ids.first() {
                    s.selected = id.clone();
                }
                s.snapshot = snapshot;
                s.hidden = reply.hidden;
            }
            if let Some(error) = reply.error {
                if matches!(reply.completion, Completion::Detail { .. }) {
                    self.notice(&format!("读取资源失败: {error}"), true);
                } else {
                    self.notice(&error, true);
                    ui.set_dialog_error(error.into());
                }
                self.render();
                continue;
            }
            let value = reply.value.unwrap_or(Value::Null);
            let should_load = reply.completion.blocking();
            match reply.completion {
                Completion::Detail { page, ref id } => {
                    if let Some(content) = value.as_str() {
                        self.state
                            .borrow_mut()
                            .details
                            .insert(presenter::detail_key(page, id), content.into());
                    }
                }
                Completion::Edit { page, id } => {
                    if let Some(content) = value.as_str() {
                        self.state
                            .borrow_mut()
                            .details
                            .insert(presenter::detail_key(page, &id), content.into());
                        use slint::Model;
                        let mut fields: Vec<_> = ui.get_fields().iter().collect();
                        for field in &mut fields {
                            if field.key == "content" {
                                field.value = content.into();
                            }
                        }
                        ui.set_fields(presenter::model(fields));
                    }
                }
                Completion::ImportScan => self.show_import_scan(value),
                Completion::Preview => {
                    let candidates: Vec<SkillCandidate> =
                        serde_json::from_value(value).unwrap_or_default();
                    let empty = candidates.is_empty();
                    ui.set_candidates(presenter::model(
                        candidates
                            .into_iter()
                            .map(|c| CandidateRow {
                                name: c.name.into(),
                                description: c.description.into(),
                                path: c.path.into(),
                                checked: !c.installed,
                                installed: c.installed,
                            })
                            .collect(),
                    ));
                    if empty {
                        ui.set_dialog_error("仓库中没有可安装的 Skill。".into());
                    }
                }
                Completion::Updates => {
                    let updates: Vec<SkillUpdate> =
                        serde_json::from_value(value).unwrap_or_default();
                    self.state.borrow_mut().updates = updates
                        .iter()
                        .map(|u| {
                            (
                                u.id.clone(),
                                match u.status.as_str() {
                                    "update" => "可更新",
                                    "deleted" => "来源已删除",
                                    _ => "名称存在歧义",
                                }
                                .into(),
                            )
                        })
                        .collect();
                    self.notice(
                        &format!("更新检查完成，{} 项需要处理。", updates.len()),
                        false,
                    );
                }
                Completion::Snapshot { close } => {
                    self.state.borrow_mut().details.clear();
                    if close {
                        self.close_dialog();
                    }
                    let retained: Vec<String> =
                        serde_json::from_value(value.get("retained").cloned().unwrap_or(json!([])))
                            .unwrap_or_default();
                    if !retained.is_empty() {
                        self.notice(
                            &format!("项目已移除，保留本地修改：\n{}", retained.join("\n")),
                            false,
                        );
                    } else if let Some(message) = value.as_str() {
                        self.notice(message, false);
                    } else if reply.method == "ImportHostResources" {
                        self.notice("导入已完成，已应用到所勾选的宿主。", false);
                    } else if reply.method.contains("Provider") {
                        self.notice("供应商配置已保存；模型目录状态见详情。", false);
                    } else {
                        self.notice("操作已完成。", false);
                    }
                }
            }
            self.render();
            if should_load {
                self.load_detail();
            }
        }
    }
}
