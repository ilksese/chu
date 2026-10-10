mod common;
use chu::{controller::Controller, ui::AppWindow};
use common::*;
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, Timer, TimerMode,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};
fn click(ui: &AppWindow, label: &str) {
    let e = ElementHandle::find_by_accessible_label(ui, label)
        .find(|e| e.accessible_enabled() == Some(true))
        .unwrap_or_else(|| panic!("缺少可用控件：{label}"));
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
fn failed_original_read_cannot_submit_an_empty_replacement() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let mut f = Fixture::new();
    f.app.create_reference("unreadable", "original").unwrap();
    let id = f.app.snapshot().unwrap().references[0].id.clone();
    let root = f.app.root.clone();
    let home = f.app.home.clone();
    let ui = AppWindow::new().unwrap();
    let c = Controller::attach(&ui, f.app).unwrap();
    ui.show().unwrap();
    // 资源名仍可发现，但正文不是 UTF-8；读取失败与合法空文件必须区分。
    chu::fs::atomic_write(&root.join("references").join(&id), &[0xff, 0xfe, 0xfd]).unwrap();
    click(&ui, "References");
    click(&ui, "编辑 Reference");
    let stage = Rc::new(Cell::new(0));
    let next = stage.clone();
    let weak = ui.as_weak();
    let started = Instant::now();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(25), move || {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "加载失败回归超时"
        );
        let ui = weak.upgrade().unwrap();
        if ui.get_busy() {
            return;
        }
        if next.get() == 0 {
            assert_eq!(ui.get_dialog_load_state(), 2);
            assert!(!ui.get_dialog_can_submit());
            assert!(
                ElementHandle::find_by_accessible_label(&ui, "保存")
                    .all(|e| e.accessible_enabled() != Some(true))
            );
            ui.invoke_submit_dialog();
            assert!(ui.get_dialog_open());
            click(&ui, "取消");
            assert!(!ui.get_dialog_open());
            ui.hide().unwrap();
            slint::quit_event_loop().unwrap();
            next.set(1);
        }
    });
    slint::run_event_loop().unwrap();
    assert_eq!(stage.get(), 1);
    drop(c);
    let _ = chu::App::open(root.clone(), home).unwrap();
    assert_eq!(
        std::fs::read(root.join("references").join(id)).unwrap(),
        vec![0xff, 0xfe, 0xfd]
    );
}
