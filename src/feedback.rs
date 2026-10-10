//! 任务结果在后台归一化；界面不通过中文错误文本猜测成功状态。
use crate::models::Snapshot;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Severity {
    #[default]
    Success,
    Warning,
    Error,
    Cancelled,
}
#[derive(Clone, Default)]
pub struct Feedback {
    pub text: String,
    pub severity: Severity,
}
impl Feedback {
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            severity: Severity::Error,
        }
    }
    pub fn cancelled() -> Self {
        Self {
            text: "操作已取消，尚未提交资源更改。".into(),
            severity: Severity::Cancelled,
        }
    }
}
pub fn label(method: &str) -> &'static str {
    match method {
        "PreviewSkillsBound" | "PreviewSkills" => "预览 Skill 仓库",
        "InstallSkillsFromPreview" | "InstallSkills" => "安装所选 Skills",
        "CheckSkillUpdates" => "检查 Skill 更新",
        "UpdateSkill" => "更新 Skill",
        "ListImportCandidates" => "扫描宿主资源",
        "ImportHostResources" => "导入宿主资源",
        "TestMCPResult" | "TestMCP" => "测试 MCP 连接",
        "RefreshProviderModels" => "获取模型目录",
        "CreateProvider" | "UpdateProvider" => "保存供应商配置",
        "UpdateMCP" => "同步 MCP 配置",
        "UpdateAgent" => "同步 Agent 内容",
        "ReadMCPEdit" | "ReadAgentEdit" | "ReadPrompt" | "ReadReference" | "ReadSkill"
        | "ReadAgent" => "读取资源内容",
        "Refresh" | "GetSnapshot" => "刷新资源状态",
        _ => "提交资源更改",
    }
}
pub fn completed(
    method: &str,
    args: &Value,
    value: &Value,
    snapshot: Option<&Snapshot>,
) -> Feedback {
    let name = |i: usize| args.get(i).and_then(Value::as_str).unwrap_or("");
    if ["CreateProvider", "UpdateProvider", "RefreshProviderModels"].contains(&method) {
        let provider = snapshot.and_then(|s| {
            if method == "CreateProvider" {
                let n = args.get(0)?.get("name")?.as_str()?;
                s.providers.iter().find(|p| p.name == n)
            } else {
                s.providers.iter().find(|p| p.id == name(0))
            }
        });
        let error = provider.is_some_and(|p| !p.models_error.is_empty());
        return Feedback {
            text: if error {
                if method == "RefreshProviderModels" {
                    "模型获取失败，已保留上次缓存。请查看详情并重试。"
                } else {
                    "供应商配置已保存，但模型获取失败，已保留上次缓存。"
                }
            } else if method == "RefreshProviderModels" {
                "模型目录已刷新。"
            } else {
                "供应商配置已保存。"
            }
            .into(),
            severity: if error {
                Severity::Warning
            } else {
                Severity::Success
            },
        };
    }
    if method == "ImportHostResources" {
        let count = args.get(1).and_then(Value::as_array).map_or(0, Vec::len);
        let hosts: Vec<_> = args
            .get(2)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|id| {
                snapshot
                    .and_then(|s| s.hosts.iter().find(|h| h.id == id))
                    .map_or_else(|| id.to_owned(), |h| h.name.clone())
            })
            .collect();
        return Feedback {
            text: if hosts.is_empty() {
                format!("已导入 {count} 项到 Chu；未应用到宿主，原来源保持不变。")
            } else {
                format!("已导入 {count} 项，并应用到 {}。", hosts.join("、"))
            },
            ..Feedback::default()
        };
    }
    if let Some(retained) = value.get("retained").and_then(Value::as_array)
        && !retained.is_empty()
    {
        return Feedback {
            text: format!("项目已移除，保留了 {} 处本地修改。", retained.len()),
            severity: Severity::Warning,
        };
    }
    let text = match method {
        "AddMCP" => "MCP 服务已创建。",
        "UpdateMCP" => "MCP 已保存并同步到全部已启用宿主。",
        "DeleteMCP" => "MCP 已删除，受管宿主配置已清理。",
        "AddAgent" => "自定义 Agent 已创建。",
        "UpdateAgent" => "Agent 已保存并同步到全部已启用宿主。",
        "DeleteAgent" => "Agent 已删除，受管副本已清理。",
        "CreatePrompt" => "提示词已创建。",
        "UpdatePrompt" => "提示词已保存。",
        "DeletePrompt" => "提示词已删除，宿主原始内容已按部署记录恢复。",
        "CreateReference" => "Reference 已创建。",
        "UpdateReference" => "Reference 已保存。",
        "DeleteReference" => "Reference 已删除。",
        "DeleteProvider" => "供应商与模型目录已移除；凭证变量保持不变。",
        "InstallSkillsFromPreview" | "InstallSkills" => "所选 Skills 已安装。",
        "UpdateSkill" => "Skill 已更新，已修改的项目副本保持不变。",
        "DeleteSkill" | "RemoveSkill" => "Skill 已删除，受管部署已清理。",
        "ToggleSkill" | "ToggleMCP" | "ToggleAgent" | "TogglePrompt" | "ToggleProjectSkill" => {
            "部署状态已更新。"
        }
        "ResetProjectSkill" => "项目 Skill 副本已重置。",
        "AddProject" => "项目已添加。",
        "RenameProject" => "项目名称已更新。",
        "RelocateProject" => "项目路径已更新。",
        "DeleteProject" => "项目关联已移除。",
        "RestoreBackup" => "宿主配置已恢复，恢复前配置已另外备份。",
        "UpdateHostPaths" => "宿主路径已保存。",
        "SetHostVisible" => "宿主显示设置已更新。",
        "Refresh" | "GetSnapshot" => "资源状态已刷新。",
        _ => "操作已完成。",
    };
    Feedback {
        text: text.into(),
        ..Feedback::default()
    }
}
