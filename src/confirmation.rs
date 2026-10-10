//! 危险确认始终展示实际对象与影响范围，不只展示操作类型。
use crate::{presenter::ViewState, ui::AppWindow};
use serde_json::Value;
pub(crate) fn describe(ui: &AppWindow, state: &ViewState, method: &str, id: &str, args: &Value) {
    let s = &state.snapshot;
    let host = |id: &str| {
        s.hosts
            .iter()
            .find(|h| h.id == id)
            .map_or(id.to_owned(), |h| h.name.clone())
    };
    let mut path = String::new();
    let name = match method {
        "DeleteMCP" => s.mcps.iter().find(|r| r.id == id).map(|r| r.name.clone()),
        "DeleteAgent" => s.agents.iter().find(|r| r.id == id).map(|r| r.name.clone()),
        "DeleteSkill" => s.skills.iter().find(|r| r.id == id).map(|r| {
            path = r.source.clone();
            r.name.clone()
        }),
        "DeletePrompt" => s.prompts.iter().find(|r| r.id == id).map(|r| {
            path = r.source.clone();
            r.name.clone()
        }),
        "DeleteReference" => s.references.iter().find(|r| r.id == id).map(|r| {
            path = r.source.clone();
            r.name.clone()
        }),
        "DeleteProvider" => s
            .providers
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.name.clone()),
        "DeleteProject" | "ResetProjectSkill" => s.projects.iter().find(|r| r.id == id).map(|r| {
            path = r.path.clone();
            r.name.clone()
        }),
        "RestoreBackup" => Some(host(id)),
        _ => None,
    }
    .unwrap_or_else(|| id.into());
    let (title, effect) = match method {
        "DeleteSkill" => (
            format!("删除 Skill：{name}"),
            "删除中央资源及其受管部署。项目仍在使用或检测到已修改副本时，整项删除会被拒绝。",
        ),
        "DeleteMCP" => (
            format!("删除 MCP：{name}"),
            "删除中央资源并清理全部受管宿主配置；任何配置漂移或冲突都会阻止整项删除，不影响无关资源。",
        ),
        "DeleteAgent" => (
            format!("删除 Agent：{name}"),
            "删除中央文件并清理全部受管宿主副本；检测到中央文件或副本被修改时整项拒绝。",
        ),
        "DeletePrompt" => (
            format!("删除提示词：{name}"),
            "删除此提示词，并按部署记录恢复宿主原始提示词备份。",
        ),
        "DeleteReference" => (
            format!("删除 Reference：{name}"),
            "永久删除此 Reference 文件。",
        ),
        "DeleteProvider" => (
            format!("移除供应商：{name}"),
            "移除供应商与模型目录；凭证文件中的变量保留。",
        ),
        "DeleteProject" => (
            format!("移除项目：{name}"),
            "移除项目关联；只有显式勾选清理才移除未修改副本，本地修改始终保留。",
        ),
        "RestoreBackup" => (
            format!("恢复 {name} 配置备份"),
            "恢复上一次配置备份；恢复前配置另存为 .chu.before-restore。",
        ),
        "ResetProjectSkill" => (
            format!("重置 {name} 的 Skill 副本"),
            "用中央 Skill 覆盖下列项目副本，放弃该副本的本地修改。",
        ),
        _ => ("确认操作".into(), "确认执行所选操作。"),
    };
    let mut description = format!("对象：{name}\n{effect}");
    if !path.is_empty() {
        description.push_str(&format!("\n路径：{path}"));
    }
    if method == "ResetProjectSkill" {
        let skill_id = args.get(1).and_then(Value::as_str).unwrap_or("");
        let host_id = args.get(2).and_then(Value::as_str).unwrap_or("");
        let skill = s
            .skills
            .iter()
            .find(|r| r.id == skill_id)
            .map_or(skill_id, |r| r.name.as_str());
        description.push_str(&format!("\nSkill：{skill}\n宿主：{}", host(host_id)));
    }
    if method == "RestoreBackup"
        && let Some(h) = s.hosts.iter().find(|h| h.id == id)
    {
        description.push_str(&format!("\n配置文件：{}", h.config_path));
    }
    ui.set_dialog_title(title.into());
    ui.set_dialog_description(description.into());
}
