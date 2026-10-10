//! 一个串行 worker 独占资源状态；GUI 只调度任务并消费安全投影。
pub(crate) use crate::worker::Completion;
use crate::{
    App,
    editor::Dialog,
    feedback::{Feedback, Severity},
    operation::{Operation, Phase},
    presenter::{self, ViewState},
    session::DetailLoad,
    tasks::{Pending, Tasks},
    ui::AppWindow,
    worker::{self, Origin, Reply, Work},
    worker_handle::Worker,
};
use anyhow::Result;
use serde_json::{Value, json};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::mpsc::Receiver,
    time::Duration,
};

pub struct Controller {
    pub(crate) window: slint::Weak<AppWindow>,
    pub(crate) state: RefCell<ViewState>,
    pub(crate) dialog: RefCell<Option<Dialog>>,
    pub(crate) import_state: RefCell<crate::import_state::ImportState>,
    pub(crate) tasks: RefCell<Tasks>,
    pub(crate) next_dialog: Cell<u64>,
    pub(crate) worker: RefCell<Worker>,
    pub(crate) closing: Cell<bool>,
    pub(crate) closed: Cell<bool>,
    receiver: RefCell<Receiver<Reply>>,
    pub(crate) timer: Timer,
}
impl Controller {
    pub fn attach(ui: &AppWindow, mut app: App) -> Result<Rc<Self>> {
        let snapshot = app.snapshot()?;
        let hidden = app.preferences.hidden_hosts.clone();
        let (worker, receiver) = worker::spawn(app)?;
        let c = Rc::new(Self {
            window: ui.as_weak(),
            state: RefCell::new(ViewState {
                snapshot,
                hidden,
                ..ViewState::default()
            }),
            dialog: RefCell::new(None),
            import_state: RefCell::new(crate::import_state::ImportState::default()),
            tasks: RefCell::new(Tasks::default()),
            next_dialog: Cell::new(0),
            worker: RefCell::new(worker),
            closing: Cell::new(false),
            closed: Cell::new(false),
            receiver: RefCell::new(receiver),
            timer: Timer::default(),
        });
        c.render();
        crate::callbacks::wire(ui, &c);
        let weak = Rc::downgrade(&c);
        ui.window().on_close_requested(move || {
            if let Some(c) = weak.upgrade() {
                c.request_shutdown();
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                slint::CloseRequestResponse::HideWindow
            }
        });
        let weak = Rc::downgrade(&c);
        c.timer
            .start(TimerMode::Repeated, Duration::from_millis(40), move || {
                if let Some(c) = weak.upgrade() {
                    c.poll();
                }
            });
        Ok(c)
    }
    pub(crate) fn post(&self, method: &str, args: Value, completion: Completion) {
        if self.closing.get() {
            return;
        }
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if completion.blocking() && self.tasks.borrow().blocking() {
            self.notice(
                "当前任务尚未完成；可以浏览资源，完成后再执行新的操作。",
                false,
            );
            return;
        }
        let origin = Origin {
            page: self.state.borrow().page,
            selection: self.state.borrow().session.selected_revision,
            dialog: self.dialog.borrow().as_ref().map_or(0, |d| d.generation),
        };
        let operation = Operation::new();
        operation.set_phase(crate::feedback::label(method), Phase::Preparing);
        let job = self.tasks.borrow_mut().allocate(Pending {
            completion: completion.clone(),
            origin: origin.clone(),
            operation: operation.clone(),
        });
        if completion.blocking() {
            ui.set_notice("".into());
        }
        if completion.dialog_result() {
            ui.set_dialog_error("".into());
        }
        if self
            .worker
            .borrow()
            .send(Work {
                job,
                method: method.into(),
                args,
                completion,
                origin,
                operation,
            })
            .is_err()
        {
            self.tasks.borrow_mut().pending.remove(&job);
            self.notice("后台线程已停止。请重新启动应用。", true);
        }
        self.update_task_ui();
    }
    pub(crate) fn notice(&self, text: &str, error: bool) {
        self.show_feedback(&Feedback {
            text: text.into(),
            severity: if error {
                Severity::Error
            } else {
                Severity::Success
            },
        });
    }
    pub(crate) fn show_feedback(&self, feedback: &Feedback) {
        if let Some(ui) = self.window.upgrade() {
            ui.set_notice(feedback.text.clone().into());
            ui.set_notice_error(feedback.severity == Severity::Error);
            ui.set_notice_warning(matches!(
                feedback.severity,
                Severity::Warning | Severity::Cancelled
            ));
        }
    }
    pub(crate) fn render(&self) {
        if let Some(ui) = self.window.upgrade() {
            presenter::render(&ui, &mut self.state.borrow_mut());
            self.update_task_ui();
        }
    }
    fn save_position(&self, ui: &AppWindow) {
        let mut s = self.state.borrow_mut();
        let page = s.page;
        let query = s.query.clone();
        let pos = s.session.pages.entry(page).or_default();
        pos.query = query;
        pos.list_scroll = ui.get_list_scroll_y();
        let key = presenter::detail_key(page, &s.selected);
        let pos = s.session.details.entry(key).or_default();
        pos.scroll = ui.get_detail_scroll_y();
        pos.source = ui.get_markdown_source();
    }
    pub fn navigate(&self, page: i32) {
        self.navigate_target(page, "", "");
    }
    pub(crate) fn navigate_target(&self, page: i32, id: &str, host: &str) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_dialog_open() {
            return;
        }
        if self.state.borrow().page == page.clamp(0, 8) && id.is_empty() && host.is_empty() {
            return;
        }
        self.save_position(&ui);
        {
            let mut s = self.state.borrow_mut();
            let page = page.clamp(0, 8);
            let pos = s.session.pages.get(&page).cloned().unwrap_or_default();
            s.page = page;
            s.query = if id.is_empty() {
                pos.query
            } else {
                String::new()
            };
            s.selected = if id.is_empty() {
                pos.remembered
            } else {
                id.into()
            };
            if !id.is_empty() {
                s.session.remember_selection(page, id);
            }
            if !host.is_empty() {
                s.session.settings_host = host.into();
            }
        }
        let s = self.state.borrow();
        ui.set_query(s.query.clone().into());
        ui.set_list_scroll_y(s.session.pages.get(&s.page).map_or(0., |p| p.list_scroll));
        drop(s);
        self.render();
        self.restore_detail_position(&ui);
        self.load_detail();
    }
    pub(crate) fn search(&self, query: &str) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_dialog_open() {
            return;
        }
        self.save_position(&ui);
        let previous = self.state.borrow().selected.clone();
        {
            let mut state = self.state.borrow_mut();
            state.query = query.into();
            let page = state.page;
            state.session.pages.entry(page).or_default().query = query.into();
        }
        self.render();
        if self.state.borrow().selected != previous {
            self.restore_detail_position(&ui);
        }
        self.load_detail();
    }
    pub(crate) fn select(&self, id: &str) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_dialog_open() {
            return;
        }
        self.save_position(&ui);
        let mut s = self.state.borrow_mut();
        let page = s.page;
        s.selected = id.into();
        s.session.remember_selection(page, id);
        drop(s);
        self.render();
        self.restore_detail_position(&ui);
        self.load_detail();
    }
    fn restore_detail_position(&self, ui: &AppWindow) {
        let s = self.state.borrow();
        let key = presenter::detail_key(s.page, &s.selected);
        let pos = s.session.details.get(&key).cloned().unwrap_or_default();
        ui.set_detail_scroll_y(pos.scroll);
        ui.set_markdown_source(pos.source);
    }
    pub(crate) fn select_relative(&self, delta: i32) {
        let s = self.state.borrow();
        let rows = crate::search::rows(&s);
        if rows.is_empty() {
            return;
        }
        let current = rows
            .iter()
            .position(|r| r.id.as_str() == s.selected)
            .unwrap_or(0) as i32;
        let index = (current + delta).clamp(0, rows.len() as i32 - 1);
        let id = rows[index as usize].id.to_string();
        drop(s);
        self.select(&id);
    }
    pub(crate) fn select_index(&self, index: i32) {
        let s = self.state.borrow();
        let rows = crate::search::rows(&s);
        let index = if index < 0 {
            rows.len().saturating_sub(1)
        } else {
            index as usize
        };
        let id = rows.get(index).map(|r| r.id.to_string());
        drop(s);
        if let Some(id) = id {
            self.select(&id);
        }
    }
    pub(crate) fn load_detail(&self) {
        let (page, id, key, method) = {
            let s = self.state.borrow();
            let method = match s.page {
                2 => "ReadSkill",
                5 => "ReadAgent",
                6 => "ReadPrompt",
                7 => "ReadReference",
                _ => return,
            };
            let key = presenter::detail_key(s.page, &s.selected);
            if s.selected.is_empty()
                || s.details.contains_key(&key)
                || matches!(
                    s.session.loads.get(&key),
                    Some(DetailLoad::Loading | DetailLoad::Failed(_))
                )
            {
                return;
            }
            (s.page, s.selected.clone(), key, method)
        };
        if self.tasks.borrow().detail_pending(page, &id) {
            return;
        }
        self.state
            .borrow_mut()
            .session
            .loads
            .insert(key, DetailLoad::Loading);
        self.post(method, json!([id]), Completion::Detail { page, id });
        self.render();
    }
    pub(crate) fn retry_detail(&self) {
        let mut s = self.state.borrow_mut();
        let key = presenter::detail_key(s.page, &s.selected);
        s.details.remove(&key);
        s.session.loads.remove(&key);
        drop(s);
        self.load_detail();
    }
    pub(crate) fn cancel_task(&self) {
        if let Some(task) = self.tasks.borrow().active() {
            if task.operation.cancel() {
                self.notice("正在安全取消任务，等待后台清理完成…", false);
            } else {
                self.notice("正在提交或回滚资源，暂时不能取消。", false);
            }
        }
        self.update_task_ui();
    }
    pub(crate) fn update_task_ui(&self) {
        if self.closing.get() {
            self.update_shutdown_ui();
            return;
        }
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let tasks = self.tasks.borrow();
        ui.set_busy(tasks.blocking());
        if let Some(p) = tasks.active() {
            ui.set_task_label(p.operation.label().into());
            ui.set_task_can_cancel(Tasks::can_cancel(p));
            ui.set_task_phase(if p.operation.phase() == Phase::Commit {
                2
            } else {
                1
            });
        } else {
            ui.set_task_label("".into());
            ui.set_task_can_cancel(false);
            ui.set_task_phase(0);
        }
        let generation = self.dialog.borrow().as_ref().map_or(0, |d| d.generation);
        let dialog_task = tasks.dialog_pending(generation);
        ui.set_dialog_busy(dialog_task.is_some());
        ui.set_dialog_can_cancel(dialog_task.is_none_or(Tasks::can_cancel));
        drop(tasks);
        self.sync_dialog_capability();
    }
    fn poll(&self) {
        loop {
            let reply = self.receiver.borrow_mut().try_recv();
            let Ok(reply) = reply else {
                break;
            };
            self.tasks.borrow_mut().pending.remove(&reply.job);
            if !self.closing.get() {
                self.apply_reply(reply);
            }
        }
        if self.closing.get() {
            self.poll_shutdown();
        } else {
            self.update_task_ui();
        }
    }
    pub(crate) fn release_preview(&self, token: &str) {
        if !token.is_empty() {
            self.post(
                "DropSkillPreview",
                json!([token]),
                Completion::ReleasePreview,
            );
        }
    }
}
