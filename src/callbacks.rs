use crate::{
    controller::{Completion, Controller},
    ui::AppWindow,
};
use serde_json::json;
use std::rc::Rc;
/// Only weak references are captured; closing the window releases models, worker and timers.
pub(crate) fn wire(ui: &AppWindow, c: &Rc<Controller>) {
    let w = Rc::downgrade(c);
    ui.on_navigate(move |page| {
        if let Some(c) = w.upgrade() {
            c.navigate(page);
        }
    });
    let w = Rc::downgrade(c);
    ui.on_search(move |query| {
        if let Some(c) = w.upgrade() {
            c.state.borrow_mut().query = query.into();
            c.render();
            c.load_detail();
        }
    });
    let w = Rc::downgrade(c);
    ui.on_select_row(move |id| {
        if let Some(c) = w.upgrade() {
            c.state.borrow_mut().selected = id.into();
            c.render();
            c.load_detail();
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
    ui.on_dismiss_notice(move || {
        if let Some(c) = w.upgrade() {
            c.notice("", false);
        }
    });
}
