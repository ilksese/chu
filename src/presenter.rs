//! Pure page projection. Ephemeral selection/search live here, not in persisted resource state.
use crate::{
    models::*,
    ui::{self, AppWindow},
};
use slint::{ModelRc, VecModel};
use std::{collections::BTreeMap, rc::Rc};

pub const TITLES: [&str; 9] = [
    "总览",
    "项目管理",
    "Skills",
    "MCP 服务",
    "Providers",
    "自定义 Agent",
    "提示词",
    "References",
    "设置",
];
const DESCRIPTIONS: [&str; 9] = [
    "一个工作台，管理你的 Agent 资源。",
    "独立部署项目 Skills，保留本地修改。",
    "管理 Chu 中的技能，从宿主导入或从仓库安装。",
    "管理 Chu 中的 MCP，支持宿主导入与立即应用。",
    "管理 API 凭证与供应商模型目录。",
    "创建专属 Agent，部署到原生宿主。",
    "编辑全局提示词，安全切换与恢复。",
    "维护可复用的 Markdown 参考资料。",
    "宿主发现、路径配置、备份与显示设置。",
];
#[derive(Default)]
pub struct ViewState {
    pub snapshot: Snapshot,
    pub hidden: Vec<String>,
    pub page: i32,
    pub query: String,
    pub selected: String,
    pub details: BTreeMap<String, String>,
    pub updates: BTreeMap<String, String>,
    pub session: crate::session::Session,
    pub markdown_cache: BTreeMap<String, Vec<ui::MarkdownBlock>>,
}
pub fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(rows)).into()
}
fn host_row(host: &Host, hidden: &[String]) -> ui::HostRow {
    ui::HostRow {
        id: host.id.clone().into(),
        name: host.name.clone().into(),
        status: if host.installed {
            "已发现 · 可配置"
        } else {
            "未检测到"
        }
        .into(),
        installed: host.installed,
        visible: !hidden.contains(&host.id),
        config: host.config_path.clone().into(),
        skills: host.skill_path.clone().into(),
        agents: host.agent_path.clone().into(),
    }
}
pub fn detail_key(page: i32, id: &str) -> String {
    format!("{page}:{id}")
}
fn toggles(
    state: &ViewState,
    deps: &Map<bool>,
    modes: &Map<String>,
    managed: bool,
    remote: bool,
) -> Vec<ui::HostToggle> {
    state
        .snapshot
        .hosts
        .iter()
        .filter(|h| !state.hidden.contains(&h.id))
        .map(|h| {
            let enabled = *deps.get(&h.id).unwrap_or(&false);
            let available = managed && h.installed && !(remote && h.id == "jcode");
            ui::HostToggle {
                id: h.id.clone().into(),
                name: h.name.clone().into(),
                enabled,
                available,
                reset: false,
                status: if remote && h.id == "jcode" {
                    "仅支持 stdio".into()
                } else if !h.installed {
                    "未安装".into()
                } else if enabled {
                    let _ = modes;
                    "已应用".into()
                } else {
                    "未应用".into()
                },
            }
        })
        .collect()
}
pub fn render(ui: &AppWindow, state: &mut ViewState) {
    let page = state.page.clamp(0, 8);
    ui.set_page(page);
    ui.set_page_title(TITLES[page as usize].into());
    ui.set_page_description(DESCRIPTIONS[page as usize].into());
    ui.set_root_path(state.snapshot.root.clone().into());
    ui.set_hosts(model(
        state
            .snapshot
            .hosts
            .iter()
            .map(|h| host_row(h, &state.hidden))
            .collect(),
    ));
    ui.set_visible_hosts(model(
        state
            .snapshot
            .hosts
            .iter()
            .filter(|h| !state.hidden.contains(&h.id))
            .map(|h| host_row(h, &state.hidden))
            .collect(),
    ));
    ui.set_counts(model(vec![
        format!(
            "{}  Skills",
            state.snapshot.skills.iter().filter(|s| s.managed).count()
        )
        .into(),
        format!(
            "{}  MCP",
            state.snapshot.mcps.iter().filter(|m| m.managed).count()
        )
        .into(),
        format!("{}  Agents", state.snapshot.agents.len()).into(),
        format!("{}  项目", state.snapshot.projects.len()).into(),
    ]));
    let rows = crate::search::rows(state);
    let remembered = state
        .session
        .pages
        .get(&page)
        .map(|p| p.remembered.as_str())
        .unwrap_or("");
    if rows.iter().any(|r| r.id.as_str() == remembered) {
        state.selected = remembered.into();
    }
    if !rows.iter().any(|r| r.id.as_str() == state.selected) {
        state.selected = rows.first().map(|r| r.id.to_string()).unwrap_or_default();
    }
    if remembered.is_empty() && !state.selected.is_empty() {
        state.session.pages.entry(page).or_default().remembered = state.selected.clone();
    }
    ui.set_rows(model(rows));
    ui.set_selected_id(state.selected.clone().into());
    ui.set_selected_name("".into());
    ui.set_selected_description("".into());
    ui.set_selected_meta("".into());
    ui.set_action_one("".into());
    ui.set_action_two("".into());
    ui.set_action_three("".into());
    ui.set_selected_managed(false);
    ui.set_selected_tracked(false);
    ui.set_selected_models("".into());
    ui.set_deployments(model(Vec::new()));
    ui.set_project_skills(model(Vec::new()));
    ui.set_selected_content(
        state
            .details
            .get(&detail_key(page, &state.selected))
            .cloned()
            .unwrap_or_default()
            .into(),
    );
    let s = &state.snapshot;
    let id = state.selected.as_str();
    match page {
        1 => {
            if let Some(p) = s.projects.iter().find(|p| p.id == id) {
                ui.set_selected_name(p.name.clone().into());
                ui.set_selected_description(p.path.clone().into());
                ui.set_selected_meta(
                    format!(
                        "{} · 添加于 {}",
                        if p.available {
                            "目录可用"
                        } else {
                            "目录不可用"
                        },
                        p.created_at
                    )
                    .into(),
                );
                ui.set_action_one("重命名".into());
                ui.set_action_two("重新定位".into());
                ui.set_action_three("移除项目".into());
                ui.set_project_skills(model(
                    s.skills
                        .iter()
                        .filter(|s| s.managed)
                        .map(|skill| {
                            let deps = p.deployments.get(&skill.id);
                            let hosts = s
                                .hosts
                                .iter()
                                .filter(|h| !state.hidden.contains(&h.id))
                                .map(|h| {
                                    let d = deps.and_then(|d| d.get(&h.id));
                                    let status = d.map(|d| d.status.as_str()).unwrap_or("");
                                    ui::HostToggle {
                                        id: h.id.clone().into(),
                                        name: h.name.clone().into(),
                                        enabled: d.is_some_and(|d| d.enabled),
                                        available: p.available && status != "conflict",
                                        status: match status {
                                            "enabled" => "已部署",
                                            "modified" => "本地已修改 · 已保护",
                                            "missing" => "副本缺失",
                                            "conflict" => "同名目录冲突",
                                            _ => "未部署",
                                        }
                                        .into(),
                                        reset: d.is_some_and(|d| {
                                            d.enabled
                                                && ["modified", "missing"]
                                                    .contains(&d.status.as_str())
                                        }),
                                    }
                                })
                                .collect();
                            ui::ProjectSkillRow {
                                id: skill.id.clone().into(),
                                name: skill.name.clone().into(),
                                hosts: model(hosts),
                            }
                        })
                        .collect(),
                ));
            }
        }
        2 => {
            if let Some(r) = s.skills.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_description(r.description.clone().into());
                ui.set_selected_meta(format!("{}\n{}", r.source, r.repository).into());
                ui.set_selected_managed(r.managed);
                ui.set_selected_tracked(r.tracked);
                ui.set_deployments(model(toggles(
                    state,
                    &r.enabled_on,
                    &r.mode_by_host,
                    r.managed,
                    false,
                )));
                if !r.managed {
                    ui.set_action_one("导入中央库".into());
                } else {
                    if r.tracked {
                        ui.set_action_one("更新 Skill".into());
                    }
                    ui.set_action_three("删除 Skill".into());
                }
            }
        }
        3 => {
            if let Some(r) = s.mcps.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_description(r.description.clone().into());
                ui.set_selected_meta(
                    format!(
                        "{}\n{}{}",
                        r.r#type,
                        if r.r#type == "stdio" {
                            &r.command
                        } else {
                            &r.endpoint
                        },
                        if r.has_credentials {
                            "\n含凭证 · 不在界面暴露"
                        } else {
                            ""
                        }
                    )
                    .into(),
                );
                ui.set_deployments(model(toggles(
                    state,
                    &r.enabled_on,
                    &Map::new(),
                    r.managed,
                    r.r#type != "stdio",
                )));
                ui.set_selected_managed(r.managed);
                if r.managed {
                    ui.set_action_one("编辑 MCP".into());
                    ui.set_action_two("测试连接".into());
                    ui.set_action_three("删除 MCP".into());
                }
            }
        }
        4 => {
            if let Some(r) = s.providers.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_description(r.base_url.clone().into());
                ui.set_selected_meta(
                    format!(
                        "凭证变量: {}\n最后获取: {}{}",
                        if r.env_api_key.is_empty() {
                            "无需凭证"
                        } else {
                            &r.env_api_key
                        },
                        r.models_fetched_at,
                        if r.models_error.is_empty() {
                            String::new()
                        } else {
                            format!("\n{}", r.models_error)
                        }
                    )
                    .into(),
                );
                ui.set_action_one("编辑供应商".into());
                ui.set_action_two("刷新模型".into());
                ui.set_action_three("删除供应商".into());
                ui.set_selected_models(r.models.join("\n").into());
            }
        }
        5 => {
            if let Some(r) = s.agents.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_description(r.description.clone().into());
                ui.set_selected_meta(format!("模型: {} · 来源: {}", r.model, r.source).into());
                ui.set_deployments(model(toggles(
                    state,
                    &r.enabled_on,
                    &Map::new(),
                    r.managed,
                    false,
                )));
                ui.set_selected_managed(r.managed);
                if r.managed {
                    ui.set_action_one("编辑 Agent".into());
                    ui.set_action_three("删除 Agent".into());
                }
            }
        }
        6 => {
            if let Some(r) = s.prompts.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_meta(r.source.clone().into());
                ui.set_action_one("编辑提示词".into());
                ui.set_action_three("删除提示词".into());
                ui.set_deployments(model(toggles(
                    state,
                    &r.enabled_on,
                    &r.mode_by_host,
                    true,
                    false,
                )));
            }
        }
        7 => {
            if let Some(r) = s.references.iter().find(|r| r.id == id) {
                ui.set_selected_name(r.name.clone().into());
                ui.set_selected_meta(r.source.clone().into());
                ui.set_action_one("编辑 Reference".into());
                ui.set_action_three("删除 Reference".into());
            }
        }
        _ => {}
    }
    crate::presenter_extra::extras(ui, state);
}
