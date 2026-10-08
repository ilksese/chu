//! Resource listing/search across descriptive and technical source fields.
use crate::{presenter::ViewState, ui};
fn row(id: &str, name: &str, description: &str, meta: &str) -> ui::ResourceRow {
    ui::ResourceRow {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        meta: meta.into(),
    }
}
pub fn rows(state: &ViewState) -> Vec<ui::ResourceRow> {
    let s = &state.snapshot;
    let mut rows: Vec<_> = match state.page {
        1 => s
            .projects
            .iter()
            .map(|r| {
                row(
                    &r.id,
                    &r.name,
                    &r.path,
                    if r.available {
                        "项目目录可用"
                    } else {
                        "目录不可用 · 可重新定位"
                    },
                )
            })
            .collect(),
        2 => s
            .skills
            .iter()
            .filter(|r| r.managed)
            .map(|r| {
                row(
                    &r.id,
                    &r.name,
                    &r.description,
                    &format!(
                        "{}{}",
                        if r.managed {
                            "Chu 管理"
                        } else {
                            "宿主发现 · 待导入"
                        },
                        state
                            .updates
                            .get(&r.id)
                            .map(|s| format!(" · {s}"))
                            .unwrap_or_default()
                    ),
                )
            })
            .collect(),
        3 => s
            .mcps
            .iter()
            .filter(|r| r.managed)
            .map(|r| {
                row(
                    &r.id,
                    &r.name,
                    &r.description,
                    &format!(
                        "{} · {}",
                        r.r#type,
                        if r.managed {
                            "Chu 管理"
                        } else {
                            "外部配置 · 只读"
                        }
                    ),
                )
            })
            .collect(),
        4 => s
            .providers
            .iter()
            .map(|r| {
                row(
                    &r.id,
                    &r.name,
                    &r.base_url,
                    &format!(
                        "{} 个模型{}",
                        r.models.len(),
                        if r.models_error.is_empty() {
                            ""
                        } else {
                            " · 获取失败"
                        }
                    ),
                )
            })
            .collect(),
        5 => s
            .agents
            .iter()
            .map(|r| row(&r.id, &r.name, &r.description, &r.model))
            .collect(),
        6 => s
            .prompts
            .iter()
            .map(|r| row(&r.id, &r.name, &r.preview, "Markdown · 全局提示词"))
            .collect(),
        7 => s
            .references
            .iter()
            .map(|r| row(&r.id, &r.name, &r.preview, "Markdown · 参考资料"))
            .collect(),
        _ => Vec::new(),
    };
    let query = state.query.trim().to_lowercase();
    if !query.is_empty() {
        rows.retain(|r| search_text(state, r).to_lowercase().contains(&query));
    }
    rows
}

fn search_text(state: &ViewState, row: &ui::ResourceRow) -> String {
    let mut text = format!("{} {} {}", row.name, row.description, row.meta);
    let id = row.id.as_str();
    match state.page {
        2 => {
            if let Some(r) = state.snapshot.skills.iter().find(|r| r.id == id) {
                text.push_str(&format!(" {} {}", r.repository, r.source));
            }
        }
        3 => {
            if let Some(r) = state.snapshot.mcps.iter().find(|r| r.id == id) {
                text.push_str(&format!(" {} {}", r.command, r.endpoint));
            }
        }
        5 => {
            if let Some(r) = state.snapshot.agents.iter().find(|r| r.id == id) {
                text.push_str(&format!(" {} {}", r.source, r.model));
            }
        }
        _ => {}
    }
    text
}
