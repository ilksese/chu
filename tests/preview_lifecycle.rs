mod common;
use chu::{
    App,
    git_runner::GitRunner,
    skill_preview::{with_before_install_commit, with_install_failure},
};
use common::write_skill;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::Duration,
};

struct Sandbox {
    directory: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    repository: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("chu");
        let home = directory.path().join("home");
        let repository = directory.path().join("repository");
        fs::create_dir(&home).unwrap();
        fs::create_dir(&repository).unwrap();
        let runner = GitRunner::new(&home);
        for args in [
            vec!["init", "--template=", "-b", "main"],
            vec!["config", "user.name", "Fixture"],
            vec!["config", "user.email", "fixture@example.invalid"],
        ] {
            git(&runner, &repository, &args);
        }
        write_skill(&repository.join("skills/tdd"), "tdd", "Version one");
        git(&runner, &repository, &["add", "."]);
        git(&runner, &repository, &["commit", "-m", "fixture"]);
        Self {
            directory,
            root,
            home,
            repository,
        }
    }

    fn open(&self) -> App {
        App::open(&self.root, &self.home).unwrap()
    }
}

fn git(runner: &GitRunner, repository: &Path, args: &[&str]) {
    let mut command = vec![OsString::from("-C"), repository.into()];
    command.extend(args.iter().map(OsString::from));
    runner.run(command, "隔离生命周期测试 Git").unwrap();
}

fn preparations(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root.join("sources"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".chu-git-prepare-")
        })
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    paths
}

fn unrelated_caches(root: &Path) -> Vec<PathBuf> {
    ["ordinary-cache", ".chu-git-prepare-not-owned"]
        .into_iter()
        .map(|name| {
            let path = root.join("sources").join(name);
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("keep"), b"unrelated").unwrap();
            path
        })
        .collect()
}

fn assert_caches_unchanged(paths: &[PathBuf]) {
    for path in paths {
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"unrelated");
    }
}

#[test]
fn last_app_drop_cleans_only_its_owned_previews_not_unrelated_sources() {
    let sandbox = Sandbox::new();
    let app = sandbox.open();
    let caches = unrelated_caches(&sandbox.root);
    for _ in 0..2 {
        app.preview_skills_bound(sandbox.repository.to_str().unwrap())
            .unwrap();
    }
    let owned: Vec<_> = preparations(&sandbox.root)
        .into_iter()
        .filter(|path| !caches.contains(path))
        .collect();
    assert_eq!(owned.len(), 2);
    drop(app);
    assert!(owned.iter().all(|path| !path.exists()));
    assert_caches_unchanged(&caches);
    assert!(!sandbox.root.join("state.json").exists());
    assert!(!sandbox.root.join("chu-lock.json").exists());
}

#[test]
fn same_domain_shares_preview_after_creator_drop_and_failed_install_restores_it() {
    let sandbox = Sandbox::new();
    // 首次打开时 root 尚未创建；后来打开仍必须命中同一规范域。
    let creator = sandbox.open();
    let preview = creator
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    let mut owner = sandbox.open();
    drop(creator);
    assert!(snapshot.exists());
    assert!(
        owner
            .install_skills_from_preview(&preview.token, vec!["missing".into()])
            .is_err()
    );
    assert!(snapshot.exists());
    owner
        .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
        .unwrap();
    assert!(!snapshot.exists());
    assert_eq!(
        owner.read_lock().unwrap().skills["tdd"].resolved_commit,
        preview.revision
    );
    owner.drop_skill_preview(&preview.token).unwrap();
    assert!(
        owner
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
}

#[test]
fn same_domain_can_drop_one_token_without_cleaning_the_other() {
    let sandbox = Sandbox::new();
    let first = sandbox.open();
    let one = first
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let one_path = preparations(&sandbox.root).pop().unwrap();
    let second = sandbox.open();
    let two = second
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let two_path = preparations(&sandbox.root)
        .into_iter()
        .find(|path| path != &one_path)
        .unwrap();
    second.drop_skill_preview(&one.token).unwrap();
    first.drop_skill_preview(&one.token).unwrap();
    assert!(!one_path.exists());
    assert!(two_path.exists());
    drop(first);
    assert!(two_path.exists());
    second.drop_skill_preview(&two.token).unwrap();
    assert!(!two_path.exists());
}

#[test]
fn weak_indices_cannot_keep_a_store_alive_or_reopen_an_expired_token() {
    let sandbox = Sandbox::new();
    let app = sandbox.open();
    let preview = app
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    drop(app);
    assert!(!snapshot.exists());
    let mut reopened = sandbox.open();
    reopened.drop_skill_preview(&preview.token).unwrap();
    assert_eq!(
        reopened
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .unwrap_err()
            .to_string(),
        "预览已失效，请重新预览来源"
    );
    reopened
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let fresh = preparations(&sandbox.root).pop().unwrap();
    drop(reopened);
    assert!(!fresh.exists());
}

#[test]
fn foreign_domains_cannot_use_or_drop_tokens_and_keep_their_sole_owner_preview() {
    let sandbox = Sandbox::new();
    let owner = sandbox.open();
    let preview = owner
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    for (root, home) in [
        (
            sandbox.root.clone(),
            sandbox.directory.path().join("other-home"),
        ),
        (
            sandbox.directory.path().join("other-root"),
            sandbox.home.clone(),
        ),
    ] {
        let mut foreign = App::open(root, home).unwrap();
        assert_eq!(
            foreign
                .drop_skill_preview(&preview.token)
                .unwrap_err()
                .to_string(),
            "预览不属于当前应用"
        );
        assert_eq!(
            foreign
                .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
                .unwrap_err()
                .to_string(),
            "预览不属于当前应用"
        );
        foreign.drop_skill_preview("unknown-token").unwrap();
        assert!(snapshot.exists());
    }
    let other_home = sandbox.directory.path().join("independent-home");
    let independent = App::open(&sandbox.root, other_home).unwrap();
    independent
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let independent_path = preparations(&sandbox.root)
        .into_iter()
        .find(|path| path != &snapshot)
        .unwrap();
    drop(owner);
    assert!(!snapshot.exists());
    assert!(independent_path.exists());
    independent.drop_skill_preview(&preview.token).unwrap();
    assert!(independent_path.exists());
    drop(independent);
    assert!(!independent_path.exists());
}

#[cfg(unix)]
#[test]
fn canonical_root_and_home_aliases_share_the_same_preview_domain() {
    let sandbox = Sandbox::new();
    let creator = sandbox.open();
    let preview = creator
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    let alias = sandbox.directory.path().join("alias");
    std::os::unix::fs::symlink(sandbox.directory.path(), &alias).unwrap();
    let peer = App::open(alias.join("chu/../chu"), alias.join("home/.")).unwrap();
    drop(creator);
    assert!(snapshot.exists());
    peer.drop_skill_preview(&preview.token).unwrap();
    assert!(!snapshot.exists());
}

#[test]
fn concurrent_app_owners_hold_snapshots_until_the_last_owner_returns() {
    let sandbox = Sandbox::new();
    let creator = sandbox.open();
    creator
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    let opened = Arc::new(Barrier::new(5));
    let release = Arc::new(Barrier::new(5));
    let owners: Vec<_> = (0..4)
        .map(|_| {
            let root = sandbox.root.clone();
            let home = sandbox.home.clone();
            let opened = opened.clone();
            let release = release.clone();
            thread::spawn(move || {
                let app = App::open(root, home).unwrap();
                opened.wait();
                release.wait();
                drop(app);
            })
        })
        .collect();
    opened.wait();
    drop(creator);
    assert!(snapshot.exists());
    release.wait();
    for owner in owners {
        owner.join().unwrap();
    }
    assert!(!snapshot.exists());
}

#[test]
fn concurrent_drop_during_failed_install_does_not_resurrect_the_preview() {
    let sandbox = Sandbox::new();
    let mut installer = sandbox.open();
    let preview = installer
        .preview_skills_bound(sandbox.repository.to_str().unwrap())
        .unwrap();
    let snapshot = preparations(&sandbox.root).pop().unwrap();
    let mut closer = sandbox.open();
    let (ready, received) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let token = preview.token.clone();
    let installation = thread::spawn(move || {
        let result = with_before_install_commit(
            move || {
                ready.send(()).unwrap();
                resumed.recv_timeout(Duration::from_secs(10)).unwrap();
            },
            || {
                with_install_failure(|| {
                    installer.install_skills_from_preview(&token, vec!["skills/tdd".into()])
                })
            },
        );
        assert!(result.is_err());
        installer
    });
    received.recv_timeout(Duration::from_secs(10)).unwrap();
    closer.drop_skill_preview(&preview.token).unwrap();
    // 安装中持有独立快照，关闭不能中途删除它；也不能再取同一能力。
    assert!(snapshot.exists());
    assert!(
        closer
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
    resume.send(()).unwrap();
    let mut installer = installation.join().unwrap();
    assert!(!snapshot.exists());
    assert!(installer.state.skills.is_empty());
    assert!(!sandbox.root.join("skills/tdd").exists());
    assert!(
        installer
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
    closer.drop_skill_preview(&preview.token).unwrap();
}

const CHILD_DIRECTORY: &str = "CHU_PREVIEW_LIFECYCLE_CHILD_DIRECTORY";

#[test]
fn normal_cli_exit_child() {
    let Some(directory) = std::env::var_os(CHILD_DIRECTORY).map(PathBuf::from) else {
        return;
    };
    // 子进程没有 TempDir/Fixture，只有 App 负责正常返回时清理；隔离目录由父进程持有。
    let root = directory.join("chu");
    let app = App::open(&root, directory.join("home")).unwrap();
    for _ in 0..2 {
        app.preview_skills_bound(directory.join("repository").to_str().unwrap())
            .unwrap();
    }
    let owned: Vec<_> = preparations(&root)
        .into_iter()
        .filter(|path| path.file_name().unwrap() != ".chu-git-prepare-not-owned")
        .collect();
    assert_eq!(owned.len(), 2);
    fs::write(
        directory.join("prepared-paths.json"),
        serde_json::to_vec(&owned).unwrap(),
    )
    .unwrap();
}

#[test]
fn normal_cli_process_exit_leaves_no_owned_preparations_or_unrelated_cache_damage() {
    let sandbox = Sandbox::new();
    let caches = unrelated_caches(&sandbox.root);
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "normal_cli_exit_child", "--nocapture"])
        .env(CHILD_DIRECTORY, sandbox.directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "子进程失败：{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let owned: Vec<PathBuf> = serde_json::from_slice(
        &fs::read(sandbox.directory.path().join("prepared-paths.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(owned.len(), 2);
    assert!(owned.iter().all(|path| !path.exists()));
    assert_caches_unchanged(&caches);
    assert_eq!(preparations(&sandbox.root), vec![caches[1].clone()]);
}
