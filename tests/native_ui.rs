mod common;
use chu::{controller::Controller, presenter::model, ui::AppWindow};
use common::*;
use i_slint_backend_testing::ElementHandle;
use sha2::{Digest, Sha256};
use slint::{
    ComponentHandle, Model, Timer, TimerMode,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
#[test]
fn native_pages_and_dialogs_use_the_real_worker() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let mut fixture = Fixture::new();
    fixture.skill("native-tdd");
    fixture
        .app
        .add_agent(chu::models::AgentInput {
            name: "native-agent".into(),
            prompt: "Native agent content".into(),
            ..chu::models::AgentInput::default()
        })
        .unwrap();
    let mut server = Server::new(vec![
        (200, r#"{"data":[{"id":"native-model-a"}]}"#.into(), vec![]),
        (200, r#"{"data":[{"id":"native-model-b"}]}"#.into(), vec![]),
    ]);
    let server_url = server.url.clone();
    let repository = repo();
    let source_url = "https://github.com/fixture/native-skills";
    let cache = fixture.app.root.join("sources").join(format!(
        "{:x}",
        Sha256::digest(format!("{source_url}\nmain").as_bytes())
    ));
    chu::fs::mkdir(cache.parent().unwrap()).unwrap();
    run_git(
        fixture.temp.path(),
        &[
            "clone",
            "--branch",
            "main",
            repository.path().to_str().unwrap(),
            cache.to_str().unwrap(),
        ],
    );
    let root = fixture.app.root.clone();
    let home = fixture.app.home.clone();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    let controller = Controller::attach(&ui, fixture.app).unwrap();
    ui.show().unwrap();
    for (page, label) in chu::presenter::TITLES.iter().enumerate() {
        click(&ui, label);
        assert_eq!(ui.get_page(), page as i32);
    }
    click(&ui, chu::presenter::TITLES[6]);
    click(
        &ui,
        if ui.get_page() == 2 {
            "安装 Skills"
        } else {
            "新建"
        },
    );
    assert!(ui.get_dialog_open());
    let mut fields: Vec<_> = ui.get_fields().iter().collect();
    for f in &mut fields {
        f.value = if f.key == "name" {
            "native-rules"
        } else {
            "# Native rules\nWritten by the real worker."
        }
        .into();
    }
    ui.set_fields(model(fields));
    click(&ui, "保存");
    assert!(ui.get_busy());
    let stage = Rc::new(Cell::new(0));
    let state = stage.clone();
    let weak = ui.as_weak();
    let started = Instant::now();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        let ui = weak.upgrade().unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "worker test timed out"
        );
        if ui.get_busy() {
            return;
        }
        match state.get() {
            0 => {
                assert!(!ui.get_dialog_open());
                assert_eq!(ui.get_rows().row_count(), 1);
                click(&ui, "Claude Code 部署");
                state.set(1);
            }
            1 => {
                assert!(
                    ui.get_deployments()
                        .iter()
                        .any(|h| h.id == "claude" && h.enabled)
                );
                click(&ui, &ui.get_action_one());
                if ui.get_busy() {
                    state.set(5);
                    return;
                }
                assert!(ui.get_dialog_open());
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                for f in &mut fields {
                    if f.key == "content" {
                        f.value = "Updated through Slint".into();
                    }
                }
                ui.set_fields(model(fields));
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(2);
            }
            2 => {
                click(&ui, &ui.get_action_three());
                assert!(ui.get_dialog_confirmation());
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(3);
            }
            3 => {
                assert_eq!(ui.get_rows().row_count(), 0);
                click(&ui, chu::presenter::TITLES[5]);
                state.set(6);
            }
            5 => {
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                assert!(
                    fields
                        .iter()
                        .any(|f| f.key == "content" && f.value.as_str().contains("Native rules"))
                );
                for f in &mut fields {
                    if f.key == "content" {
                        f.value = "Updated through Slint".into();
                    }
                }
                ui.set_fields(model(fields));
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(2);
            }
            6 => {
                if !ui.get_selected_content().contains("Native agent content") {
                    return;
                }
                click(&ui, "Markdown 源码");
                let body = ElementHandle::find_by_accessible_label(&ui, "native-agent 内容")
                    .find(|e| {
                        e.accessible_value()
                            .is_some_and(|v| v.contains("Native agent content"))
                    })
                    .expect("agent source body is visible");
                assert!(
                    body.size().height >= 80.,
                    "native detail body must remain readable at minimum window size"
                );
                click(&ui, chu::presenter::TITLES[4]);
                click(
                    &ui,
                    if ui.get_page() == 2 {
                        "安装 Skills"
                    } else {
                        "新建"
                    },
                );
                fill_draft(
                    &ui,
                    &[("name", "Native provider"), ("baseUrl", &server_url)],
                );
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(7);
            }
            7 => {
                assert!(ui.get_selected_models().contains("native-model-a"));
                click(&ui, &ui.get_action_two());
                state.set(8);
            }
            8 => {
                assert!(ui.get_selected_models().contains("native-model-b"));
                click(&ui, &ui.get_action_three());
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(9);
            }
            9 => {
                click(&ui, chu::presenter::TITLES[3]);
                click(
                    &ui,
                    if ui.get_page() == 2 {
                        "安装 Skills"
                    } else {
                        "新建"
                    },
                );
                fill_draft(&ui, &[("name", "native-mcp"), ("command", "echo")]);
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(10);
            }
            10 => {
                assert_eq!(ui.get_selected_name(), "native-mcp");
                click(&ui, "Claude Code 部署");
                state.set(11);
            }
            11 => {
                assert!(
                    ui.get_deployments()
                        .iter()
                        .any(|d| d.id == "claude" && d.enabled)
                );
                click(&ui, &ui.get_action_two());
                state.set(12);
            }
            12 => {
                assert!(ui.get_notice().contains("命令"));
                click(&ui, chu::presenter::TITLES[2]);
                click(
                    &ui,
                    if ui.get_page() == 2 {
                        "安装 Skills"
                    } else {
                        "新建"
                    },
                );
                fill_draft(&ui, &[("repository", "fixture/native-skills")]);
                click(&ui, "预览仓库");
                state.set(13);
            }
            13 => {
                assert_eq!(ui.get_candidates().row_count(), 1);
                assert!(ui.get_candidates().iter().all(|c| c.checked));
                click(
                    &ui,
                    if ui.get_dialog_installer() {
                        "安装所选"
                    } else if ui.get_dialog_confirmation() {
                        "确认操作"
                    } else {
                        "保存"
                    },
                );
                state.set(14);
            }
            14 => {
                assert!(ui.get_rows().iter().any(|r| r.name == "tdd"));
                let search = ElementHandle::find_by_element_type_name(&ui, "LineEdit")
                    .find(|e| {
                        e.accessible_placeholder_text()
                            .is_some_and(|p| p.starts_with("搜索名称"))
                    })
                    .expect("search field");
                pointer_click(&ui, &search);
                for c in "fixture/native-skills".chars() {
                    ui.window().dispatch_event(WindowEvent::KeyPressed {
                        text: c.to_string().into(),
                    });
                    ui.window().dispatch_event(WindowEvent::KeyReleased {
                        text: c.to_string().into(),
                    });
                }
                assert_eq!(ui.get_query(), "fixture/native-skills");
                assert_eq!(ui.get_rows().row_count(), 1);
                click(&ui, "检查更新");
                state.set(15);
            }
            15 => {
                assert!(ui.get_notice().contains("更新检查完成"));
                click(&ui, &ui.get_action_one());
                state.set(16);
            }
            16 => {
                assert_eq!(ui.get_selected_name(), "tdd");
                slint::quit_event_loop().unwrap();
                state.set(4);
            }
            _ => {}
        }
    });
    slint::run_event_loop().unwrap();
    assert_eq!(stage.get(), 4);
    drop(controller);
    server.finish();
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    let mut reopened = chu::App::open(root, home).unwrap();
    let snapshot = reopened.snapshot().unwrap();
    assert!(snapshot.prompts.is_empty());
    assert!(snapshot.providers.is_empty());
    assert!(snapshot.mcps[0].enabled_on["claude"]);
    assert!(snapshot.skills.iter().any(|s| s.name == "tdd" && s.tracked));
}

fn fill_draft(ui: &AppWindow, values: &[(&str, &str)]) {
    let mut fields: Vec<_> = ui.get_fields().iter().collect();
    for field in &mut fields {
        if let Some((_, value)) = values.iter().find(|(key, _)| *key == field.key.as_str()) {
            field.value = (*value).into();
        }
    }
    ui.set_fields(model(fields));
    for (key, value) in values {
        if *key == "command" {
            ui.set_mcp_command((*value).into());
        }
        if *key == "endpoint" {
            ui.set_mcp_endpoint((*value).into());
        }
    }
}

fn click(ui: &AppWindow, label: &str) {
    let element = ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("missing enabled pointer target: {label}"));
    pointer_click(ui, &element);
}
fn pointer_click(ui: &AppWindow, element: &ElementHandle) {
    // This worker test uses real time. Do not advance mock time with mock_single_click.
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
