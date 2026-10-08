//! Import modal projection and host compatibility derived from the current checked candidates.
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
        match serde_json::from_value::<ImportScan>(value) {
            Ok(scan) => {
                ui.set_import_rows(model(
                    scan.candidates
                        .into_iter()
                        .map(|c| ImportRow {
                            id: c.id.into(),
                            name: format!("{} · {}", c.name, c.host_name).into(),
                            description: c.description.into(),
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
                            checked: false,
                            available: c.available,
                            reason: c.reason.into(),
                            remote: c.r#type != "skill" && c.r#type != "stdio",
                        })
                        .collect(),
                ));
                ui.set_dialog_error(scan.warnings.join("\n").into());
                self.sync_import_options();
            }
            Err(_) => ui.set_dialog_error("导入候选数据无效，请重新扫描。".into()),
        }
    }
    pub(crate) fn sync_import_options(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let selected: Vec<_> = ui
            .get_import_rows()
            .iter()
            .filter(|r| r.checked && r.available)
            .collect();
        ui.set_import_count(selected.len() as i32);
        let remote = selected.iter().any(|r| r.remote);
        let state = self.state.borrow();
        let hosts = ui
            .get_apply_hosts()
            .iter()
            .map(|mut h| {
                let installed = state
                    .snapshot
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
                if !h.available {
                    h.checked = false;
                }
                h
            })
            .collect::<Vec<_>>();
        ui.set_has_apply_targets(hosts.iter().any(|h| h.checked && h.available));
        ui.set_apply_hosts(model(hosts));
    }
}
