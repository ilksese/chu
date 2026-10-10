mod common;
use chu::{
    controller::Controller,
    editor,
    presenter::model,
    resource_edit::{McpPatch, Patch},
    ui::{AppWindow, KeyValueRow},
};
use common::*;
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton, WindowEvent},
};
fn click(ui: &AppWindow, label: &str) {
    let e = ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("未找到控件：{label}"));
    let p = e.absolute_position();
    let s = e.size();
    let position = slint::LogicalPosition::new(p.x + s.width / 2., p.y + s.height / 2.);
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
#[test]
fn filtering_temporarily_changes_selection_without_leaking_reading_mode() {
    i_slint_backend_testing::init_no_event_loop();
    let mut f = Fixture::new();
    f.app.create_reference("alpha", "# Alpha").unwrap();
    f.app.create_reference("beta", "# Beta").unwrap();
    let ui = AppWindow::new().unwrap();
    let controller = Controller::attach(&ui, f.app).unwrap();
    ui.show().unwrap();
    click(&ui, "References");
    assert_eq!(ui.get_selected_name(), "alpha");
    click(&ui, "Markdown 源码");
    assert!(ui.get_markdown_source());
    click(&ui, "搜索资源");
    for c in "beta".chars() {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: c.to_string().into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: c.to_string().into(),
        });
    }
    assert_eq!(ui.get_selected_name(), "beta");
    assert!(!ui.get_markdown_source());
    click(&ui, "搜索资源");
    for _ in 0..4 {
        ui.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Backspace.into(),
        });
        ui.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Backspace.into(),
        });
    }
    assert_eq!(ui.get_query(), "");
    assert_eq!(ui.get_selected_name(), "alpha");
    assert!(ui.get_markdown_source());
    drop(controller);
}
#[test]
fn inactive_transport_drafts_do_not_block_or_change_mcp_edits() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    let state = chu::presenter::ViewState {
        page: 3,
        ..Default::default()
    };
    let mut dialog = editor::open(
        &ui,
        &state,
        "UpdateMCP",
        "existing",
        serde_json::Value::Null,
    );
    ui.set_mcp_type(1);
    ui.set_mcp_endpoint("https://fixture.invalid/mcp".into());
    let mut fields: Vec<_> = ui.get_fields().iter().collect();
    for f in &mut fields {
        if f.key == "name" {
            f.value = "Existing".into();
        }
    }
    ui.set_fields(model(fields));
    dialog.loaded = true;
    dialog.rebase(&ui);
    ui.set_mcp_env(model(vec![KeyValueRow {
        key: "".into(),
        value: "inapplicable-value".into(),
        action: 1,
        stored: false,
        ..Default::default()
    }]));
    ui.set_mcp_args(model(vec!["hidden-argument".into()]));
    ui.set_mcp_args_action(1);
    let (_, args) = editor::command(&ui, &dialog).unwrap();
    let patch: McpPatch = serde_json::from_value(args[1].clone()).unwrap();
    assert!(patch.env.is_empty());
    assert!(matches!(patch.args, Patch::Keep));
    ui.set_mcp_type(0);
    ui.set_mcp_command("echo".into());
    assert!(
        editor::command(&ui, &dialog).is_err(),
        "切回适用类型时必须校验未填写的Key"
    );
}
