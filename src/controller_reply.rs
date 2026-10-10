//! 后台回复只作用于其原始页面、对象和弹窗代次。
use crate::{
    controller::{Completion, Controller},
    feedback::Severity,
    presenter,
    session::DetailLoad,
    ui::{AppWindow, CandidateRow},
    worker::Reply,
};
use serde_json::Value;
use slint::Model;
impl Controller {
    pub(crate) fn apply_reply(&self, reply: Reply) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let is_release = matches!(reply.completion, Completion::ReleasePreview);
        let stale_detail = match &reply.completion {
            Completion::Detail { page, id } => !matches!(
                self.state
                    .borrow()
                    .session
                    .loads
                    .get(&presenter::detail_key(*page, id)),
                Some(DetailLoad::Loading)
            ),
            _ => false,
        };
        if is_release {
            return;
        }
        if stale_detail {
            self.load_detail();
            return;
        }
        if let Some(snapshot) = reply.snapshot {
            let mut s = self.state.borrow_mut();
            let ids = new_ids(&reply.method, reply.origin.page, &s.snapshot, &snapshot);
            if let Some(id) = ids.first() {
                let preserve_current = s.page == reply.origin.page
                    && s.session.selected_revision != reply.origin.selection;
                if !preserve_current {
                    s.session
                        .pages
                        .entry(reply.origin.page)
                        .or_default()
                        .remembered = id.clone();
                }
                if s.page == reply.origin.page
                    && s.session.selected_revision == reply.origin.selection
                {
                    if !s.query.is_empty() {
                        s.query.clear();
                        ui.set_query("".into());
                    }
                    s.selected = id.clone();
                }
            }
            s.snapshot = snapshot;
            s.hidden = reply.hidden;
            s.details.clear();
            s.markdown_cache.clear();
            s.session.invalidate_details();
        }
        let matches_dialog = self
            .dialog
            .borrow()
            .as_ref()
            .is_some_and(|d| d.generation == reply.origin.dialog);
        if let Some(error) = &reply.error {
            if let Completion::Detail { page, id } = &reply.completion {
                self.state.borrow_mut().session.loads.insert(
                    presenter::detail_key(*page, id),
                    DetailLoad::Failed(error.clone()),
                );
            } else if matches_dialog && reply.completion.dialog_result() {
                ui.set_dialog_error(error.clone().into());
                if matches!(reply.completion, Completion::Edit { .. }) {
                    if let Some(d) = self.dialog.borrow_mut().as_mut() {
                        d.loaded = false;
                    }
                    ui.set_dialog_load_state(2);
                }
            }
            self.show_feedback(&reply.feedback);
        } else if reply.feedback.severity == Severity::Cancelled {
            self.show_feedback(&reply.feedback);
        } else {
            let value = reply.value.unwrap_or(Value::Null);
            match reply.completion {
                Completion::Detail { page, id } => {
                    if let Some(content) = value.as_str() {
                        let key = presenter::detail_key(page, &id);
                        let mut s = self.state.borrow_mut();
                        s.details.insert(key.clone(), content.into());
                        s.session.loads.insert(key, DetailLoad::Ready);
                    }
                }
                Completion::Edit { page, id } if matches_dialog => {
                    self.apply_edit(&ui, page, &id, value)
                }
                Completion::ImportScan if matches_dialog => self.show_import_scan(value),
                Completion::Preview if matches_dialog => self.show_skill_preview(&ui, value),
                Completion::Preview => {
                    if let Some(token) = value.get("token").and_then(Value::as_str) {
                        self.release_preview(token);
                    }
                }
                Completion::Updates => {
                    let updates: Vec<crate::models::SkillUpdate> =
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
                    if close && matches_dialog {
                        self.dismiss_dialog();
                    }
                    self.show_feedback(&reply.feedback);
                }
                _ => {}
            }
        }
        let should_close = self.tasks.borrow().close_after_cancel == Some(reply.origin.dialog);
        if should_close {
            self.tasks.borrow_mut().close_after_cancel = None;
            if matches_dialog {
                self.close_dialog();
            }
        }
        self.render();
        self.load_detail();
    }
    fn show_skill_preview(&self, ui: &AppWindow, value: Value) {
        let Ok(preview) = serde_json::from_value::<crate::skill_preview::SkillPreview>(value)
        else {
            ui.set_dialog_error("仓库预览无效，请重新预览。".into());
            return;
        };
        let source = self
            .dialog
            .borrow()
            .as_ref()
            .map(|d| d.preview_source.clone())
            .unwrap_or_default();
        if crate::editor::values(ui)
            .get("repository")
            .map(String::as_str)
            != Some(source.as_str())
        {
            self.release_preview(&preview.token);
            return;
        }
        let mut dialog = self.dialog.borrow_mut();
        let Some(d) = dialog.as_mut() else {
            return;
        };
        d.preview_token = preview.token;
        ui.set_candidates(presenter::model(
            preview
                .candidates
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
        if ui.get_candidates().row_count() == 0 {
            ui.set_dialog_error("仓库中没有可安装的 Skill。".into());
        }
    }
}
fn new_ids(
    method: &str,
    page: i32,
    old: &crate::models::Snapshot,
    next: &crate::models::Snapshot,
) -> Vec<String> {
    match method {
        "CreateProvider" => next
            .providers
            .iter()
            .filter(|r| !old.providers.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        "CreatePrompt" | "UpdatePrompt" => next
            .prompts
            .iter()
            .filter(|r| !old.prompts.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        "CreateReference" | "UpdateReference" => next
            .references
            .iter()
            .filter(|r| !old.references.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        "AddAgent" => next
            .agents
            .iter()
            .filter(|r| !old.agents.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        "AddMCP" | "ImportHostResources" if page == 3 => next
            .mcps
            .iter()
            .filter(|r| r.managed && !old.mcps.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        "InstallSkillsFromPreview" | "InstallSkills" | "ImportHostResources" if page == 2 => next
            .skills
            .iter()
            .filter(|r| r.managed && !old.skills.iter().any(|p| p.id == r.id))
            .map(|r| r.id.clone())
            .collect(),
        _ => Vec::new(),
    }
}
