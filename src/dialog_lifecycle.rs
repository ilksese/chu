//! 表单加载、放弃确认及安全关闭保持在同一个弹窗身份边界内。
use crate::{
    controller::{Completion, Controller},
    editor, presenter,
    resource_edit::{AgentEditView, McpEditView},
    tasks::Tasks,
    ui::AppWindow,
};
use serde_json::{Value, json};
use slint::Model;
impl Controller {
    pub(crate) fn open(&self, method: &str, id: &str, args: Value) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_dialog_open() {
            return;
        }
        let mut d = editor::open(&ui, &self.state.borrow(), method, id, args);
        self.next_dialog.set(self.next_dialog.get() + 1);
        d.generation = self.next_dialog.get();
        let page = match method {
            "UpdatePrompt" => 6,
            "UpdateReference" => 7,
            "UpdateAgent" | "DeleteAgent" => 5,
            "UpdateMCP" | "DeleteMCP" => 3,
            _ => self.state.borrow().page,
        };
        if matches!(method, "UpdatePrompt" | "UpdateReference") {
            d.loaded = self
                .state
                .borrow()
                .details
                .contains_key(&presenter::detail_key(page, id));
        }
        if matches!(method, "DeleteMCP" | "DeleteAgent") {
            d.loaded = false;
        }
        d.rebase(&ui);
        let loading = !d.loaded;
        *self.dialog.borrow_mut() = Some(d);
        ui.set_dialog_load_state(if loading { 1 } else { 0 });
        if loading {
            self.request_edit(page, id, method);
        }
        self.sync_dialog_capability();
    }
    fn request_edit(&self, page: i32, id: &str, method: &str) {
        let read = match method {
            "UpdatePrompt" => "ReadPrompt",
            "UpdateReference" => "ReadReference",
            "UpdateMCP" | "DeleteMCP" => "ReadMCPEdit",
            "UpdateAgent" | "DeleteAgent" => "ReadAgentEdit",
            _ => return,
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
    pub(crate) fn retry_dialog(&self) {
        let dialog = self.dialog.borrow().clone();
        let Some(d) = dialog else {
            return;
        };
        if self.tasks.borrow().blocking() {
            return;
        }
        if let Some(ui) = self.window.upgrade() {
            ui.set_dialog_load_state(1);
            ui.set_dialog_error("".into());
            self.request_edit(self.state.borrow().page, &d.id, &d.method);
        }
    }
    pub(crate) fn apply_edit(&self, ui: &AppWindow, page: i32, id: &str, value: Value) {
        let mut dialog = self.dialog.borrow_mut();
        let Some(d) = dialog.as_mut() else {
            return;
        };
        if d.id != id {
            return;
        }
        if matches!(d.method.as_str(), "UpdateMCP" | "DeleteMCP") {
            let Ok(view) = serde_json::from_value::<McpEditView>(value) else {
                ui.set_dialog_error("MCP 编辑数据无效，请重新读取。".into());
                ui.set_dialog_load_state(2);
                return;
            };
            d.version = Some(view.version.clone());
            if d.method == "UpdateMCP" {
                crate::editor_mcp::load(ui, &view);
            }
        } else if matches!(d.method.as_str(), "UpdateAgent" | "DeleteAgent") {
            let Ok(view) = serde_json::from_value::<AgentEditView>(value) else {
                ui.set_dialog_error("Agent 编辑数据无效，请重新读取。".into());
                ui.set_dialog_load_state(2);
                return;
            };
            d.version = Some(view.version);
            if d.method == "UpdateAgent" {
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                for field in &mut fields {
                    field.value = match field.key.as_str() {
                        "name" => &view.input.name,
                        "description" => &view.input.description,
                        "model" => &view.input.model,
                        "prompt" => &view.input.prompt,
                        _ => continue,
                    }
                    .clone()
                    .into();
                }
                ui.set_fields(presenter::model(fields));
            }
        } else if let Some(content) = value.as_str() {
            self.state
                .borrow_mut()
                .details
                .insert(presenter::detail_key(page, id), content.into());
            let mut fields: Vec<_> = ui.get_fields().iter().collect();
            for field in &mut fields {
                if field.key == "content" {
                    field.value = content.into();
                }
            }
            ui.set_fields(presenter::model(fields));
        } else {
            ui.set_dialog_error("原始内容读取结果无效，请重新读取。".into());
            ui.set_dialog_load_state(2);
            return;
        }
        d.loaded = true;
        d.rebase(ui);
        ui.set_dialog_load_state(0);
        drop(dialog);
        self.sync_dialog_capability();
    }
    pub(crate) fn close_dialog(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let generation = self.dialog.borrow().as_ref().map_or(0, |d| d.generation);
        {
            let tasks = self.tasks.borrow();
            if let Some(p) = tasks.dialog_pending(generation) {
                if Tasks::can_cancel(p) && p.operation.cancel() {
                    drop(tasks);
                    self.tasks.borrow_mut().close_after_cancel = Some(generation);
                    ui.set_dialog_error("正在取消任务，等待后台清理完成后关闭…".into());
                } else {
                    ui.set_dialog_error("资源正在提交或回滚，完成前不能关闭。".into());
                }
                return;
            }
        }
        if self.dialog.borrow().as_ref().is_some_and(|d| d.dirty(&ui)) {
            ui.set_dialog_discard_open(true);
            return;
        }
        self.dismiss_dialog();
    }
    pub(crate) fn dismiss_dialog(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let preview = self
            .dialog
            .borrow_mut()
            .take()
            .map(|d| d.preview_token)
            .unwrap_or_default();
        ui.set_dialog_open(false);
        ui.set_dialog_discard_open(false);
        ui.set_dialog_mcp(false);
        ui.set_fields(presenter::model(Vec::new()));
        ui.set_candidates(presenter::model(Vec::new()));
        ui.set_import_rows(presenter::model(Vec::new()));
        ui.set_apply_hosts(presenter::model(Vec::new()));
        ui.set_dialog_importer(false);
        ui.set_dialog_load_state(0);
        ui.set_dialog_error("".into());
        crate::editor_mcp::reset(&ui);
        self.import_state.borrow_mut().clear();
        if !preview.is_empty() {
            self.release_preview(&preview);
        }
        self.update_task_ui();
    }
    pub(crate) fn dialog_changed(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let mut token = String::new();
        if let Some(d) = self.dialog.borrow_mut().as_mut()
            && d.method == "InstallSkills"
            && !d.preview_token.is_empty()
            && editor::values(&ui).get("repository").map(String::as_str)
                != Some(d.preview_source.as_str())
        {
            token = std::mem::take(&mut d.preview_token);
            ui.set_candidates(presenter::model(Vec::new()));
            ui.set_dialog_error("来源已改变，请重新预览仓库。".into());
        }
        if !token.is_empty() {
            self.release_preview(&token);
        }
        self.sync_dialog_capability();
    }
    pub(crate) fn sync_dialog_capability(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let dialog = self.dialog.borrow();
        let Some(d) = dialog.as_ref() else {
            ui.set_dialog_can_submit(false);
            return;
        };
        let mut ready = d.loaded && !ui.get_dialog_busy() && !ui.get_dialog_discard_open();
        if ui.get_dialog_installer() {
            ready &= !d.preview_token.is_empty()
                && editor::values(&ui).get("repository").map(String::as_str)
                    == Some(d.preview_source.as_str())
                && ui
                    .get_candidates()
                    .iter()
                    .any(|c| c.checked && !c.installed);
        }
        if ui.get_dialog_importer() {
            ready &= ui.get_import_count() > 0 && ui.get_import_conflict().is_empty();
        }
        ui.set_dialog_can_submit(ready);
    }
}
