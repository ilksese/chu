mod common;
use chu::{
    App, controller::Controller, models::AgentInput, presenter::model, ui::AppWindow,
    write_observer,
};
use common::{run_git, write_skill};
use i_slint_backend_testing::ElementHandle;
use slint::{
    ComponentHandle, Model, Timer, TimerMode,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
const CHILD: &str = "CHU_SHUTDOWN_TEST_ROOT";
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
fn child(mode: &str, base: &Path) {
    let root = base.join("chu");
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let mut app = App::open(&root, &home).unwrap();
    for h in app.hosts() {
        chu::fs::atomic_write(
            Path::new(&h.config_path),
            if h.format == "toml" {
                b"model = \"fixture\"\n"
            } else {
                b"{}"
            },
        )
        .unwrap();
    }
    let is_preview = mode == "preview";
    let id = if is_preview {
        String::new()
    } else {
        app.add_agent(AgentInput {
            name: "closable".into(),
            prompt: "OLD".into(),
            ..Default::default()
        })
        .unwrap();
        let id = app.state.agents[0].id.clone();
        for host in ["claude", "codex"] {
            app.toggle_agent(&id, host, true).unwrap();
        }
        id
    };
    let central = if is_preview {
        PathBuf::new()
    } else {
        app.root
            .join("agents")
            .join(format!("{id}.md"))
            .canonicalize()
            .unwrap()
    };
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    let paused = Arc::new(AtomicBool::new(false));
    let once = paused.clone();
    let rollback = mode == "rollback";
    let observer: write_observer::WriteObserver = Arc::new(move |path| {
        if path == central && !once.swap(true, Ordering::SeqCst) {
            entered_tx.send(()).unwrap();
            release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .unwrap();
            if rollback {
                anyhow::bail!("隔离测试提交失败");
            }
        }
        Ok(())
    });
    i_slint_backend_testing::init_integration_test_with_system_time();
    let ui = AppWindow::new().unwrap();
    ui.window().set_size(slint::LogicalSize::new(1080., 680.));
    let controller =
        write_observer::with_write_observer(observer, || Controller::attach(&ui, app)).unwrap();
    ui.show().unwrap();
    let stage = Rc::new(Cell::new(0));
    let next = stage.clone();
    let weak = ui.as_weak();
    let started = Instant::now();
    let timer = Timer::default();
    let requested = Rc::new(Cell::new(None::<Instant>));
    let request_time = requested.clone();
    let loop_return = mode == "loop-return";
    let release = Arc::new(Mutex::new(Some(release_tx)));
    let release_in_loop = release.clone();
    if is_preview {
        let repo = base.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        run_git(&repo, &["init", "-b", "main"]);
        run_git(&repo, &["config", "user.email", "fixture@example.invalid"]);
        run_git(&repo, &["config", "user.name", "Fixture"]);
        write_skill(&repo.join("skill"), "preview-skill", "隔离来源");
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-m", "fixture"]);
        click(&ui, "Skills");
        click(&ui, "安装 Skills");
        let mut fields: Vec<_> = ui.get_fields().iter().collect();
        for f in &mut fields {
            if f.key == "repository" {
                f.value = repo.to_string_lossy().to_string().into();
            }
        }
        ui.set_fields(model(fields));
        click(&ui, "预览仓库");
    } else {
        click(&ui, "自定义 Agent");
        click(&ui, "编辑 Agent");
    }
    timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "关闭验收超时，阶段{}",
            next.get()
        );
        let ui = weak.upgrade().unwrap();
        if is_preview {
            if !ui.get_busy() && ui.get_candidates().row_count() > 0 {
                next.set(3);
                ui.window().dispatch_event(WindowEvent::CloseRequested);
            }
            return;
        }
        match next.get() {
            0 if !ui.get_busy() && ui.get_dialog_load_state() == 0 => {
                let mut fields: Vec<_> = ui.get_fields().iter().collect();
                for f in &mut fields {
                    if f.key == "prompt" {
                        f.value = "NEW".into();
                    }
                }
                ui.set_fields(model(fields));
                click(&ui, "保存");
                next.set(1);
            }
            1 if entered_rx.try_recv().is_ok() => {
                assert!(ui.get_busy());
                next.set(2);
                if loop_return {
                    slint::quit_event_loop().unwrap();
                } else {
                    ui.window().dispatch_event(WindowEvent::CloseRequested);
                    assert!(ui.get_closing());
                    // 重复原生关闭仍需等待同一个提交，不重启或打断事务。
                    ui.window().dispatch_event(WindowEvent::CloseRequested);
                    request_time.set(Some(Instant::now()));
                }
            }
            2 if !loop_return => {
                assert!(ui.get_closing());
                if request_time
                    .get()
                    .is_some_and(|t| t.elapsed() > Duration::from_millis(100))
                {
                    release_in_loop
                        .lock()
                        .unwrap()
                        .take()
                        .unwrap()
                        .send(())
                        .unwrap();
                    next.set(3);
                }
            }
            _ => {}
        }
    });
    let run_result = ui.run();
    if loop_return {
        assert_eq!(stage.get(), 2);
        // 事件循环意外停止时，shutdown 兜底必须等待真正结束，而非丢弃线程。
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            release.lock().unwrap().take().unwrap().send(()).unwrap();
        });
    } else {
        assert_eq!(stage.get(), 3);
    }
    controller.shutdown().unwrap();
    run_result.unwrap();
    drop(controller);
    let reopened = App::open(&root, &home).unwrap();
    if !is_preview {
        let agent = &reopened.state.agents[0];
        let expected = if rollback { "OLD" } else { "NEW" };
        assert_eq!(agent.prompt, expected);
        let central = std::fs::read(
            reopened
                .root
                .join("agents")
                .join(format!("{}.md", agent.id)),
        )
        .unwrap();
        for d in agent.deployments.values() {
            assert_eq!(std::fs::read(&d.target).unwrap(), central);
            assert_eq!(chu::fs::hash_path(Path::new(&d.target)), d.last_hash);
        }
    }
    println!("SHUTDOWN_COMPLETED_{mode}");
}
#[test]
fn native_shutdown_child() {
    let Some(root) = std::env::var_os(CHILD) else {
        return;
    };
    child(
        &std::env::var("CHU_SHUTDOWN_TEST_MODE").unwrap(),
        Path::new(&root),
    );
}
fn run(mode: &str) {
    let base = tempfile::tempdir().unwrap();
    let output = base.path().join("output.log");
    let file = std::fs::File::create(&output).unwrap();
    let mut process = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_shutdown_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, base.path())
        .env("CHU_SHUTDOWN_TEST_MODE", mode)
        .stdout(Stdio::from(file.try_clone().unwrap()))
        .stderr(Stdio::from(file))
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = process.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(25) {
            process.kill().unwrap();
            let _ = process.wait();
            panic!("关闭子进程超时：{mode}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let log = std::fs::read_to_string(output).unwrap();
    assert!(status.success(), "{mode}: {status}\n{log}");
    assert!(log.contains(&format!("SHUTDOWN_COMPLETED_{mode}")), "{log}");
    let sources = base.path().join("chu/sources");
    if sources.exists() {
        assert!(std::fs::read_dir(sources).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".chu-git-prepare-")
        }));
    }
}
#[test]
fn close_requested_waits_for_committed_files_and_state() {
    run("commit");
}
#[test]
fn close_requested_waits_for_transaction_rollback() {
    run("rollback");
}
#[test]
fn event_loop_return_joins_the_active_worker() {
    run("loop-return");
}
#[test]
fn close_requested_releases_preview_before_exit() {
    run("preview");
}
