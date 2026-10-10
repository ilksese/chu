mod common;
use chu::{
    fs::{atomic_write, hash_path, mkdir},
    git_runner::{GitRunner, with_git_executable},
    operation::{self, Operation, Phase, with_operation},
};
use common::Fixture;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};

#[test]
fn operation_context_is_scoped_and_cli_checks_remain_no_ops() {
    assert!(operation::current().is_none());
    operation::check().unwrap();
    operation::begin_commit().unwrap();
    let outer = Operation::new();
    let inner = Operation::new();
    with_operation(&outer, || {
        with_operation(&inner, || {
            operation::begin_commit().unwrap();
            assert_eq!(inner.phase(), Phase::Commit);
        });
        assert!(outer.cancel());
        assert!(operation::check().is_err());
    });
    assert!(operation::current().is_none());
    assert!(outer.is_cancelled());
    assert!(!inner.is_cancelled());
    assert!(!inner.cancel());
}

#[test]
fn cancel_and_begin_commit_have_exactly_one_atomic_winner() {
    for _ in 0..64 {
        let operation = Operation::new();
        let barrier = Arc::new(Barrier::new(3));
        let cancel_op = operation.clone();
        let cancel_barrier = barrier.clone();
        let cancel = thread::spawn(move || {
            cancel_barrier.wait();
            cancel_op.cancel()
        });
        let commit_op = operation.clone();
        let commit_barrier = barrier.clone();
        let commit = thread::spawn(move || {
            commit_barrier.wait();
            commit_op.begin_commit().is_ok()
        });
        barrier.wait();
        let cancelled = cancel.join().unwrap();
        let committed = commit.join().unwrap();
        assert_ne!(cancelled, committed);
        assert_eq!(operation.is_cancelled(), cancelled);
        if committed {
            operation.set_phase("不能退回准备阶段", Phase::Preparing);
            assert_eq!(operation.phase(), Phase::Commit);
            assert!(!operation.cancel());
        } else {
            assert!(operation.check().is_err());
        }
        operation.finish();
        assert_eq!(operation.phase(), Phase::Finished);
        assert!(!operation.cancel());
    }
}

#[cfg(unix)]
fn fake_git(f: &Fixture, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = f.temp.path().join(name);
    atomic_write(&path, format!("#!/bin/sh\n{body}\n").as_bytes()).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
fn wait_file(path: &Path) {
    let limit = Instant::now() + Duration::from_secs(3);
    while !path.exists() {
        assert!(Instant::now() < limit, "假 Git 未按期启动");
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(unix)]
fn not_running(pid: &str) {
    let limit = Instant::now() + Duration::from_secs(2);
    loop {
        let output = std::process::Command::new("ps")
            .args(["-p", pid.trim(), "-o", "stat="])
            .output()
            .unwrap();
        let status = String::from_utf8_lossy(&output.stdout);
        if status.trim().is_empty() || status.trim().starts_with('Z') {
            return;
        }
        assert!(Instant::now() < limit, "Git 或 helper 进程仍在运行");
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
#[test]
fn cancellation_kills_git_and_helper_and_waits_without_worker_queue() {
    let f = Fixture::new();
    let ready = f.temp.path().join("ready");
    let git = fake_git(
        &f,
        "fake-git-cancel",
        &format!(
            "printf '%s' \"$$\" > '{0}/child.pid'\n/bin/sleep 30 &\nprintf '%s' \"$!\" > '{0}/helper.pid'\nprintf started > '{1}'\nwait",
            f.temp.path().display(),
            ready.display()
        ),
    );
    let operation = Operation::new();
    let worker_op = operation.clone();
    let home = f.app.home.clone();
    let handle = thread::spawn(move || {
        with_operation(&worker_op, || {
            with_git_executable(&git, || {
                GitRunner::new(home).run(["synthetic-task"], "正在连接来源")
            })
        })
    });
    wait_file(&ready);
    assert_eq!(operation.phase(), Phase::Network);
    let cancelled = Instant::now();
    assert!(operation.cancel());
    assert!(handle.join().unwrap().is_err());
    assert!(cancelled.elapsed() < Duration::from_secs(2));
    not_running(&fs::read_to_string(f.temp.path().join("child.pid")).unwrap());
    not_running(&fs::read_to_string(f.temp.path().join("helper.pid")).unwrap());
}

#[cfg(unix)]
#[test]
fn timeout_of_an_active_command_kills_and_waits_for_its_helper() {
    let f = Fixture::new();
    let ready = f.temp.path().join("deadline-ready");
    let git = fake_git(
        &f,
        "fake-git-deadline-helper",
        &format!(
            "/bin/sleep 30 &\nprintf '%s' \"$!\" > '{0}/deadline-helper.pid'\nprintf started > '{1}'\nwait",
            f.temp.path().display(),
            ready.display()
        ),
    );
    let operation = Operation::with_timeout(Duration::from_secs(5));
    let worker_op = operation.clone();
    let home = f.app.home.clone();
    let handle = thread::spawn(move || {
        with_operation(&worker_op, || {
            with_git_executable(&git, || {
                GitRunner::new(home).run(["deadline"], "超时辅助进程")
            })
        })
    });
    wait_file(&ready);
    assert!(operation.shorten_network_deadline(Duration::from_millis(60)));
    let started = Instant::now();
    let error = handle.join().unwrap().unwrap_err().to_string();
    assert!(error.contains("超时"));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(!operation.is_cancelled());
    not_running(&fs::read_to_string(f.temp.path().join("deadline-helper.pid")).unwrap());
}

#[cfg(unix)]
#[test]
fn network_deadline_is_shared_across_commands_and_uses_short_test_policy() {
    let f = Fixture::new();
    let git = fake_git(
        &f,
        "fake-git-budget",
        "printf x >> \"$HOME/invocations\"\nprintf ok",
    );
    let operation = Operation::with_timeout(Duration::from_secs(5));
    let started = Instant::now();
    with_operation(&operation, || {
        with_git_executable(&git, || {
            let runner = GitRunner::new(&f.app.home);
            assert_eq!(runner.run(["first"], "第一阶段").unwrap(), b"ok");
            // 首阶段成功是屏障，只缩短已启动预算，第二阶段必须复用同一时限。
            assert!(operation.shorten_network_deadline(Duration::ZERO));
            operation.set_phase("阶段之间的准备", Phase::Preparing);
            let error = runner.run(["second"], "第二阶段").unwrap_err().to_string();
            assert!(error.contains("超时"));
            assert!(runner.run(["third"], "不应重置预算").is_err());
            assert!(operation.begin_commit().is_err());
        })
    });
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(fs::read(f.app.home.join("invocations")).unwrap(), b"x");
    assert!(!operation.is_cancelled());
}

#[cfg(unix)]
#[test]
fn stdout_is_capped_but_both_pipes_are_continuously_drained() {
    let f = Fixture::new();
    let git = fake_git(
        &f,
        "fake-git-output",
        "i=0\nwhile [ \"$i\" -lt 6000 ]; do\n  printf '0123456789012345678901234567890123456789\\n'\n  printf 'synthetic-secret-stderr-output-xxxxxxxx\\n' >&2\n  i=$((i+1))\ndone",
    );
    let operation = Operation::with_timeout(Duration::from_secs(3));
    let output = with_operation(&operation, || {
        with_git_executable(&git, || {
            GitRunner::new(&f.app.home).run(["output"], "读取输出")
        })
    })
    .unwrap();
    assert_eq!(output.len(), 64 * 1024);
    assert!(!String::from_utf8_lossy(&output).contains("synthetic-secret"));
}

#[cfg(unix)]
#[test]
fn failed_git_errors_never_include_arguments_credentials_or_raw_stderr() {
    let f = Fixture::new();
    let git = fake_git(
        &f,
        "fake-git-error",
        "printf 'https://user:synthetic-secret@example.invalid token=synthetic-secret' >&2\nexit 7",
    );
    let error = with_git_executable(&git, || {
        GitRunner::new(&f.app.home).run(
            ["https://user:synthetic-secret@example.invalid"],
            "测试错误",
        )
    })
    .unwrap_err();
    let error = format!("{error:#}");
    assert!(!error.contains("synthetic-secret"));
    assert!(!error.contains("example.invalid"));
    assert!(!error.contains("user:"));
}

#[cfg(unix)]
#[test]
fn successful_git_exiting_with_a_helper_cannot_leave_pipe_readers_hanging() {
    let f = Fixture::new();
    let git = fake_git(
        &f,
        "fake-git-orphan",
        &format!(
            "/bin/sleep 30 &\nprintf '%s' \"$!\" > '{}/helper.pid'\nprintf done\nexit 0",
            f.temp.path().display()
        ),
    );
    let started = Instant::now();
    let output = with_git_executable(&git, || {
        GitRunner::new(&f.app.home).run(["helper"], "辅助进程")
    })
    .unwrap();
    assert_eq!(output, b"done");
    assert!(started.elapsed() < Duration::from_secs(2));
    not_running(&fs::read_to_string(f.temp.path().join("helper.pid")).unwrap());
}

#[test]
fn failed_network_preparation_keeps_previous_cache_intact() {
    use std::ffi::OsString;
    let f = Fixture::new();
    let repo = f.temp.path().join("local-repository");
    mkdir(&repo).unwrap();
    let runner = GitRunner::new(&f.app.home);
    let git = |args: &[&str]| {
        let mut command: Vec<OsString> = vec!["-C".into(), repo.as_os_str().into()];
        command.extend(args.iter().map(OsString::from));
        runner.run(command, "本地夹具").unwrap();
    };
    git(&["init", "--template=", "-b", "main"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["config", "user.email", "fixture@example.invalid"]);
    common::write_skill(&repo.join("skills/tdd"), "tdd", "Version one");
    git(&["add", "."]);
    git(&["commit", "-m", "fixture"]);
    let cache = f.app.checkout(repo.to_str().unwrap(), "main").unwrap();
    let previous = hash_path(&cache);
    git(&["branch", "-m", "other"]);
    assert!(f.app.checkout(repo.to_str().unwrap(), "main").is_err());
    assert_eq!(hash_path(&cache), previous);
    let entries: Vec<_> = fs::read_dir(cache.parent().unwrap())
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn invalid_source_or_reference_is_rejected_before_git_or_cache_mutation() {
    let f = Fixture::new();
    let repo = f.temp.path().join("repository");
    mkdir(&repo).unwrap();
    for reference in [
        "--upload-pack=evil",
        "branch..other",
        "HEAD@{0}",
        "branch:target",
        "bad\nref",
    ] {
        assert!(f.app.checkout(repo.to_str().unwrap(), reference).is_err());
    }
    assert!(
        f.app
            .checkout("https://user:synthetic-secret@example.invalid/repo", "main")
            .is_err()
    );
    assert!(!f.app.root.join("sources").exists());
}
