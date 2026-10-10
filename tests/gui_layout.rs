//! 纯原生 UI 几何与鼠标命中回归，不访问宿主文件系统或工作线程。
use chu::{
    markdown_view,
    presenter::model,
    ui::{
        AppWindow, CountRow, FormField, HostRow, HostToggle, ImportRow, KeyValueRow, ResourceRow,
    },
};
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, LogicalPosition, LogicalSize, Model,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn window(width: f32, height: f32) -> AppWindow {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(LogicalSize::new(width, height));
    ui.show().unwrap();
    ui
}
fn element(ui: &AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| {
            e.accessible_enabled() == Some(true) && e.size().width > 0. && e.size().height > 0.
        })
        .or_else(|| {
            ElementHandle::find_by_accessible_label(ui, label).find(|e| {
                e.accessible_enabled() == Some(false) && e.size().width > 0. && e.size().height > 0.
            })
        })
        .or_else(|| {
            ElementHandle::find_by_accessible_label(ui, label)
                .find(|e| e.size().width > 0. && e.size().height > 0.)
        })
        .unwrap_or_else(|| panic!("missing element {label}"))
}
fn pointer(ui: &AppWindow, position: LogicalPosition) {
    ui.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    ui.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    ui.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}
fn click(ui: &AppWindow, label: &str) {
    let target = element(ui, label);
    let origin = target.absolute_position();
    let size = target.size();
    pointer(
        ui,
        LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.),
    );
}
fn key(ui: &AppWindow, key: impl Into<slint::SharedString>) {
    let text = key.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn resources(ui: &AppWindow, count: usize) {
    ui.set_page(2);
    ui.set_rows(model(
        (0..count)
            .map(|index| ResourceRow {
                id: format!("r{index}").into(),
                name: format!("Resource {index}").into(),
                description: "简短摘要".into(),
                meta: "本地 · managed".into(),
            })
            .collect(),
    ));
    ui.set_selected_id("r0".into());
    ui.set_selected_name("Resource 0".into());
    let source = "# 标题\n\n正文 **强调**。\n\n```rust\nlet value = 42;\n```\n";
    ui.set_selected_content(source.into());
    ui.set_markdown_blocks(model(markdown_view::project(&markdown_view::parse(source))));
}
fn field(key: &str, kind: i32) -> FormField {
    FormField {
        key: key.into(),
        label: key.into(),
        kind,
        ..Default::default()
    }
}

#[test]
fn compact_whole_rows_and_readable_preview_fit_both_desktop_sizes() {
    i_slint_backend_testing::init_no_event_loop();
    for (width, height) in [(1360., 860.), (1080., 680.)] {
        let ui = AppWindow::new().unwrap();
        ui.window().set_size(LogicalSize::new(width, height));
        ui.show().unwrap();
        resources(&ui, 12);
        ui.set_selected_description("很长的说明 ".repeat(60).into());
        ui.set_selected_meta("/very/long/path/".repeat(60).into());
        ui.set_action_one("编辑".into());
        ui.set_action_two("查看来源".into());
        ui.set_action_three("删除".into());
        ui.set_deployments(model(vec![HostToggle {
            id: "host".into(),
            name: "宿主".into(),
            available: true,
            ..Default::default()
        }]));
        let row = element(&ui, "Resource 0");
        assert!(
            (68. ..=76.).contains(&row.size().height),
            "row must remain compact: {:?}",
            row.size()
        );
        assert!(
            (270. ..=340.).contains(&row.size().width),
            "list must retain constrained width: {:?}",
            row.size()
        );
        let preview = element(&ui, "Resource 0 预览");
        assert!(
            preview.size().height >= 100.,
            "preview squeezed at {width}x{height}: {:?}",
            preview.size()
        );
        assert!(preview.absolute_position().y + preview.size().height <= height + 1.);
        assert!(preview.absolute_position().x + preview.size().width <= width + 1.);
        let selected = Rc::new(RefCell::new(Vec::new()));
        let output = selected.clone();
        ui.on_select_row(move |id| output.borrow_mut().push(id.to_string()));
        let origin = row.absolute_position();
        // 点击元数据区域，不仅点击标题或直接调用无障碍动作。
        pointer(
            &ui,
            LogicalPosition::new(origin.x + 24., origin.y + row.size().height - 12.),
        );
        assert_eq!(*selected.borrow(), vec!["r0"]);
    }
}

#[test]
fn resource_keyboard_selects_read_preview_and_scrolls_selected_row_into_view() {
    let ui = window(1080., 680.);
    resources(&ui, 24);
    let weak = ui.as_weak();
    ui.on_select_relative(move |delta| {
        let ui = weak.upgrade().unwrap();
        let index: i32 = ui.get_selected_id()[1..].parse().unwrap();
        ui.set_selected_id(format!("r{}", (index + delta).clamp(0, 23)).into());
    });
    let weak = ui.as_weak();
    ui.on_select_index(move |index| {
        weak.upgrade()
            .unwrap()
            .set_selected_id(format!("r{}", if index < 0 { 23 } else { index }).into())
    });
    click(&ui, "Resource 0");
    key(&ui, Key::DownArrow);
    assert_eq!(ui.get_selected_id(), "r1");
    key(&ui, Key::End);
    assert_eq!(ui.get_selected_id(), "r23");
    assert!(
        ui.get_list_scroll_y() < -500.,
        "keyboard selection must move scroll state"
    );
    key(&ui, Key::Home);
    assert_eq!(ui.get_selected_id(), "r0");
    assert!(ui.get_list_scroll_y().abs() <= 1.);
    key(&ui, Key::UpArrow);
    assert_eq!(ui.get_selected_id(), "r0");
}

#[test]
fn background_task_keeps_browsing_enabled_and_write_controls_disabled() {
    let ui = window(1080., 680.);
    resources(&ui, 2);
    ui.set_busy(true);
    ui.set_action_one("编辑".into());
    ui.set_task_label("扫描资源".into());
    ui.set_task_can_cancel(true);
    ui.set_task_phase(1);
    for label in [
        "项目管理",
        "搜索资源",
        "Resource 0",
        "Markdown 预览",
        "Markdown 源码",
        "复制内容",
        "取消任务",
    ] {
        assert_eq!(
            element(&ui, label).accessible_enabled(),
            Some(true),
            "read control disabled: {label}"
        );
    }
    for label in ["安装 Skills", "导入", "编辑"] {
        assert_eq!(
            element(&ui, label).accessible_enabled(),
            Some(false),
            "write control enabled: {label}"
        );
    }
    ui.set_task_phase(2);
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "取消任务")
            .next()
            .is_none()
    );
}

#[test]
fn modal_blocks_background_mouse_and_tab_then_restores_trigger_focus() {
    let ui = window(1080., 680.);
    resources(&ui, 2);
    ui.set_fields(model(vec![field("名称", 0)]));
    let calls = Rc::new(Cell::new(0));
    let output = calls.clone();
    let weak = ui.as_weak();
    ui.on_action(move |action| {
        output.set(output.get() + 1);
        if action == "add" {
            weak.upgrade().unwrap().set_dialog_open(true);
        }
    });
    let navigation = Rc::new(Cell::new(0));
    let output = navigation.clone();
    ui.on_navigate(move |_| output.set(output.get() + 1));
    let submitted = Rc::new(Cell::new(0));
    let output = submitted.clone();
    ui.on_submit_dialog(move || output.set(output.get() + 1));
    click(&ui, "安装 Skills");
    assert_eq!(calls.get(), 1);
    assert!(ui.get_dialog_open());
    for label in [
        "项目管理",
        "搜索资源",
        "Resource 0",
        "Markdown 源码",
        "安装 Skills",
    ] {
        assert_eq!(
            element(&ui, label).accessible_enabled(),
            Some(false),
            "background tab target enabled: {label}"
        );
    }
    click(&ui, "项目管理");
    assert_eq!(navigation.get(), 0);
    for _ in 0..24 {
        key(&ui, Key::Tab);
        key(&ui, Key::Return);
    }
    assert_eq!(calls.get(), 1, "Tab escaped to toolbar action");
    assert_eq!(navigation.get(), 0, "Tab escaped to navigation");
    assert!(
        submitted.get() > 0,
        "native Tab must still reach modal controls"
    );
    let weak = ui.as_weak();
    ui.on_close_dialog(move || weak.upgrade().unwrap().set_dialog_open(false));
    key(&ui, Key::Escape);
    assert!(!ui.get_dialog_open());
    key(&ui, Key::Return);
    assert_eq!(calls.get(), 2, "focus should return to the dialog trigger");
}

#[test]
fn native_multiline_enter_and_loading_failed_capability_are_preserved() {
    let ui = window(1080., 680.);
    ui.set_fields(model(vec![field("Markdown 内容", 1)]));
    ui.set_dialog_open(true);
    click(&ui, "Markdown 内容");
    key(&ui, "a");
    key(&ui, Key::Return);
    key(&ui, "b");
    assert_eq!(ui.get_fields().row_data(0).unwrap().value, "a\nb");
    for state in [1, 2] {
        ui.set_dialog_load_state(state);
        assert_eq!(
            element(&ui, "Markdown 内容").accessible_enabled(),
            Some(false)
        );
        assert_eq!(element(&ui, "保存").accessible_enabled(), Some(false));
    }
    assert_eq!(element(&ui, "重试读取").accessible_enabled(), Some(true));
    ui.set_dialog_load_state(0);
    assert_eq!(element(&ui, "保存").accessible_enabled(), Some(true));
}

#[test]
fn importer_reason_cards_grow_and_footer_stays_visible_at_minimum_size() {
    let ui = window(1080., 680.);
    ui.set_dialog_importer(true);
    ui.set_dialog_title("导入资源".into());
    ui.set_import_host_names(model(vec!["全部宿主".into()]));
    ui.set_import_count(2);
    ui.set_import_rows(model(vec![ImportRow {
        id: "invalid".into(),
        name: "Invalid resource".into(),
        raw_name: "Invalid".into(),
        host_id: "host".into(),
        source: "/source/path".into(),
        reason: "此配置存在重复项，需要先处理。".repeat(30).into(),
        ..Default::default()
    }]));
    ui.set_dialog_open(true);
    let submit = element(&ui, "导入到 Chu");
    let p = submit.absolute_position();
    assert!(p.y + submit.size().height <= 652.);
    assert!(p.y >= 500., "fixed footer should stay at the modal bottom");
    ui.set_import_conflict("存在同名资源，请只选择一个来源。".into());
    assert_eq!(element(&ui, "导入到 Chu").accessible_enabled(), Some(false));
}

#[test]
fn overview_count_targets_and_host_config_location_are_explicit() {
    let ui = window(1080., 680.);
    ui.set_count_rows(model(vec![CountRow {
        label: "Skills".into(),
        value: 12,
        page: 2,
    }]));
    ui.set_visible_hosts(model(vec![HostRow {
        id: "host".into(),
        name: "宿主".into(),
        installed: true,
        ..Default::default()
    }]));
    let pages = Rc::new(RefCell::new(Vec::new()));
    let output = pages.clone();
    ui.on_navigate(move |page| output.borrow_mut().push(page));
    click(&ui, "Skills · 12 · 查看列表");
    assert_eq!(*pages.borrow(), vec![2]);
    let target = Rc::new(RefCell::new(None));
    let output = target.clone();
    ui.on_navigate_target(move |page, id, host| {
        *output.borrow_mut() = Some((page, id.to_string(), host.to_string()))
    });
    click(&ui, "配置 宿主");
    assert_eq!(*target.borrow(), Some((8, String::new(), "host".into())));
}

#[test]
fn structured_mcp_form_keeps_secrets_out_of_initial_fields_and_routes_actions() {
    let ui = window(1080., 680.);
    ui.set_dialog_mcp(true);
    ui.set_fields(model(vec![field("名称", 0)]));
    ui.set_mcp_args_stored_count(2);
    ui.set_mcp_args_action(0);
    ui.set_mcp_secret_stored(true);
    ui.set_mcp_secret_action(0);
    ui.set_mcp_env(model(vec![KeyValueRow {
        id: "TOKEN".into(),
        key: "TOKEN".into(),
        action: 0,
        stored: true,
        ..Default::default()
    }]));
    ui.set_dialog_open(true);
    assert!(ui.get_mcp_args().row_count() == 0);
    assert!(ui.get_mcp_env().row_data(0).unwrap().value.is_empty());
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "环境变量 TOKEN 新值")
            .next()
            .is_none()
    );
    assert_eq!(
        element(&ui, "环境变量 第 1 行 Key").accessible_read_only(),
        Some(true)
    );
    click(&ui, "环境变量 TOKEN · 设置新值");
    assert_eq!(ui.get_mcp_env().row_data(0).unwrap().action, 1);
    click(&ui, "环境变量 TOKEN 新值");
    key(&ui, "new-value");
    assert_eq!(ui.get_mcp_env().row_data(0).unwrap().value, "new-value");
    click(&ui, "环境变量 TOKEN · 移除值");
    assert_eq!(ui.get_mcp_env().row_data(0).unwrap().action, 2);
    let type_changes = Rc::new(RefCell::new(Vec::new()));
    let output = type_changes.clone();
    ui.on_mcp_type_changed(move |value| output.borrow_mut().push(value));
    click(&ui, "MCP 连接类型");
    key(&ui, Key::DownArrow);
    key(&ui, Key::Return);
    assert_eq!(ui.get_mcp_type(), 1);
    assert!(type_changes.borrow().contains(&1));
    assert!(
        ElementHandle::find_by_accessible_label(&ui, "启动命令")
            .next()
            .is_none()
    );
    assert_eq!(element(&ui, "服务地址").accessible_enabled(), Some(true));
}

#[test]
fn dirty_prompt_disables_outer_form_and_keeps_continue_or_discard_accessible() {
    let ui = window(1080., 680.);
    ui.set_fields(model(vec![field("名称", 0)]));
    ui.set_dialog_open(true);
    ui.set_dialog_discard_open(true);
    for label in ["名称", "保存", "取消", "关闭编辑窗口"] {
        assert_eq!(element(&ui, label).accessible_enabled(), Some(false));
    }
    let continued = Rc::new(Cell::new(0));
    let output = continued.clone();
    let weak = ui.as_weak();
    ui.on_continue_dialog(move || {
        output.set(output.get() + 1);
        weak.upgrade().unwrap().set_dialog_discard_open(false);
    });
    key(&ui, Key::Escape);
    assert_eq!(continued.get(), 1);
    assert!(!ui.get_dialog_discard_open());
    ui.set_dialog_discard_open(true);
    let discarded = Rc::new(Cell::new(0));
    let output = discarded.clone();
    ui.on_discard_dialog(move || output.set(output.get() + 1));
    click(&ui, "放弃更改");
    assert_eq!(discarded.get(), 1);
}

#[test]
fn short_confirmation_is_centered_complete_and_focuses_the_safe_cancel_action() {
    let ui = window(1080., 680.);
    ui.set_dialog_confirmation(true);
    ui.set_dialog_description(
        format!(
            "确认目标：\n{}\n不会覆盖已修改副本。",
            "/long/project/target/".repeat(70)
        )
        .into(),
    );
    ui.set_fields(model(vec![FormField {
        key: "cleanup".into(),
        label: "同时移除未修改副本".into(),
        kind: 2,
        ..Default::default()
    }]));
    ui.set_dialog_open(true);
    let dialog = element(&ui, "编辑窗口");
    let position = dialog.absolute_position();
    assert!(dialog.size().height < 624.);
    assert!((position.y - (680. - dialog.size().height) / 2.).abs() < 1.);
    let submit = element(&ui, "确认操作");
    assert!(
        submit.absolute_position().y + submit.size().height <= position.y + dialog.size().height
    );
    let canceled = Rc::new(Cell::new(0));
    let output = canceled.clone();
    ui.on_close_dialog(move || output.set(output.get() + 1));
    key(&ui, Key::Return);
    assert_eq!(
        canceled.get(),
        1,
        "danger confirmation must start on safe cancellation"
    );
}
