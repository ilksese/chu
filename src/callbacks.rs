//! 只捕获弱引用；回调只更新瞬时状态或发送后台命令。
use crate::{
    controller::{Completion, Controller},
    presenter::model,
    ui::AppWindow,
};
use serde_json::json;
use slint::{ComponentHandle, Model};
use std::rc::Rc;
pub(crate) fn wire(ui: &AppWindow, c: &Rc<Controller>) {
    let w = Rc::downgrade(c);
    ui.on_navigate(move |page| {
        if let Some(c) = w.upgrade() {
            c.navigate(page);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_navigate_target(move |page, id, host| {
        if let Some(c) = w.upgrade() {
            c.navigate_target(page, &id, &host);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_search(move |query| {
        if let Some(c) = w.upgrade() {
            c.search(&query);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_select_row(move |id| {
        if let Some(c) = w.upgrade() {
            c.select(&id);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_select_relative(move |delta| {
        if let Some(c) = w.upgrade() {
            c.select_relative(delta);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_select_index(move |index| {
        if let Some(c) = w.upgrade() {
            c.select_index(index);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_action(move |action| {
        if let Some(c) = w.upgrade() {
            c.action(&action);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_toggle(move |host, enabled| {
        if let Some(c) = w.upgrade() {
            c.toggle(&host, enabled);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_project_toggle(move |skill, host, enabled| {
        if let Some(c) = w.upgrade() {
            let id = c.state.borrow().selected.clone();
            c.post(
                "ToggleProjectSkill",
                json!([id, skill.as_str(), host.as_str(), enabled]),
                Completion::Snapshot { close: false },
            );
        }
    });
    let w = Rc::downgrade(c);
    ui.on_project_reset(move |skill, host| {
        if let Some(c) = w.upgrade() {
            let id = c.state.borrow().selected.clone();
            c.open(
                "ResetProjectSkill",
                &id,
                json!([id, skill.as_str(), host.as_str()]),
            );
        }
    });
    let w = Rc::downgrade(c);
    ui.on_save_host(move |id, config, skills, agents| {
        if let Some(c) = w.upgrade() {
            c.post(
                "UpdateHostPaths",
                json!([
                    id.as_str(),
                    config.as_str(),
                    skills.as_str(),
                    agents.as_str()
                ]),
                Completion::Snapshot { close: false },
            );
        }
    });
    let w = Rc::downgrade(c);
    ui.on_host_visible(move |id, visible| {
        if let Some(c) = w.upgrade() {
            c.post(
                "SetHostVisible",
                json!([id.as_str(), visible]),
                Completion::Snapshot { close: false },
            );
        }
    });
    let w = Rc::downgrade(c);
    ui.on_restore_host(move |id| {
        if let Some(c) = w.upgrade() {
            c.open("RestoreBackup", &id, json!([id.as_str()]));
        }
    });
    let w = Rc::downgrade(c);
    ui.on_submit_dialog(move || {
        if let Some(c) = w.upgrade() {
            c.submit();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_close_dialog(move || {
        if let Some(c) = w.upgrade() {
            c.close_dialog();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_discard_dialog(move || {
        if let Some(c) = w.upgrade() {
            c.dismiss_dialog();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_continue_dialog(move || {
        if let Some(c) = w.upgrade()
            && let Some(ui) = c.window.upgrade()
        {
            ui.set_dialog_discard_open(false);
            c.sync_dialog_capability();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_retry_dialog(move || {
        if let Some(c) = w.upgrade() {
            c.retry_dialog();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_retry_detail(move || {
        if let Some(c) = w.upgrade() {
            c.retry_detail();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_cancel_task(move || {
        if let Some(c) = w.upgrade() {
            c.cancel_task();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_preview_skills(move || {
        if let Some(c) = w.upgrade() {
            c.preview();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_import_selection_changed(move || {
        if let Some(c) = w.upgrade() {
            c.sync_import_options();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_import_filter_changed(move |query, host| {
        if let Some(c) = w.upgrade() {
            c.filter_import(&query, host);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_field_changed(move |_| {
        if let Some(c) = w.upgrade() {
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_dialog_changed(move || {
        if let Some(c) = w.upgrade() {
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_type_changed(move |_| {
        if let Some(c) = w.upgrade() {
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_arg_add(move || {
        if let Some(c) = w.upgrade()
            && let Some(ui) = c.window.upgrade()
        {
            let mut args: Vec<_> = ui.get_mcp_args().iter().collect();
            args.push("".into());
            ui.set_mcp_args(model(args));
            ui.set_mcp_args_action(1);
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_arg_remove(move |index| {
        if let Some(c) = w.upgrade()
            && let Some(ui) = c.window.upgrade()
        {
            let mut args: Vec<_> = ui.get_mcp_args().iter().collect();
            if index >= 0 && (index as usize) < args.len() {
                args.remove(index as usize);
            }
            ui.set_mcp_args(model(args));
            ui.set_mcp_args_action(1);
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_entry_add(move |kind| {
        if let Some(c) = w.upgrade()
            && let Some(ui) = c.window.upgrade()
        {
            crate::editor_mcp::add_entry(&ui, &kind);
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_entry_remove(move |kind, index| {
        if let Some(c) = w.upgrade()
            && let Some(ui) = c.window.upgrade()
        {
            crate::editor_mcp::remove_entry(&ui, &kind, index);
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_mcp_entry_changed(move |_, _| {
        if let Some(c) = w.upgrade() {
            c.dialog_changed();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_dismiss_notice(move || {
        if let Some(c) = w.upgrade() {
            c.notice("", false);
        }
    });
    let w = ui.as_weak();
    ui.on_copy_text(move |text| {
        if let Some(ui) = w.upgrade() {
            ui.invoke_copy_to_clipboard(text);
        }
    });
    let w = ui.as_weak();
    ui.on_copy_code(move |text| {
        if let Some(ui) = w.upgrade() {
            ui.invoke_copy_to_clipboard(text);
        }
    });
    let w = ui.as_weak();
    ui.on_copy_uri(move |text| {
        if let Some(ui) = w.upgrade() {
            ui.invoke_copy_to_clipboard(text);
        }
    });
}
