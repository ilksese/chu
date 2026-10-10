//! Hit-tested mouse and keyboard regression: accessibility actions alone bypass event overlays.
use chu::{
    presenter::model,
    ui::{AppWindow, CandidateRow, FormField, HostRow, HostToggle, ProjectSkillRow, ResourceRow},
};
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn window() -> AppWindow {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    ui.show().unwrap();
    ui
}
fn control(ui: &AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("missing enabled control: {label}"))
}
fn click(ui: &AppWindow, label: &str) {
    control(ui, label).mock_single_click(PointerEventButton::Left);
}
fn key(ui: &AppWindow, text: slint::SharedString) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn type_text(ui: &AppWindow, text: &str) {
    for c in text.chars() {
        key(ui, c.to_string().into());
    }
}
fn open_dialog(ui: &AppWindow) {
    ui.set_dialog_title("鼠标事件回归".into());
    ui.set_dialog_open(true);
}
fn deployment() -> HostToggle {
    HostToggle {
        id: "claude".into(),
        name: "Claude Code".into(),
        available: true,
        enabled: false,
        reset: false,
        status: "未启用".into(),
    }
}

#[test]
fn menu_changes_on_the_first_pointer_click() {
    let ui = window();
    let calls = Rc::new(Cell::new(0));
    let result = calls.clone();
    let weak = ui.as_weak();
    ui.on_navigate(move |page| {
        result.set(result.get() + 1);
        weak.upgrade().unwrap().set_page(page);
    });
    click(&ui, "项目管理");
    let after_first = calls.get();
    click(&ui, "项目管理");
    assert_eq!(
        after_first,
        1,
        "first click must navigate; first={after_first}, after second={}",
        calls.get()
    );
    for (page, label) in chu::presenter::TITLES.iter().enumerate() {
        let before = calls.get();
        click(&ui, label);
        assert_eq!(
            calls.get(),
            before + 1,
            "menu {label} must dispatch once on the first click"
        );
        assert_eq!(ui.get_page(), page as i32);
    }
}
#[test]
fn toolbar_and_resource_selection_dispatch_once_per_pointer_click() {
    let ui = window();
    ui.set_page(2);
    ui.set_rows(model(vec![
        ResourceRow {
            id: "first".into(),
            name: "First skill".into(),
            ..Default::default()
        },
        ResourceRow {
            id: "second".into(),
            name: "Second skill".into(),
            ..Default::default()
        },
    ]));
    let actions = Rc::new(RefCell::new(Vec::new()));
    let output = actions.clone();
    ui.on_action(move |action| output.borrow_mut().push(action.to_string()));
    for (label, expected) in [
        ("刷新", "refresh"),
        ("检查更新", "check-updates"),
        ("安装 Skills", "add"),
    ] {
        let count = actions.borrow().len();
        click(&ui, label);
        assert_eq!(
            actions.borrow().len(),
            count + 1,
            "{label} swallowed the first click"
        );
        assert_eq!(actions.borrow().last().unwrap(), expected);
    }
    let selected = Rc::new(RefCell::new(Vec::new()));
    let output = selected.clone();
    ui.on_select_row(move |id| output.borrow_mut().push(id.to_string()));
    click(&ui, "First skill");
    click(&ui, "Second skill");
    click(&ui, "First skill");
    assert_eq!(*selected.borrow(), vec!["first", "second", "first"]);
}
#[test]
fn detail_actions_dispatch_once_and_keep_keyboard_activation() {
    let ui = window();
    ui.set_page(4);
    ui.set_selected_id("provider".into());
    ui.set_selected_name("Fixture provider".into());
    ui.set_action_one("编辑供应商".into());
    ui.set_action_two("刷新模型".into());
    ui.set_action_three("删除供应商".into());
    let actions = Rc::new(RefCell::new(Vec::new()));
    let output = actions.clone();
    ui.on_action(move |action| output.borrow_mut().push(action.to_string()));
    for (label, expected) in [
        ("编辑供应商", "one"),
        ("刷新模型", "two"),
        ("删除供应商", "three"),
    ] {
        let before = actions.borrow().len();
        click(&ui, label);
        assert_eq!(
            actions.borrow().len(),
            before + 1,
            "{label} must activate on its first click"
        );
        assert_eq!(actions.borrow().last().unwrap(), expected);
    }
    key(&ui, Key::Return.into());
    key(&ui, Key::Space.into());
    assert_eq!(
        *actions.borrow(),
        vec!["one", "two", "three", "three", "three"]
    );
}
#[test]
fn dialog_buttons_preview_install_and_confirmation_activate_on_first_click() {
    let ui = window();
    open_dialog(&ui);
    let closed = Rc::new(Cell::new(0));
    let output = closed.clone();
    ui.on_close_dialog(move || output.set(output.get() + 1));
    let submitted = Rc::new(Cell::new(0));
    let output = submitted.clone();
    ui.on_submit_dialog(move || output.set(output.get() + 1));
    let previewed = Rc::new(Cell::new(0));
    let output = previewed.clone();
    ui.on_preview_skills(move || output.set(output.get() + 1));
    click(&ui, "保存");
    assert_eq!(submitted.get(), 1);
    click(&ui, "取消");
    click(&ui, "关闭编辑窗口");
    assert_eq!(closed.get(), 2);
    ui.set_dialog_installer(true);
    click(&ui, "预览仓库");
    click(&ui, "安装所选");
    assert_eq!(previewed.get(), 1);
    assert_eq!(submitted.get(), 2);
    ui.set_dialog_installer(false);
    ui.set_dialog_confirmation(true);
    click(&ui, "确认操作");
    assert_eq!(submitted.get(), 3);
    key(&ui, Key::Escape.into());
    assert_eq!(closed.get(), 3, "Escape must still bubble to the editor");
}
#[test]
fn deployment_checkboxes_toggle_with_one_click_when_focus_moves() {
    let ui = window();
    ui.set_page(2);
    ui.set_selected_id("skill".into());
    ui.set_deployments(model(vec![deployment()]));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let output = toggles.clone();
    ui.on_toggle(move |id, enabled| output.borrow_mut().push((id.to_string(), enabled)));
    click(&ui, "Claude Code 部署");
    assert_eq!(*toggles.borrow(), vec![("claude".into(), true)]);
    click(&ui, "刷新");
    click(&ui, "Claude Code 部署");
    assert_eq!(toggles.borrow().len(), 2);
    assert_eq!(toggles.borrow()[1], ("claude".into(), false));
}
#[test]
fn project_checkbox_and_reset_button_activate_with_one_click() {
    let ui = window();
    ui.set_page(1);
    ui.set_selected_id("project".into());
    ui.set_hosts(model(vec![HostRow {
        id: "claude".into(),
        name: "Claude Code".into(),
        installed: true,
        visible: true,
        ..Default::default()
    }]));
    ui.set_visible_hosts(ui.get_hosts());
    ui.set_project_skills(model(vec![ProjectSkillRow {
        id: "tdd".into(),
        name: "Test driven".into(),
        hosts: model(vec![HostToggle {
            reset: true,
            ..deployment()
        }]),
    }]));
    let toggles = Rc::new(RefCell::new(Vec::new()));
    let output = toggles.clone();
    ui.on_project_toggle(move |skill, host, enabled| {
        output
            .borrow_mut()
            .push((skill.to_string(), host.to_string(), enabled))
    });
    let resets = Rc::new(Cell::new(0));
    let output = resets.clone();
    ui.on_project_reset(move |_, _| output.set(output.get() + 1));
    click(&ui, "Test driven · Claude Code 部署");
    assert_eq!(
        *toggles.borrow(),
        vec![("tdd".into(), "claude".into(), true)]
    );
    click(&ui, "重置 Test driven · Claude Code 副本");
    assert_eq!(resets.get(), 1);
}
#[test]
fn editor_text_inputs_and_checkboxes_work_after_one_focus_click() {
    let ui = window();
    for (kind, label) in [(0, "名称"), (1, "Markdown 内容"), (4, "API Key")] {
        ui.set_dialog_open(false);
        ui.set_fields(model(vec![FormField {
            key: "value".into(),
            label: label.into(),
            kind,
            ..Default::default()
        }]));
        open_dialog(&ui);
        click(&ui, label);
        type_text(&ui, "first");
        assert_eq!(
            ui.get_fields().row_data(0).unwrap().value,
            "first",
            "{label} must accept typing after one click"
        );
    }
    ui.set_dialog_open(false);
    ui.set_fields(model(vec![FormField {
        key: "cleanup".into(),
        label: "清理副本".into(),
        value: "false".into(),
        kind: 2,
        ..Default::default()
    }]));
    open_dialog(&ui);
    click(&ui, "清理副本");
    assert_eq!(ui.get_fields().row_data(0).unwrap().value, "true");
}
#[test]
fn candidate_checkbox_changes_selection_on_the_first_click() {
    let ui = window();
    ui.set_dialog_installer(true);
    ui.set_candidates(model(vec![CandidateRow {
        name: "Candidate skill".into(),
        description: "Fixture".into(),
        path: "skills/tdd".into(),
        checked: false,
        installed: false,
    }]));
    open_dialog(&ui);
    click(&ui, "Candidate skill");
    assert!(ui.get_candidates().row_data(0).unwrap().checked);
    click(&ui, "Candidate skill");
    assert!(!ui.get_candidates().row_data(0).unwrap().checked);
}
#[test]
fn settings_visibility_save_and_restore_each_take_one_click() {
    let ui = window();
    ui.set_page(8);
    ui.set_hosts(model(vec![HostRow {
        id: "claude".into(),
        name: "Claude Code".into(),
        config: "/isolated/settings.json".into(),
        skills: "/isolated/skills".into(),
        agents: "/isolated/agents".into(),
        visible: true,
        ..Default::default()
    }]));
    let visibility = Rc::new(RefCell::new(Vec::new()));
    let output = visibility.clone();
    ui.on_host_visible(move |id, visible| output.borrow_mut().push((id.to_string(), visible)));
    let saves = Rc::new(Cell::new(0));
    let output = saves.clone();
    ui.on_save_host(move |_, config, skills, agents| {
        assert_eq!(config, "/isolated/settings.json");
        assert_eq!(skills, "/isolated/skills");
        assert_eq!(agents, "/isolated/agents");
        output.set(output.get() + 1);
    });
    let restored = Rc::new(Cell::new(0));
    let output = restored.clone();
    ui.on_restore_host(move |_| output.set(output.get() + 1));
    click(&ui, "在界面中显示 Claude Code");
    assert_eq!(*visibility.borrow(), vec![("claude".into(), false)]);
    click(&ui, "保存 Claude Code 路径");
    assert_eq!(saves.get(), 1);
    click(&ui, "恢复 Claude Code 配置备份");
    assert_eq!(restored.get(), 1);
}
#[test]
fn disabled_actions_and_modal_overlay_do_not_dispatch() {
    let ui = window();
    ui.set_page(4);
    let actions = Rc::new(Cell::new(0));
    let output = actions.clone();
    ui.on_action(move |_| output.set(output.get() + 1));
    let button = control(&ui, "新建");
    ui.set_busy(true);
    button.mock_single_click(PointerEventButton::Left);
    assert_eq!(actions.get(), 0);
    ui.set_busy(false);
    open_dialog(&ui);
    button.mock_single_click(PointerEventButton::Left);
    assert_eq!(
        actions.get(),
        0,
        "modal backdrop must block underlying buttons"
    );
}

#[test]
fn zero_sized_keyboard_targets_still_participate_in_tab_navigation() {
    let ui = window();
    let pages = Rc::new(RefCell::new(Vec::new()));
    let output = pages.clone();
    ui.on_navigate(move |page| output.borrow_mut().push(page));
    click(&ui, "总览");
    key(&ui, Key::Tab.into());
    key(&ui, Key::Return.into());
    assert_eq!(
        *pages.borrow(),
        vec![0, 1],
        "Tab + Enter must move from overview to projects"
    );
    key(&ui, Key::Tab.into());
    key(&ui, Key::Space.into());
    assert_eq!(*pages.borrow(), vec![0, 1, 2]);
}
