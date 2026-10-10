//! 导入弹窗展示安全候选；筛选和重扫不重置有效选择。
use crate::{
    controller::{Completion, Controller},
    models::ImportScan,
    presenter::model,
    ui::ImportRow,
};
use serde_json::{Value, json};
use slint::Model;
impl Controller {
    pub(crate) fn open_import(&self, kind: &str) {
        self.import_state.borrow_mut().clear();
        self.open("ImportHostResources", kind, Value::Null);
        self.post(
            "ListImportCandidates",
            json!([kind]),
            Completion::ImportScan,
        );
    }
    pub(crate) fn show_import_scan(&self, value: Value) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        self.capture_import_selection();
        match serde_json::from_value::<ImportScan>(value) {
            Ok(scan) => {
                let (mut warnings, removed) = self.import_state.borrow_mut().merge(scan);
                if removed > 0 {
                    warnings.push(format!(
                        "重新扫描后，{removed} 项来源内容已变化、不可用或消失，已取消这些选择。"
                    ));
                }
                let mut names = vec!["全部宿主".into()];
                names.extend(
                    self.import_state
                        .borrow()
                        .hosts
                        .iter()
                        .map(|(_, name)| name.clone().into()),
                );
                ui.set_import_host_names(model(names));
                ui.set_import_host_filter(self.import_state.borrow().host_filter as i32);
                ui.set_dialog_error(warnings.join("\n").into());
                self.render_import();
                self.sync_import_options();
            }
            Err(_) => ui.set_dialog_error("导入候选数据无效，请重新扫描。".into()),
        }
    }
    pub(crate) fn capture_import_selection(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let mut state = self.import_state.borrow_mut();
        for r in ui.get_import_rows().iter() {
            if r.available && r.checked {
                state.selected.insert(r.id.to_string());
            } else {
                state.selected.remove(r.id.as_str());
            }
        }
    }
    pub(crate) fn render_import(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let state = self.import_state.borrow();
        ui.set_import_rows(model(
            state
                .visible()
                .into_iter()
                .map(|c| ImportRow {
                    id: c.id.clone().into(),
                    raw_name: c.name.clone().into(),
                    host_id: c.host_id.clone().into(),
                    name: format!("{} · {}", c.name, c.host_name).into(),
                    description: c.description.clone().into(),
                    source: format!(
                        "{} · {}{}",
                        c.r#type,
                        c.source,
                        if c.has_credentials {
                            " · 含凭证（不展示）"
                        } else {
                            ""
                        }
                    )
                    .into(),
                    checked: state.selected.contains(&c.id),
                    available: c.available,
                    reason: c.reason.clone().into(),
                    remote: c.r#type != "skill" && c.r#type != "stdio",
                })
                .collect(),
        ));
        ui.set_import_conflict(state.conflicts().into());
    }
    pub(crate) fn filter_import(&self, query: &str, filter: i32) {
        self.capture_import_selection();
        {
            let mut state = self.import_state.borrow_mut();
            state.query = query.into();
            state.host_filter = filter.max(0) as usize;
        }
        self.render_import();
        self.sync_import_options();
    }
    pub(crate) fn sync_import_options(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        self.capture_import_selection();
        let state = self.import_state.borrow();
        let selected = state.selected_candidates();
        ui.set_import_count(selected.len() as i32);
        ui.set_import_conflict(state.conflicts().into());
        let remote = selected
            .iter()
            .any(|r| r.r#type != "skill" && r.r#type != "stdio");
        let snapshot = &self.state.borrow().snapshot;
        let mut removed = false;
        let hosts = ui
            .get_apply_hosts()
            .iter()
            .map(|mut h| {
                let installed = snapshot
                    .hosts
                    .iter()
                    .any(|host| host.id == h.id.as_str() && host.installed);
                h.available = installed && !(h.id == "jcode" && remote);
                h.reason = if !installed {
                    "未检测到"
                } else if h.id == "jcode" && remote {
                    "仅支持 stdio"
                } else {
                    ""
                }
                .into();
                if !h.available && h.checked {
                    removed = true;
                    h.checked = false;
                }
                h
            })
            .collect::<Vec<_>>();
        if removed {
            ui.set_import_compatibility(
                "不兼容的宿主已从本次应用计划移除；兼容性恢复后需重新勾选。".into(),
            );
        }
        ui.set_has_apply_targets(hosts.iter().any(|h| h.checked && h.available));
        ui.set_apply_hosts(model(hosts));
        drop(state);
        self.sync_dialog_capability();
    }
    pub(crate) fn import_command_args(&self) -> Option<Value> {
        let ui = self.window.upgrade()?;
        self.capture_import_selection();
        let kind = self.dialog.borrow().as_ref()?.id.clone();
        let state = self.import_state.borrow();
        if !state.conflicts().is_empty() || state.selected.is_empty() {
            return None;
        }
        let selected: Vec<_> = state
            .selected_candidates()
            .iter()
            .map(|c| c.id.clone())
            .collect();
        let hosts: Vec<_> = ui
            .get_apply_hosts()
            .iter()
            .filter(|h| h.available && h.checked)
            .map(|h| h.id.to_string())
            .collect();
        Some(json!([kind, selected, hosts]))
    }
}
