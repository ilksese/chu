//! Typed dialog-to-command conversion. Secrets exist only in draft fields and the worker request.
use crate::{
    presenter::{ViewState, model},
    ui::{AppWindow, FormField},
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use slint::Model;
#[derive(Clone, Default)]
pub struct Dialog {
    pub method: String,
    pub id: String,
    pub args: Value,
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
            title = if method.contains("Prompt") {
                "编辑提示词"
            } else {
                "编辑 Reference"
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
        "AddMCP" => {
            title = "新建 MCP 服务";
            description =
                "args 使用 JSON 数组；env、headers 使用 JSON 对象。远程地址必须为 HTTPS。";
            fields.extend([
                field("name", "名称", "", 0, ""),
                field("description", "描述", "", 0, ""),
                field("type", "类型", "stdio", 0, "stdio / http / sse"),
                field("endpoint", "服务地址", "", 0, "https://…"),
                field("command", "启动命令", "", 0, "npx -y …"),
                field("args", "参数（JSON 数组）", "[]", 0, ""),
                field("env", "环境变量（JSON 对象）", "{}", 1, ""),
                field("headers", "请求头（JSON 对象）", "{}", 1, ""),
                field("secret", "访问令牌（可选）", "", 4, ""),
            ]);
        }
        "AddAgent" => {
            title = "新建自定义 Agent";
            fields.extend([
                field("name", "名称", "", 0, ""),
                field("description", "描述", "", 0, ""),
                field("model", "模型（可选）", "", 0, ""),
                field("prompt", "系统提示词", "", 1, ""),
            ]);
        }
        "CreateProvider" | "UpdateProvider" => {
            title = "编辑供应商";
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
    Dialog {
        method: method.into(),
        id: id.into(),
        args,
    }
}
pub fn values(ui: &AppWindow) -> std::collections::BTreeMap<String, String> {
    ui.get_fields()
        .iter()
        .map(|f| (f.key.to_string(), f.value.to_string()))
        .collect()
}
pub fn command(ui: &AppWindow, dialog: &Dialog) -> Result<(String, Value)> {
    let values = values(ui);
    let get = |key: &str| values.get(key).map(String::as_str).unwrap_or("");
    let method = dialog.method.clone();
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
        "InstallSkills" => {
            let paths: Vec<_> = ui
                .get_candidates()
                .iter()
                .filter(|c| c.checked && !c.installed)
                .map(|c| c.path.to_string())
                .collect();
            json!([get("repository"), paths])
        }
        "CreatePrompt" | "CreateReference" => json!([get("name"), get("content")]),
        "UpdatePrompt" | "UpdateReference" => json!([id, get("name"), get("content")]),
        "RenameProject" => json!([id, get("name")]),
        "DeleteProject" => json!([id, get("cleanup") == "true"]),
        "AddMCP" => {
            let args: Vec<String> =
                serde_json::from_str(get("args")).context("参数必须是字符串 JSON 数组")?;
            let env: crate::models::Map<String> =
                serde_json::from_str(get("env")).context("环境变量必须是字符串 JSON 对象")?;
            let headers: crate::models::Map<String> =
                serde_json::from_str(get("headers")).context("请求头必须是字符串 JSON 对象")?;
            json!([{"name":get("name"),"description":get("description"),"type":get("type"),"endpoint":get("endpoint"),"command":get("command"),"args":args,"env":env,"headers":headers,"secret":get("secret")}])
        }
        "AddAgent" => {
            json!([{"name":get("name"),"description":get("description"),"prompt":get("prompt"),"model":get("model")}])
        }
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
