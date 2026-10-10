mod common;
use chu::{controller::Controller, presenter::model, ui::AppWindow};
use common::*;
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, Model, Timer, TimerMode,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
fn click(ui: &AppWindow, name: &str) {
    let element = ElementHandle::find_by_accessible_label(ui, name)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("缺少可用控件：{name}"));
    let origin = element.absolute_position();
    let size = element.size();
    let position =
        slint::LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.);
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
fn key(ui: &AppWindow, text: slint::SharedString) {
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn fill(ui: &AppWindow, name: &str, value: &str) {
    click(ui, name);
    for c in value.chars() {
        key(ui, c.to_string().into());
    }
}
#[test]
fn draft_loading_navigation_and_resource_edits_keep_user_context() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let mut f = Fixture::new();
    f.skill("alpha");
    f.skill("beta");
    f.app
        .create_prompt("rules", "# Original\n\nKeep this content.")
        .unwrap();
    f.app.add_mcp(stdio("mcp-one")).unwrap();
    f.app
        .add_agent(chu::AgentInput {
            name: "agent-one".into(),
            prompt: "Initial agent prompt".into(),
            ..Default::default()
        })
        .unwrap();
    let root = f.app.root.clone();
    let home = f.app.home.clone();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    let controller = Controller::attach(&ui, f.app).unwrap();
    ui.show().unwrap();
    click(&ui, "Skills");
    click(&ui, "beta");
    let stage = Rc::new(Cell::new(0));
    let next = stage.clone();
    let weak = ui.as_weak();
    let timer = Timer::default();
    let started = Instant::now();
    timer.start(TimerMode::Repeated, Duration::from_millis(30), move || {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "交互回归超时，阶段{}",
            next.get()
        );
        let ui = weak.upgrade().unwrap();
        if ui.get_busy() {
            return;
        }
        match next.get() {
            0 => {
                assert_eq!(ui.get_selected_name(), "beta");
                click(&ui, "设置");
                click(&ui, "Skills");
                assert_eq!(ui.get_selected_name(), "beta");
                click(&ui, "提示词");
                click(&ui, "编辑提示词");
            }
            1 => {
                if ui.get_dialog_load_state() != 0 {
                    return;
                }
                assert!(
                    ui.get_fields()
                        .iter()
                        .any(|f| f.key == "content" && f.value.contains("Keep this content"))
                );
                fill(&ui, "Markdown 内容", "Changed");
                key(&ui, Key::Escape.into());
                assert!(ui.get_dialog_open());
                assert!(ui.get_dialog_discard_open());
                click(&ui, "继续编辑");
                assert!(!ui.get_dialog_discard_open());
                assert!(
                    ui.get_fields()
                        .iter()
                        .any(|f| f.key == "content" && f.value.contains("Changed"))
                );
                click(&ui, "取消");
                assert!(ui.get_dialog_discard_open());
                click(&ui, "放弃更改");
                assert!(!ui.get_dialog_open());
                click(&ui, "MCP 服务");
                click(&ui, "编辑 MCP");
            }
            2 => {
                if ui.get_dialog_load_state() != 0 {
                    return;
                }
                assert!(ui.get_mcp_args().row_count() == 0);
                assert_eq!(ui.get_mcp_args_action(), 0);
                assert!(ui.get_mcp_args_stored_count() > 0);
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                for f in &mut fields {
                    if f.key == "description" {
                        f.value = "Updated safely".into();
                    }
                }
                ui.set_fields(model(fields));
                click(&ui, "保存");
            }
            3 => {
                assert!(!ui.get_dialog_open());
                assert_eq!(ui.get_selected_name(), "mcp-one");
                assert!(!ui.get_notice_error());
                click(&ui, "自定义 Agent");
                click(&ui, "编辑 Agent");
            }
            4 => {
                if ui.get_dialog_load_state() != 0 {
                    return;
                }
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                for f in &mut fields {
                    if f.key == "prompt" {
                        f.value = "Updated agent prompt".into();
                    }
                }
                ui.set_fields(model(fields));
                click(&ui, "保存");
            }
            5 => {
                assert!(!ui.get_dialog_open());
                click(&ui, "删除 Agent");
            }
            6 => {
                if ui.get_dialog_load_state() != 0 {
                    return;
                }
                assert!(ui.get_dialog_description().contains("agent-one"));
                click(&ui, "确认操作");
            }
            7 => {
                assert_eq!(ui.get_rows().row_count(), 0);
                ui.hide().unwrap();
                slint::quit_event_loop().unwrap();
            }
            _ => return,
        }
        next.set(next.get() + 1);
    });
    slint::run_event_loop().unwrap();
    assert_eq!(stage.get(), 8);
    drop(controller);
    let mut app = chu::App::open(root, home).unwrap();
    let snapshot = app.snapshot().unwrap();
    assert!(snapshot.agents.is_empty());
    assert_eq!(app.state.mcps[0].description, "Updated safely");
    let prompt_id = app.snapshot().unwrap().prompts[0].id.clone();
    assert_eq!(
        app.read_prompt(&prompt_id).unwrap(),
        "# Original\n\nKeep this content."
    );
}
