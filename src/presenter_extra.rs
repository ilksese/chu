//! 阅读与概览投影不进行文件访问，也不在渲染时触发模型请求。
use crate::{
    presenter::{ViewState, detail_key, model},
    session::DetailLoad,
    ui::{AppWindow, IssueRow},
};
pub(crate) fn extras(ui: &AppWindow, state: &mut ViewState) {
    let overview = crate::overview::project(&state.snapshot);
    ui.set_count_rows(model(overview.counts));
    let mut issues = overview.issues;
    for (id, status) in &state.updates {
        if let Some(skill) = state
            .snapshot
            .skills
            .iter()
            .find(|s| s.managed && &s.id == id)
        {
            issues.push(IssueRow {
                title: format!("{} · {}", skill.name, status).into(),
                detail: "查看 Skill 来源与更新状态。".into(),
                page: 2,
                id: id.clone().into(),
                host: "".into(),
            });
        }
    }
    ui.set_issues(model(issues));
    ui.set_settings_host_id(state.session.settings_host.clone().into());
    let key = detail_key(state.page, &state.selected);
    let load = state.session.loads.get(&key);
    let read_page = matches!(state.page, 2 | 5 | 6 | 7);
    ui.set_detail_load_state(if !read_page || state.details.contains_key(&key) {
        0
    } else if matches!(load, Some(DetailLoad::Failed(_))) {
        2
    } else {
        1
    });
    ui.set_detail_error(
        if let Some(DetailLoad::Failed(error)) = load {
            error.clone()
        } else {
            String::new()
        }
        .into(),
    );
    let blocks = if let Some(content) = state.details.get(&key) {
        let signature = format!("{key}:{:x}", md5::compute(content));
        if let Some(blocks) = state.markdown_cache.get(&signature) {
            Some(blocks.clone())
        } else {
            let blocks = crate::markdown_view::project(&crate::markdown_view::parse(content));
            state.markdown_cache.insert(signature, blocks.clone());
            Some(blocks)
        }
    } else {
        None
    };
    ui.set_markdown_blocks(model(blocks.unwrap_or_default()));
}
