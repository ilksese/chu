//! 将表单转换为类型化命令；秘密仅存在于当前草稿和后台请求中。
use crate::{
    presenter::{ViewState, model},
    ui::{AppWindow, FormField},
};
use anyhow::Result;
use serde_json::{Value, json};
use slint::Model;
#[derive(Clone, Default)]
pub struct Dialog {
    pub method: String,
    pub id: String,
    pub args: Value,
    pub generation: u64,
    pub loaded: bool,
    pub baseline: Value,
    pub version: Option<String>,
    pub preview_token: String,
    pub preview_source: String,
}
impl Dialog {
    pub fn dirty(&self, ui: &AppWindow) -> bool {
        !ui.get_dialog_confirmation()
            && !ui.get_dialog_installer()
            && !ui.get_dialog_importer()
            && self.loaded
            && self.baseline != draft(ui)
    }
    pub fn rebase(&mut self, ui: &AppWindow) {
        self.baseline = draft(ui);
    }
}
pub fn draft(ui: &AppWindow) -> Value {
    json!({"fields": values(ui), "mcp": crate::editor_mcp::draft(ui)})
}
fn field(key: &str, label: &str, value: &str, kind: i32, placeholder: &str) -> FormField {
    FormField {
        key: key.into(),
        label: label.into(),
        value: value.into(),
        kind,
        placeholder: placeholder.into(),
    }
}
pub fn open(ui: &AppWindow, state: &ViewState, method: &str, id: &str, args: Value) -> Dialog {
    ui.set_dialog_error("".into());
    ui.set_dialog_open(true);
    ui.set_dialog_load_state(0);
    ui.set_dialog_discard_open(false);
    ui.set_dialog_short(matches!(
        method,
        "RenameProject"
            | "DeleteProject"
            | "DeleteSkill"
            | "DeleteMCP"
            | "DeleteAgent"
            | "DeletePrompt"
            | "DeleteReference"
            | "DeleteProvider"
            | "RestoreBackup"
            | "ResetProjectSkill"
    ));
    ui.set_dialog_mcp(matches!(method, "AddMCP" | "UpdateMCP"));
    ui.set_import_query("".into());
    ui.set_import_host_filter(0);
    ui.set_import_conflict("".into());
    ui.set_import_compatibility("".into());
    crate::editor_mcp::reset(ui);
    ui.set_dialog_installer(false);
    ui.set_dialog_importer(false);
    ui.set_import_rows(model(Vec::new()));
    ui.set_apply_hosts(model(Vec::new()));
    ui.set_import_count(0);
    ui.set_has_apply_targets(false);
    ui.set_dialog_confirmation(false);
    ui.set_candidates(model(Vec::new()));
    let mut fields = Vec::new();
    let title;
    let mut description = "文件写入使用原子替换，已有用户配置与改动受到保护。";
    match method {
        "ImportHostResources" => {
            title = if id == "skills" {
                "从宿主导入 Skills"
            } else {
                "从宿主导入 MCP 服务"
            };
            description = "勾选需要纳入 Chu 管理的资源，再选择立即应用的宿主。同名但内容不同的目标不会被覆盖。";
            ui.set_dialog_importer(true);
            ui.set_apply_hosts(model(
                state
                    .snapshot
                    .hosts
                    .iter()
                    .map(|h| crate::ui::ApplyHostRow {
                        id: h.id.clone().into(),
                        name: h.name.clone().into(),
                        checked: false,
                        available: h.installed,
                        reason: if h.installed { "" } else { "未检测到" }.into(),
                    })
                    .collect(),
            ));
        }
        "InstallSkills" => {
            title = "安装 Skills";
            description = "支持 owner/repo、owner/repo@skill、GitHub/GitLab HTTPS 地址及 #分支。先预览，再选择所需技能。";
            ui.set_dialog_installer(true);
            fields.push(field(
                "repository",
                "来源仓库",
                "",
                0,
                "owner/repo 或 HTTPS 仓库地址",
            ));
        }
        "CreatePrompt" | "UpdatePrompt" | "CreateReference" | "UpdateReference" => {
            title = match method {
                "CreatePrompt" => "新建提示词",
                "UpdatePrompt" => "编辑提示词",
                "CreateReference" => "新建 Reference",
                _ => "编辑 Reference",
            };
            let name = if method.contains("Prompt") {
                state
                    .snapshot
                    .prompts
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str())
            } else {
                state
                    .snapshot
                    .references
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str())
            }
            .unwrap_or("");
            let content = state
                .details
                .get(&crate::presenter::detail_key(state.page, id))
                .map(String::as_str)
                .unwrap_or("");
            fields.extend([
                field("name", "名称", name, 0, "Markdown 文件名"),
                field("content", "Markdown 内容", content, 1, ""),
            ]);
        }
        "AddMCP" | "UpdateMCP" => {
            title = if method == "AddMCP" {
                "新建 MCP 服务"
            } else {
                "编辑 MCP 服务"
            };
            description = "保存会同步全部已启用宿主；检测到外部修改或不兼容时整项拒绝。已有秘密默认保持，不回填明文。";
            fields.extend([
                field("name", "名称", "", 0, ""),
                field("description", "描述", "", 0, ""),
            ]);
        }
        "AddAgent" | "UpdateAgent" => {
            title = if method == "AddAgent" {
                "新建自定义 Agent"
            } else {
                "编辑自定义 Agent"
            };
            description = "保存会同步全部已启用宿主；中央文件或宿主副本已被外部修改时整项拒绝。";
            fields.extend([
                field("name", "名称", "", 0, ""),
                field("description", "描述", "", 0, ""),
                field("model", "模型（可选）", "", 0, ""),
                field("prompt", "系统提示词", "", 1, ""),
            ]);
        }
        "CreateProvider" | "UpdateProvider" => {
            title = if method == "CreateProvider" {
                "新建供应商"
            } else {
                "编辑供应商"
            };
            description =
                "API Key 不写入状态或快照。编辑时留空保留现有凭证；模型请求不跟随重定向。";
            let p = state.snapshot.providers.iter().find(|p| p.id == id);
            fields.extend([
                field(
                    "name",
                    "供应商名称",
                    p.map(|p| p.name.as_str()).unwrap_or(""),
                    0,
                    "",
                ),
                field(
                    "baseUrl",
                    "Base URL",
                    p.map(|p| p.base_url.as_str()).unwrap_or(""),
                    0,
                    "https://api.example.com/v1",
                ),
                field(
                    "envApiKey",
                    "API Key 环境变量",
                    p.map(|p| p.env_api_key.as_str()).unwrap_or(""),
                    0,
                    "EXAMPLE_API_KEY",
                ),
                field("apiKey", "API Key（可选）", "", 4, "留空保留现有凭证"),
            ]);
        }
        "RenameProject" => {
            title = "重命名项目";
            fields.push(field(
                "name",
                "项目名称",
                state
                    .snapshot
                    .projects
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.as_str())
                    .unwrap_or(""),
                0,
                "",
            ));
        }
        _ => {
            title = "确认操作";
            description = match method {
                "DeleteSkill" => {
                    "将删除中央 Skill 并关闭所有宿主部署；被项目使用或已修改的副本不会被删除。"
                }
                "DeletePrompt" => "删除提示词并恢复宿主原始提示词备份。",
                "DeleteReference" => "将永久删除此 Reference 文件。",
                "DeleteProvider" => "移除供应商与模型目录；凭证文件中的变量保留。",
                "DeleteProject" => {
                    "移除项目关联。可选择清理未修改的 Skill 副本；本地改动始终保留。"
                }
                "ResetProjectSkill" => "此操作将用中央 Skill 覆盖项目副本，放弃该副本的本地修改。",
                "RestoreBackup" => "恢复上一次配置备份；当前配置会另存为 .chu.before-restore。",
                _ => "确认执行所选操作？",
            };
            ui.set_dialog_confirmation(true);
            if method == "DeleteProject" {
                fields.push(field(
                    "cleanup",
                    "同时清理未修改的 Skill 副本",
                    "false",
                    2,
                    "",
                ));
            }
        }
    }
    ui.set_dialog_title(title.into());
    ui.set_dialog_description(description.into());
    ui.set_fields(model(fields));
    if ui.get_dialog_confirmation() {
        crate::confirmation::describe(ui, state, method, id, &args);
    }
    Dialog {
        method: method.into(),
        id: id.into(),
        args,
        loaded: !matches!(
            method,
            "UpdatePrompt" | "UpdateReference" | "UpdateMCP" | "UpdateAgent"
        ),
        ..Dialog::default()
    }
}
pub fn values(ui: &AppWindow) -> std::collections::BTreeMap<String, String> {
    ui.get_fields()
        .iter()
        .map(|f| (f.key.to_string(), f.value.to_string()))
        .collect()
}
pub fn command(ui: &AppWindow, dialog: &Dialog) -> Result<(String, Value)> {
    if !dialog.loaded {
        anyhow::bail!("原始内容尚未成功加载，请先重试读取。");
    }
    let values = values(ui);
    let get = |key: &str| values.get(key).map(String::as_str).unwrap_or("");
    let method = if dialog.method == "InstallSkills" {
        "InstallSkillsFromPreview".into()
    } else {
        dialog.method.clone()
    };
    let id = &dialog.id;
    let args = match method.as_str() {
        "ImportHostResources" => {
            let selected: Vec<_> = ui
                .get_import_rows()
                .iter()
                .filter(|r| r.available && r.checked)
                .map(|r| r.id.to_string())
                .collect();
            let hosts: Vec<_> = ui
                .get_apply_hosts()
                .iter()
                .filter(|h| h.available && h.checked)
                .map(|h| h.id.to_string())
                .collect();
            if selected.is_empty() {
                anyhow::bail!("请至少勾选一项需要导入的资源");
            }
            json!([id, selected, hosts])
        }
        "InstallSkillsFromPreview" => {
            if dialog.preview_token.is_empty() || get("repository") != dialog.preview_source {
                anyhow::bail!("来源已改变或尚未预览，请重新预览仓库。");
            }
            let paths: Vec<_> = ui
                .get_candidates()
                .iter()
                .filter(|c| c.checked && !c.installed)
                .map(|c| c.path.to_string())
                .collect();
            if paths.is_empty() {
                anyhow::bail!("请至少选择一个未安装的 Skill。");
            }
            json!([dialog.preview_token, paths])
        }
        "CreatePrompt" | "CreateReference" => json!([get("name"), get("content")]),
        "UpdatePrompt" | "UpdateReference" => json!([id, get("name"), get("content")]),
        "RenameProject" => json!([id, get("name")]),
        "DeleteProject" => json!([id, get("cleanup") == "true"]),
        "AddMCP" | "UpdateMCP" => return crate::editor_mcp::command(ui, dialog, &values),
        "AddAgent" => {
            json!([{"name":get("name"),"description":get("description"),"prompt":get("prompt"),"model":get("model")}])
        }
        "UpdateAgent" => {
            json!([id, {"version":dialog.version,"input":{"name":get("name"),"description":get("description"),"prompt":get("prompt"),"model":get("model")}}])
        }
        "DeleteMCP" | "DeleteAgent" => json!([id, {"version":dialog.version}]),
        "CreateProvider" | "UpdateProvider" => {
            let input = json!({"name":get("name"),"baseUrl":get("baseUrl"),"envApiKey":get("envApiKey"),"apiKey":get("apiKey")});
            if method == "UpdateProvider" {
                json!([id, input])
            } else {
                json!([input])
            }
        }
        _ => {
            if dialog.args.is_null() {
                json!([id])
            } else {
                dialog.args.clone()
            }
        }
    };
    Ok((method, args))
}
