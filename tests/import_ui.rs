mod common;
use chu::{controller::Controller, mcp::*, ui::AppWindow};
use common::*;
use i_slint_backend_testing::ElementHandle;
use serde_json::json;
use slint::{
    ComponentHandle, Model, Timer, TimerMode,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};
fn click(ui: &AppWindow, label: &str) {
    let control = ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("missing enabled control: {label}"));
    let origin = control.absolute_position();
    let size = control.size();
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
#[test]
fn import_modals_select_resources_apply_hosts_and_update_managed_pages_with_real_pointer_events() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let f = Fixture::new();
    let claude = f.app.host("claude").unwrap();
    let source = Path::new(&claude.skill_path).join("host-skill");
    write_skill(&source, "host-skill", "Imported fixture");
    write_skill(
        &Path::new(&claude.skill_path).join("skip-skill"),
        "skip-skill",
        "Do not import",
    );
    let mut root = read_config(Path::new(&claude.config_path), "json").unwrap();
    root["mcpServers"] = json!({"local":{"command":"echo","args":["fixture"],"env":{"SECRET":"synthetic-import-secret"}},"remote":{"type":"http","url":"https://fixture.invalid/mcp"}});
    write_config(Path::new(&claude.config_path), "json", &root).unwrap();
    let state_root = f.app.root.clone();
    let home = f.app.home.clone();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    let controller = Controller::attach(&ui, f.app).unwrap();
    ui.show().unwrap();
    click(&ui, "Skills");
    assert_eq!(
        ui.get_rows().row_count(),
        0,
        "host resources must not appear by default"
    );
    click(&ui, "导入");
    assert!(ui.get_dialog_importer());
    let stage = Rc::new(Cell::new(0));
    let output = stage.clone();
    let weak = ui.as_weak();
    let timer = Timer::default();
    let started = Instant::now();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "import UI test timed out"
        );
        let ui = weak.upgrade().unwrap();
        if ui.get_busy() {
            return;
        }
        match output.get() {
            0 => {
                assert_eq!(ui.get_import_rows().row_count(), 2);
                assert_eq!(ui.get_import_count(), 0);
                assert!(!ui.get_has_apply_targets());
                assert!(
                    ElementHandle::find_by_accessible_label(&ui, "导入到 Chu")
                        .any(|e| e.accessible_enabled() == Some(false))
                );
                click(&ui, "host-skill · Claude Code");
                assert_eq!(ui.get_import_count(), 1);
                click(&ui, "立即应用到 Codex");
                assert!(ui.get_has_apply_targets());
                click(&ui, "导入并应用");
                assert!(ui.get_busy());
                output.set(1);
            }
            1 => {
                assert!(!ui.get_dialog_open());
                assert_eq!(ui.get_rows().row_count(), 1);
                assert_eq!(ui.get_selected_name(), "host-skill");
                assert!(
                    ui.get_deployments()
                        .iter()
                        .any(|h| h.id == "codex" && h.enabled)
                );
                click(&ui, "MCP 服务");
                assert_eq!(ui.get_rows().row_count(), 0);
                click(&ui, "导入");
                output.set(2);
            }
            2 => {
                assert_eq!(ui.get_import_rows().row_count(), 2);
                click(&ui, "立即应用到 jcode");
                assert!(
                    ui.get_apply_hosts()
                        .iter()
                        .any(|h| h.id == "jcode" && h.checked)
                );
                click(&ui, "remote · Claude Code");
                assert!(
                    ui.get_apply_hosts()
                        .iter()
                        .any(|h| h.id == "jcode" && !h.available && !h.checked)
                );
                assert!(
                    ElementHandle::find_by_accessible_label(&ui, "立即应用到 jcode")
                        .all(|e| e.accessible_enabled() != Some(true))
                );
                click(&ui, "local · Claude Code");
                assert_eq!(ui.get_import_count(), 2);
                click(&ui, "立即应用到 Codex");
                let submit = ElementHandle::find_by_accessible_label(&ui, "导入并应用")
                    .find(|e| e.accessible_enabled() == Some(true))
                    .unwrap();
                let p = submit.absolute_position();
                assert!(
                    p.y >= 0. && p.y + submit.size().height <= 680.,
                    "import/apply action must remain visible at minimum window size"
                );
                click(&ui, "导入并应用");
                output.set(3);
            }
            3 => {
                assert!(!ui.get_dialog_open());
                assert_eq!(ui.get_rows().row_count(), 2);
                assert!(
                    ui.get_deployments()
                        .iter()
                        .any(|h| h.id == "codex" && h.enabled)
                );
                click(&ui, "导入");
                output.set(4);
            }
            4 => {
                assert_eq!(ui.get_import_count(), 0);
                assert!(
                    ui.get_import_rows()
                        .iter()
                        .all(|r| !r.available && !r.checked)
                );
                assert!(ui.get_import_rows().iter().all(|r| {
                    !format!("{} {} {}", r.name, r.description, r.source)
                        .contains("synthetic-import-secret")
                }));
                click(&ui, "取消");
                assert!(!ui.get_dialog_open());
                slint::quit_event_loop().unwrap();
                output.set(5);
            }
            _ => {}
        }
    });
    slint::run_event_loop().unwrap();
    assert_eq!(stage.get(), 5);
    drop(controller);
    let mut app = chu::App::open(state_root, home).unwrap();
    let snapshot = app.snapshot().unwrap();
    assert_eq!(snapshot.skills.iter().filter(|s| s.managed).count(), 1);
    assert_eq!(snapshot.mcps.iter().filter(|m| m.managed).count(), 2);
    assert!(
        snapshot
            .mcps
            .iter()
            .filter(|m| m.managed)
            .all(|m| m.enabled_on["codex"] && !m.enabled_on["jcode"])
    );
    assert!(source.join("SKILL.md").is_file());
    assert!(!app.root.join("skills/skip-skill").exists());
}
