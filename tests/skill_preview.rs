mod common;
use chu::{
    fs::{atomic_write, hash_path, identifier, mkdir, remove},
    git_runner::{GitRunner, with_git_executable},
    models::SkillLock,
    operation::{Operation, Phase, with_operation},
};
use common::{Fixture, write_skill};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

fn git(f: &Fixture, root: &Path, args: &[&str]) -> String {
    let mut command: Vec<OsString> = vec!["-C".into(), root.into()];
    command.extend(args.iter().map(OsString::from));
    String::from_utf8(
        GitRunner::new(&f.app.home)
            .run(command, "隔离测试 Git")
            .unwrap(),
    )
    .unwrap()
    .trim()
    .into()
}

fn repository(f: &Fixture, names: &[&str]) -> PathBuf {
    let repo = f.temp.path().join("repository");
    mkdir(&repo).unwrap();
    git(f, &repo, &["init", "--template=", "-b", "main"]);
    git(f, &repo, &["config", "user.name", "Fixture"]);
    git(
        f,
        &repo,
        &["config", "user.email", "fixture@example.invalid"],
    );
    for name in names {
        write_skill(
            &repo.join("skills").join(identifier(name, "skill")),
            name,
            "Version one",
        );
    }
    git(f, &repo, &["add", "."]);
    git(f, &repo, &["commit", "-m", "fixture"]);
    repo
}

fn snapshots(f: &Fixture) -> Vec<PathBuf> {
    fs::read_dir(f.app.root.join("sources"))
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
fn bound_install_keeps_preview_commit_when_branch_advances_and_does_not_fetch() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    assert_eq!(preview.revision, git(&f, &repo, &["rev-parse", "HEAD"]));
    assert_eq!(preview.candidates[0].path, "skills/tdd");
    let serialized = serde_json::to_value(&preview).unwrap();
    assert!(serialized.get("cachePath").is_none());
    assert!(serialized.get("snapshotPath").is_none());
    write_skill(&repo.join("skills/tdd"), "tdd", "Version two");
    git(&f, &repo, &["add", "."]);
    git(&f, &repo, &["commit", "-m", "advanced"]);
    assert_ne!(preview.revision, git(&f, &repo, &["rev-parse", "HEAD"]));
    with_git_executable(&f.temp.path().join("missing-git"), || {
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .unwrap();
    });
    assert!(
        fs::read_to_string(Path::new(&f.app.state.skills[0].source_dir).join("SKILL.md"))
            .unwrap()
            .contains("Version one")
    );
    let lock = f.app.read_lock().unwrap();
    assert_eq!(lock.skills["tdd"].resolved_commit, preview.revision);
    assert_eq!(lock.skills["tdd"].r#ref, "main");
    assert!(snapshots(&f).is_empty());
    assert_eq!(f.app.check_skill_updates().unwrap()[0].status, "update");
}

#[test]
fn bound_install_does_not_need_the_original_repository_after_preview() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    remove(&repo).unwrap();
    with_git_executable(&f.temp.path().join("missing-git"), || {
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .unwrap();
    });
    assert!(f.app.root.join("skills/tdd/SKILL.md").is_file());
    assert_eq!(
        f.app.read_lock().unwrap().skills["tdd"].resolved_commit,
        preview.revision
    );
}

#[test]
fn snapshot_tampering_is_rejected_without_state_lock_or_skill_writes() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let snapshot = snapshots(&f).pop().unwrap();
    atomic_write(
        &snapshot.join("repository/skills/tdd/SKILL.md"),
        b"tampered",
    )
    .unwrap();
    let state = fs::read(f.app.root.join("state.json")).ok();
    let lock = fs::read(f.app.root.join("chu-lock.json")).ok();
    assert!(
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
    assert!(f.app.state.skills.is_empty());
    assert_eq!(fs::read(f.app.root.join("state.json")).ok(), state);
    assert_eq!(fs::read(f.app.root.join("chu-lock.json")).ok(), lock);
    assert!(!f.app.root.join("skills/tdd").exists());
    f.app.drop_skill_preview(&preview.token).unwrap();
    assert!(!snapshot.exists());
}

#[test]
fn tampering_outside_selected_candidate_is_also_rejected() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let snapshot = snapshots(&f).pop().unwrap();
    atomic_write(&snapshot.join("repository/extra-file"), b"new content").unwrap();
    assert!(
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
    assert!(f.app.state.skills.is_empty());
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[test]
fn only_selected_preview_paths_and_owner_can_use_capability() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    for path in [
        "../repository/skills/tdd",
        "/skills/tdd",
        "skills/./tdd",
        "skills\\tdd",
        ".git",
        "skills/missing",
    ] {
        assert!(
            f.app
                .install_skills_from_preview(&preview.token, vec![path.into()])
                .is_err()
        );
    }
    assert!(
        f.app
            .install_skills_from_preview(
                &preview.token,
                vec!["skills/tdd".into(), "skills/tdd".into()]
            )
            .is_err()
    );
    assert!(
        f.app
            .install_skills_from_preview("unknown-token", vec!["skills/tdd".into()])
            .is_err()
    );
    let mut other = Fixture::new();
    assert!(other.app.drop_skill_preview(&preview.token).is_err());
    assert!(
        other
            .app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
    f.app
        .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
        .unwrap();
    assert_eq!(f.app.state.skills.len(), 1);
    assert!(
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
            .is_err()
    );
}

#[test]
fn whole_batch_preflight_protects_installed_and_untracked_directories() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["fresh", "existing"]);
    f.skill("existing");
    let previous = hash_path(&f.app.root.join("skills/existing"));
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    assert!(
        preview
            .candidates
            .iter()
            .find(|c| c.name == "existing")
            .unwrap()
            .installed
    );
    assert!(
        f.app
            .install_skills_from_preview(
                &preview.token,
                vec!["skills/fresh".into(), "skills/existing".into()]
            )
            .is_err()
    );
    assert!(!f.app.root.join("skills/fresh").exists());
    assert_eq!(f.app.state.skills.len(), 1);
    assert_eq!(hash_path(&f.app.root.join("skills/existing")), previous);
    let untracked = f.app.root.join("skills/fresh");
    mkdir(&untracked).unwrap();
    atomic_write(&untracked.join("mine"), b"keep").unwrap();
    assert!(
        f.app
            .install_skills_from_preview(&preview.token, vec!["skills/fresh".into()])
            .is_err()
    );
    assert_eq!(fs::read(untracked.join("mine")).unwrap(), b"keep");
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[test]
fn two_display_names_with_one_destination_id_reject_the_entire_batch() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["Alpha Beta"]);
    write_skill(&repo.join("skills/second"), "alpha-beta", "Another skill");
    git(&f, &repo, &["add", "."]);
    git(&f, &repo, &["commit", "-m", "colliding identity"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    assert_eq!(preview.candidates.len(), 2);
    let paths = preview.candidates.iter().map(|c| c.path.clone()).collect();
    assert!(
        f.app
            .install_skills_from_preview(&preview.token, paths)
            .is_err()
    );
    assert!(f.app.state.skills.is_empty());
    assert!(!f.app.root.join("skills/alpha-beta").exists());
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[test]
fn dropping_one_preview_only_cleans_its_own_snapshot() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let first = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let second = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    assert_eq!(snapshots(&f).len(), 2);
    let unrelated = f.app.root.join("sources/unrelated-cache");
    atomic_write(&unrelated.join("mine"), b"keep").unwrap();
    f.app.drop_skill_preview(&first.token).unwrap();
    f.app.drop_skill_preview(&first.token).unwrap();
    assert_eq!(snapshots(&f).len(), 1);
    assert_eq!(fs::read(unrelated.join("mine")).unwrap(), b"keep");
    f.app
        .install_skills_from_preview(&second.token, vec!["skills/tdd".into()])
        .unwrap();
    assert!(snapshots(&f).is_empty());
    assert_eq!(fs::read(unrelated.join("mine")).unwrap(), b"keep");
}

#[test]
fn cancellation_before_install_has_no_resource_side_effects_and_preserves_preview() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let operation = Operation::new();
    assert!(operation.cancel());
    with_operation(&operation, || {
        assert!(
            f.app
                .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
                .is_err()
        );
    });
    assert!(operation.is_cancelled());
    assert!(f.app.state.skills.is_empty());
    assert_eq!(snapshots(&f).len(), 1);
    f.app
        .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
        .unwrap();
}

#[test]
fn successful_preview_atomically_finishes_and_prevents_late_cancel_token_leaks() {
    let f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let operation = Operation::new();
    let preview = with_operation(&operation, || {
        f.app.preview_skills_bound(repo.to_str().unwrap())
    })
    .unwrap();
    assert_eq!(operation.phase(), Phase::Finished);
    assert!(!operation.cancel());
    f.app.drop_skill_preview(&preview.token).unwrap();
    assert!(snapshots(&f).is_empty());
}

#[test]
fn bound_preview_rejects_credentials_and_query_without_echoing_secret() {
    let f = Fixture::new();
    for source in [
        "https://user:synthetic-secret@github.com/org/repo",
        "https://github.com/org/repo?token=synthetic-secret",
    ] {
        let error = f.app.preview_skills_bound(source).unwrap_err().to_string();
        assert!(!error.contains("synthetic-secret"));
        assert!(!error.contains("user:"));
    }
    let migrated: SkillLock =
        serde_json::from_str(r#"{"source":"https://github.com/org/repo","ref":"branch"}"#).unwrap();
    assert!(migrated.resolved_commit.is_empty());
    assert_eq!(migrated.r#ref, "branch");
}

#[test]
fn cancellation_winning_at_final_preflight_does_not_rewrite_state_or_lock() {
    let mut f = Fixture::new();
    f.skill("keep");
    f.app.write_lock(&f.app.read_lock().unwrap()).unwrap();
    let state_path = f.app.root.join("state.json");
    let lock_path = f.app.root.join("chu-lock.json");
    let state_bytes = fs::read(&state_path).unwrap();
    let lock_bytes = fs::read(&lock_path).unwrap();
    let state_time = fs::metadata(&state_path).unwrap().modified().unwrap();
    let lock_time = fs::metadata(&lock_path).unwrap().modified().unwrap();
    let repo = repository(&f, &["tdd"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let operation = Operation::new();
    let cancel = operation.clone();
    with_operation(&operation, || {
        chu::skill_preview::with_before_install_commit(
            move || {
                assert!(cancel.cancel());
            },
            || {
                assert!(
                    f.app
                        .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
                        .is_err()
                );
            },
        )
    });
    assert!(operation.is_cancelled());
    assert_eq!(fs::read(&state_path).unwrap(), state_bytes);
    assert_eq!(fs::read(&lock_path).unwrap(), lock_bytes);
    assert_eq!(
        fs::metadata(&state_path).unwrap().modified().unwrap(),
        state_time
    );
    assert_eq!(
        fs::metadata(&lock_path).unwrap().modified().unwrap(),
        lock_time
    );
    assert!(!f.app.root.join("skills/tdd").exists());
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[test]
fn external_directory_appearing_after_preflight_is_not_removed_by_batch_rollback() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["first", "second"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let target = f.app.root.join("skills/second");
    chu::skill_preview::with_before_install_commit(
        move || {
            atomic_write(&target.join("mine"), b"external").unwrap();
        },
        || {
            assert!(
                f.app
                    .install_skills_from_preview(
                        &preview.token,
                        vec!["skills/first".into(), "skills/second".into()]
                    )
                    .is_err()
            );
        },
    );
    assert!(!f.app.root.join("skills/first").exists());
    assert_eq!(
        fs::read(f.app.root.join("skills/second/mine")).unwrap(),
        b"external"
    );
    assert!(f.app.state.skills.is_empty());
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[test]
fn committed_batch_failure_rolls_back_state_lock_and_every_new_skill_directory() {
    let mut f = Fixture::new();
    f.skill("keep-existing");
    let mut lock = f.app.read_lock().unwrap();
    lock.skills
        .insert("keep-existing".into(), SkillLock::default());
    f.app.write_lock(&lock).unwrap();
    let original_state = f.app.state.clone();
    let state_bytes = fs::read(f.app.root.join("state.json")).unwrap();
    let lock_bytes = fs::read(f.app.root.join("chu-lock.json")).unwrap();
    let existing = hash_path(&f.app.root.join("skills/keep-existing"));
    let repo = repository(&f, &["first", "second"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    let paths = vec!["skills/first".into(), "skills/second".into()];
    let operation = Operation::new();
    with_operation(&operation, || {
        chu::skill_preview::with_install_failure(|| {
            assert!(
                f.app
                    .install_skills_from_preview(&preview.token, paths.clone())
                    .is_err()
            );
        })
    });
    assert_eq!(operation.phase(), Phase::Commit);
    assert!(!operation.cancel());
    assert_eq!(f.app.state, original_state);
    assert_eq!(
        fs::read(f.app.root.join("state.json")).unwrap(),
        state_bytes
    );
    assert_eq!(
        fs::read(f.app.root.join("chu-lock.json")).unwrap(),
        lock_bytes
    );
    assert_eq!(
        hash_path(&f.app.root.join("skills/keep-existing")),
        existing
    );
    assert!(!f.app.root.join("skills/first").exists());
    assert!(!f.app.root.join("skills/second").exists());
    assert_eq!(snapshots(&f).len(), 1);
    f.app
        .install_skills_from_preview(&preview.token, paths)
        .unwrap();
    assert_eq!(f.app.state.skills.len(), 3);
    assert!(snapshots(&f).is_empty());
}

#[test]
fn committed_failure_removes_new_parent_without_touching_unrelated_root_data() {
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    let marker = f.app.root.join("unrelated");
    atomic_write(&marker, b"keep").unwrap();
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    assert!(!f.app.root.join("skills").exists());
    chu::skill_preview::with_install_failure(|| {
        assert!(
            f.app
                .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
                .is_err()
        );
    });
    assert!(!f.app.root.join("skills").exists());
    assert_eq!(fs::read(marker).unwrap(), b"keep");
    assert!(f.app.state.skills.is_empty());
    assert!(!f.app.root.join("chu-lock.json").exists());
    f.app.drop_skill_preview(&preview.token).unwrap();
}

#[cfg(unix)]
#[test]
fn outside_symlinks_are_rejected_and_safe_internal_links_are_preserved() {
    use std::os::unix::fs::symlink;
    let mut f = Fixture::new();
    let repo = repository(&f, &["tdd"]);
    atomic_write(&repo.join("skills/tdd/data"), b"inside").unwrap();
    symlink("data", repo.join("skills/tdd/link")).unwrap();
    git(&f, &repo, &["add", "."]);
    git(&f, &repo, &["commit", "-m", "internal link"]);
    let preview = f.app.preview_skills_bound(repo.to_str().unwrap()).unwrap();
    f.app
        .install_skills_from_preview(&preview.token, vec!["skills/tdd".into()])
        .unwrap();
    assert_eq!(
        fs::read(f.app.root.join("skills/tdd/link")).unwrap(),
        b"inside"
    );
    let outside = f.temp.path().join("outside");
    atomic_write(&outside, b"private data").unwrap();
    remove(&repo.join("skills/tdd/link")).unwrap();
    symlink(&outside, repo.join("skills/tdd/link")).unwrap();
    git(&f, &repo, &["add", "."]);
    git(&f, &repo, &["commit", "-m", "unsafe link"]);
    assert!(f.app.preview_skills_bound(repo.to_str().unwrap()).is_err());
    assert!(snapshots(&f).is_empty());
    assert_eq!(fs::read(&outside).unwrap(), b"private data");
}
