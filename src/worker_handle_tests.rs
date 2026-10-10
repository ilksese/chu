use super::*;
use crate::{
    fs,
    worker::{self, Completion, Origin},
};
use serde_json::json;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
fn repository(base: &Path, home: &Path) -> PathBuf {
    let path = base.join("repository");
    std::fs::create_dir(&path).unwrap();
    let runner = crate::git_runner::GitRunner::new(home);
    for args in [
        vec!["init", "--template=", "-b", "main"],
        vec!["config", "user.name", "Fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
    ] {
        let mut command = vec![OsString::from("-C"), path.as_os_str().into()];
        command.extend(args.into_iter().map(OsString::from));
        runner.run(command, "隔离仓库").unwrap();
    }
    fs::atomic_write(
        &path.join("skill/SKILL.md"),
        b"---\nname: worker-preview\ndescription: fixture\n---\n\n# Fixture\n",
    )
    .unwrap();
    for args in [vec!["add", "."], vec!["commit", "-m", "fixture"]] {
        let mut command = vec![OsString::from("-C"), path.as_os_str().into()];
        command.extend(args.into_iter().map(OsString::from));
        runner.run(command, "隔离仓库").unwrap();
    }
    path
}
fn work(job: u64, method: &str, args: serde_json::Value, operation: Operation) -> Work {
    Work {
        job,
        method: method.into(),
        args,
        completion: Completion::Snapshot { close: false },
        origin: Origin::default(),
        operation,
    }
}
fn preparations(root: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(root.join("sources"))
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".chu-git-prepare-")
        })
        .map(|e| e.path())
        .collect()
}
#[test]
fn failed_reply_delivery_removes_its_token_even_with_another_app_owner() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("chu");
    let home = base.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let repository = repository(base.path(), &home);
    let app = crate::App::open(&root, &home).unwrap();
    let peer = crate::App::open(&root, &home).unwrap();
    let (mut worker, receiver) = worker::spawn(app).unwrap();
    drop(receiver);
    worker
        .send(work(
            1,
            "PreviewSkillsBound",
            json!([repository]),
            Operation::new(),
        ))
        .unwrap();
    let started = Instant::now();
    while !worker.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(10));
    }
    worker.join().unwrap();
    assert!(preparations(&root).is_empty());
    drop(peer);
}
#[test]
fn shutdown_cancels_queued_writes_and_raii_waits_for_the_current_commit() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("chu");
    let home = base.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let app = crate::App::open(&root, &home).unwrap();
    let (entered, entry) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let resume = Mutex::new(resume);
    let once = AtomicBool::new(false);
    let target = root.join("state.json");
    let observer: crate::write_observer::WriteObserver = Arc::new(move |path| {
        if path == target && !once.swap(true, Ordering::SeqCst) {
            entered.send(()).unwrap();
            resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        Ok(())
    });
    let (mut worker, receiver) =
        crate::write_observer::with_write_observer(observer, || worker::spawn(app)).unwrap();
    let current = Operation::new();
    let queued = Operation::new();
    worker
        .send(work(
            1,
            "AddMCP",
            json!([{"name":"one","type":"stdio","command":"echo"}]),
            current.clone(),
        ))
        .unwrap();
    entry.recv_timeout(Duration::from_secs(5)).unwrap();
    worker
        .send(work(
            2,
            "AddMCP",
            json!([{"name":"must-not-be-created","type":"stdio","command":"echo"}]),
            queued.clone(),
        ))
        .unwrap();
    worker.begin_shutdown();
    assert!(!current.is_cancelled());
    assert!(queued.is_cancelled());
    assert!(!worker.is_finished());
    let (done, finished) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        drop(worker);
        done.send(()).unwrap();
    });
    assert!(finished.recv_timeout(Duration::from_millis(75)).is_err());
    release.send(()).unwrap();
    finished.recv_timeout(Duration::from_secs(5)).unwrap();
    waiter.join().unwrap();
    let replies: Vec<_> = receiver.into_iter().collect();
    assert_eq!(replies.len(), 2);
    assert!(replies[0].error.is_none());
    assert_eq!(
        replies[1].feedback.severity,
        crate::feedback::Severity::Cancelled
    );
    let app = crate::App::open(root, home).unwrap();
    assert_eq!(app.state.mcps.len(), 1);
    assert_eq!(app.state.mcps[0].name, "one");
}
#[test]
fn dropping_a_worker_during_network_preparation_cancels_and_joins_it() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("chu");
    let home = base.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let (ready, entered) = mpsc::channel();
    let operation = Operation::new();
    let task = operation.clone();
    let operations: Operations = Arc::new(Mutex::new(std::collections::BTreeMap::from([(
        1,
        operation.clone(),
    )])));
    let (sender, receiver) = mpsc::channel::<Work>();
    let thread = std::thread::spawn(move || {
        let _app = crate::App::open(root, home).unwrap();
        task.set_phase("等待取消", crate::operation::Phase::Network);
        ready.send(()).unwrap();
        while task.check().is_ok() {
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(receiver);
    });
    let worker = Worker::new(sender, thread, operations);
    entered.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(worker);
    assert!(operation.is_cancelled());
}
