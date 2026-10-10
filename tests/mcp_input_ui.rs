mod common;
use chu::{
    App, McpInput, State,
    controller::Controller,
    mcp::{item_config, read_config, root_key},
    ui::AppWindow,
};
use common::Fixture;
use i_slint_backend_testing::ElementHandle;
use serde_json::json;
use slint::{
    ComponentHandle, Model, Timer, TimerMode,
    platform::{Key, PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    fs,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

const ARG: &str = "ui-argument-sentinel";
const SECRET: &str = "ui-secret-sentinel";
const BAD: &str = "ui-wrong-type-secret-sentinel";
const CASES: [(bool, &str); 6] = [
    (false, "stdio"),
    (false, "http"),
    (false, "sse"),
    (true, "stdio"),
    (true, "http"),
    (true, "sse"),
];

fn element(ui: &AppWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| {
            e.accessible_enabled() == Some(true) && e.size().width > 0. && e.size().height > 0.
        })
        .unwrap_or_else(|| panic!("缺少可用控件：{label}"))
}
fn click(ui: &AppWindow, label: &str) {
    // 先通过真实滚轮事件露出控件，再发送按下/抬起，避免无障碍动作绕过命中测试。
    for _ in 0..30 {
        let target = element(ui, label);
        let origin = target.absolute_position();
        let size = target.size();
        let center =
            slint::LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.);
        if ui.get_dialog_open() && !["保存", "新建", "编辑 MCP"].contains(&label) {
            let modal = ElementHandle::find_by_accessible_label(ui, "编辑窗口")
                .find(|e| e.size().width > 0. && e.size().height > 0.)
                .expect("缺少编辑窗口");
            let top = modal.absolute_position().y + 120.;
            let bottom = modal.absolute_position().y + modal.size().height - 90.;
            if center.y < top || center.y > bottom {
                ui.window().dispatch_event(WindowEvent::PointerScrolled {
                    position: slint::LogicalPosition::new(
                        modal.absolute_position().x + 300.,
                        (top + bottom) / 2.,
                    ),
                    delta_x: 0.,
                    delta_y: if center.y < top { 100. } else { -100. },
                });
                continue;
            }
        }
        ui.window()
            .dispatch_event(WindowEvent::PointerMoved { position: center });
        ui.window().dispatch_event(WindowEvent::PointerPressed {
            position: center,
            button: PointerEventButton::Left,
        });
        ui.window().dispatch_event(WindowEvent::PointerReleased {
            position: center,
            button: PointerEventButton::Left,
        });
        return;
    }
    panic!("控件未能滚动到可点击范围：{label}");
}
fn key(ui: &AppWindow, text: impl Into<slint::SharedString>) {
    let text = text.into();
    ui.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    ui.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
}
fn fill(ui: &AppWindow, label: &str, text: &str) {
    click(ui, label);
    for character in text.chars() {
        key(ui, character.to_string());
    }
}
fn set_then_remove(ui: &AppWindow, kind: &str, editing: bool) {
    if !editing {
        fill(ui, "名称", &format!("create-{kind}"));
        ui.set_mcp_type(match kind {
            "http" => 1,
            "sse" => 2,
            _ => 0,
        });
        fill(
            ui,
            if kind == "stdio" {
                "启动命令"
            } else {
                "服务地址"
            },
            if kind == "stdio" {
                "echo"
            } else {
                "https://fixture.invalid/mcp"
            },
        );
    } else {
        assert_eq!(ui.get_mcp_secret_action(), 0);
        assert!(ui.get_mcp_secret().is_empty());
        assert_eq!(ui.get_mcp_args().row_count(), 0);
    }
    if kind == "stdio" {
        click(ui, "启动参数 · 设置新值");
        assert_eq!(ui.get_mcp_args_action(), 1, "首次点击必须切换动作");
        click(ui, "添加参数");
        assert_eq!(ui.get_mcp_args().row_count(), 1, "首次点击必须添加参数");
        fill(ui, "启动参数 1", ARG);
        assert_eq!(ui.get_mcp_args().row_data(0).unwrap(), ARG);
        click(ui, "启动参数 · 设置新值");
        key(ui, Key::Tab);
        key(ui, Key::Space);
        assert_eq!(ui.get_mcp_args_action(), 2, "Tab/Space 必须激活移除");
        assert_eq!(ui.get_mcp_args().row_data(0).unwrap(), ARG);
    }
    let (title, map_key, map_kind) = if kind == "stdio" {
        ("环境变量", "TOKEN", "env")
    } else {
        ("请求 Headers", "X-Token", "headers")
    };
    if !editing {
        click(ui, &format!("添加 {title}"));
        let rows = if map_kind == "env" {
            ui.get_mcp_env()
        } else {
            ui.get_mcp_headers()
        };
        assert_eq!(rows.row_count(), 1);
        fill(ui, &format!("{title} 第 1 行 Key"), map_key);
    } else {
        click(ui, &format!("{title} {map_key} · 设置新值"));
    }
    fill(ui, &format!("{title} {map_key} 新值"), "ui-map-sentinel");
    click(ui, &format!("{title} {map_key} · 移除值"));
    let rows = if map_kind == "env" {
        ui.get_mcp_env()
    } else {
        ui.get_mcp_headers()
    };
    assert_eq!(rows.row_data(0).unwrap().action, 2);
    click(ui, "认证 Secret · 设置新值");
    assert_eq!(ui.get_mcp_secret_action(), 1, "首次点击必须设置 Secret");
    fill(ui, "认证 Secret 新值", SECRET);
    assert_eq!(ui.get_mcp_secret(), SECRET);
    click(ui, "认证 Secret · 设置新值");
    key(ui, Key::Tab);
    key(ui, Key::Return);
    assert_eq!(
        ui.get_mcp_secret_action(),
        2,
        "Tab/Enter 必须激活移除 Secret"
    );
    assert_eq!(ui.get_mcp_secret(), SECRET);
}
fn invalid_json_never_echoes(ui: &AppWindow, root: &Path) {
    let before = fs::read(root.join("state.json")).unwrap();
    click(ui, "显示 MCP 高级 JSON");
    assert!(ui.get_mcp_show_advanced());
    for (field, value) in [
        ("args", json!(BAD)),
        ("args", json!([[BAD]])),
        ("secret", json!({"token": BAD})),
        ("env", json!({"TOKEN": [BAD]})),
        ("headers", json!({"Authorization": {"token": BAD}})),
    ] {
        ui.set_mcp_advanced(json!({field: value}).to_string().into());
        click(ui, "保存");
        assert!(ui.get_dialog_open());
        assert_eq!(ui.get_dialog_error(), format!("高级配置 {field} 格式无效"));
        assert!(!ui.get_dialog_error().contains(BAD));
        assert!(!ui.get_notice().contains(BAD));
        assert!(!ui.get_busy());
        assert_eq!(fs::read(root.join("state.json")).unwrap(), before);
    }
    click(ui, "显示 MCP 高级 JSON");
    assert!(!ui.get_mcp_show_advanced());
}

#[test]
fn actual_mcp_callbacks_and_keyboard_remove_drafts_before_saving_and_redact_errors() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let mut f = Fixture::new();
    for kind in ["stdio", "http", "sse"] {
        f.app
            .add_mcp(McpInput {
                name: format!("edit-{kind}"),
                r#type: kind.into(),
                command: "echo".into(),
                endpoint: "https://fixture.invalid/mcp".into(),
                args: vec!["existing-argument".into()],
                env: [("TOKEN".into(), "existing-env".into())].into(),
                headers: [("X-Token".into(), "existing-header".into())].into(),
                secret: "existing-secret".into(),
                ..Default::default()
            })
            .unwrap();
        let id = f.app.state.mcps.last().unwrap().id.clone();
        for host in f
            .app
            .hosts()
            .into_iter()
            .filter(|h| kind == "stdio" || h.id != "jcode")
        {
            f.app.toggle_mcp(&id, &host.id, true).unwrap();
        }
    }
    let root = f.app.root.clone();
    let home = f.app.home.clone();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1360., 860.));
    let controller = Controller::attach(&ui, f.app).unwrap();
    ui.show().unwrap();
    click(&ui, "MCP 服务");
    let stage = Rc::new(Cell::new((0usize, 0u8)));
    let next = stage.clone();
    let weak = ui.as_weak();
    let test_root = root.clone();
    let started = Instant::now();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(30), move || {
        let ui = weak.upgrade().unwrap();
        let (case, phase) = next.get();
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "MCP 交互超时：{case}/{phase}"
        );
        if ui.get_busy() {
            return;
        }
        if case == CASES.len() {
            ui.hide().unwrap();
            slint::quit_event_loop().unwrap();
            return;
        }
        let (editing, kind) = CASES[case];
        match phase {
            0 => {
                if editing {
                    click(&ui, &format!("edit-{kind}"));
                    click(&ui, "编辑 MCP");
                } else {
                    click(&ui, "新建");
                }
                assert!(ui.get_dialog_open(), "新建或编辑必须响应首次点击");
                next.set((case, 1));
            }
            1 => {
                if ui.get_dialog_load_state() != 0 {
                    return;
                }
                set_then_remove(&ui, kind, editing);
                invalid_json_never_echoes(&ui, &test_root);
                click(&ui, "保存");
                next.set((case, 2));
            }
            2 => {
                assert!(!ui.get_dialog_open(), "保存失败：{}", ui.get_dialog_error());
                assert!(!ui.get_notice_error());
                for sentinel in [ARG, SECRET, BAD, "ui-map-sentinel"] {
                    assert!(!ui.get_notice().contains(sentinel));
                }
                next.set((case + 1, 0));
            }
            _ => unreachable!(),
        }
    });
    slint::run_event_loop().unwrap();
    assert_eq!(stage.get(), (CASES.len(), 0));
    drop(controller);
    let mut app = App::open(root, home).unwrap();
    let persisted: State =
        serde_json::from_slice(&fs::read(app.root.join("state.json")).unwrap()).unwrap();
    assert_eq!(persisted, app.state);
    assert_eq!(app.state.mcps.len(), 6);
    for item in &app.state.mcps {
        assert!(item.secret.is_empty());
        assert!(!item.native_formats);
        if item.name.starts_with("create-") {
            assert!(item.args.is_empty());
            assert!(item.env.is_empty());
            assert!(item.headers.is_empty());
        } else {
            assert_eq!(
                item.args,
                if item.r#type == "stdio" {
                    vec![]
                } else {
                    vec!["existing-argument"]
                }
            );
            assert_eq!(item.env.is_empty(), item.r#type == "stdio");
            assert_eq!(item.headers.is_empty(), item.r#type != "stdio");
            for (host_id, deployment) in &item.deployments {
                assert!(Path::new(&deployment.target).starts_with(f.temp.path()));
                let host = app.host(host_id).unwrap();
                let config = read_config(Path::new(&deployment.target), &host.format).unwrap();
                assert_eq!(
                    config[root_key(host_id, &config)][&item.name],
                    item_config(item)
                );
                assert_eq!(
                    deployment.last_hash,
                    chu::fs::hash_path(Path::new(&deployment.target))
                );
            }
        }
    }
    let response = serde_json::to_string(&app.snapshot().unwrap()).unwrap();
    for sentinel in [
        ARG,
        SECRET,
        BAD,
        "ui-map-sentinel",
        "existing-argument",
        "existing-header",
        "existing-env",
        "existing-secret",
    ] {
        assert!(!response.contains(sentinel));
    }
}
