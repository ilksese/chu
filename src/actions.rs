use crate::controller::{Completion, Controller};
use serde_json::{Value, json};
impl Controller {
    pub(crate) fn action(&self, action: &str) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if ui.get_busy() || ui.get_dialog_open() {
            return;
        }
        let (page, id) = {
            let s = self.state.borrow();
            (s.page, s.selected.clone())
        };
        match action {
            "refresh" => self.post("Refresh", json!([]), Completion::Snapshot { close: false }),
            "check-updates" => self.post("CheckSkillUpdates", json!([]), Completion::Updates),
            "import" if page == 2 || page == 3 => {
                self.open_import(if page == 2 { "skills" } else { "mcps" })
            }
            "add" => match page {
                1 => self.choose_project("AddProject", &id),
                2 => self.open("InstallSkills", "", Value::Null),
                3 => self.open("AddMCP", "", Value::Null),
                4 => self.open("CreateProvider", "", Value::Null),
                5 => self.open("AddAgent", "", Value::Null),
                6 => self.open("CreatePrompt", "", Value::Null),
                7 => self.open("CreateReference", "", Value::Null),
                _ => {}
            },
            "one" if !id.is_empty() => match page {
                1 => self.open("RenameProject", &id, Value::Null),
                2 => {
                    let s = self.state.borrow();
                    let Some(skill) = s.snapshot.skills.iter().find(|s| s.id == id) else {
                        return;
                    };
                    if skill.managed {
                        self.post(
                            "UpdateSkill",
                            json!([id]),
                            Completion::Snapshot { close: false },
                        );
                    } else if let Some((host, _)) = skill.enabled_on.iter().find(|(_, e)| **e) {
                        self.post(
                            "ImportSkill",
                            json!([host, skill.name]),
                            Completion::Snapshot { close: false },
                        );
                    }
                }
                3 => self.post(
                    "TestMCP",
                    json!([id]),
                    Completion::Snapshot { close: false },
                ),
                4 => self.open("UpdateProvider", &id, Value::Null),
                6 => self.open("UpdatePrompt", &id, Value::Null),
                7 => self.open("UpdateReference", &id, Value::Null),
                _ => {}
            },
            "two" if !id.is_empty() => match page {
                1 => self.choose_project("RelocateProject", &id),
                4 => self.post(
                    "RefreshProviderModels",
                    json!([id]),
                    Completion::Snapshot { close: false },
                ),
                _ => {}
            },
            "three" if !id.is_empty() => {
                let method = match page {
                    1 => "DeleteProject",
                    2 => "DeleteSkill",
                    4 => "DeleteProvider",
                    6 => "DeletePrompt",
                    7 => "DeleteReference",
                    _ => return,
                };
                self.open(method, &id, Value::Null);
            }
            _ => {}
        }
    }
    fn choose_project(&self, method: &str, id: &str) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(if method == "AddProject" {
                "选择项目根目录"
            } else {
                "重新选择项目根目录"
            })
            .pick_folder()
        {
            let path = path.to_string_lossy();
            let args = if method == "AddProject" {
                json!([path])
            } else {
                json!([id, path])
            };
            self.post(method, args, Completion::Snapshot { close: false });
        }
    }
    pub(crate) fn toggle(&self, host: &str, enabled: bool) {
        let s = self.state.borrow();
        let method = match s.page {
            2 => "ToggleSkill",
            3 => "ToggleMCP",
            5 => "ToggleAgent",
            6 => "TogglePrompt",
            _ => return,
        };
        self.post(
            method,
            json!([s.selected, host, enabled]),
            Completion::Snapshot { close: false },
        );
    }
    pub(crate) fn submit(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        let Some(dialog) = self.dialog.borrow().clone() else {
            return;
        };
        match crate::editor::command(&ui, &dialog) {
            Ok((method, args)) => self.post(&method, args, Completion::Snapshot { close: true }),
            Err(e) => ui.set_dialog_error(e.to_string().into()),
        }
    }
    pub(crate) fn preview(&self) {
        let Some(ui) = self.window.upgrade() else {
            return;
        };
        if let Some(dialog) = self.dialog.borrow().as_ref()
            && dialog.method == "ImportHostResources"
        {
            self.post(
                "ListImportCandidates",
                json!([dialog.id]),
                Completion::ImportScan,
            );
            return;
        }
        let values = crate::editor::values(&ui);
        self.post(
            "PreviewSkills",
            json!([values.get("repository").cloned().unwrap_or_default()]),
            Completion::Preview,
        );
    }
}
