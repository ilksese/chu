mod common;
use chu::{App, fs::*};
use common::*;
use std::{fs, path::Path};
#[test]
fn lifecycle_preserves_modified_copies_until_explicit_reset() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    let p = f.project();
    f.app.toggle_project_skill(&p, &id, "codex", true).unwrap();
    let target = f.app.state.projects[0].deployments[&id]["codex"]
        .target
        .clone();
    assert!(target.contains(".agents"));
    assert_eq!(
        f.app.snapshot().unwrap().projects[0].deployments[&id]["codex"].status,
        "enabled"
    );
    atomic_write(&Path::new(&target).join("mine.md"), b"mine").unwrap();
    assert_eq!(
        f.app.snapshot().unwrap().projects[0].deployments[&id]["codex"].status,
        "modified"
    );
    assert!(f.app.toggle_project_skill(&p, &id, "codex", false).is_err());
    f.app.reset_project_skill(&p, &id, "codex").unwrap();
    assert!(!exists(Path::new(&target).join("mine.md")));
    f.app.toggle_project_skill(&p, &id, "codex", false).unwrap();
    assert!(!exists(target));
}
#[test]
fn missing_copies_can_be_reset_or_detached() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    let p = f.project();
    f.app.toggle_project_skill(&p, &id, "claude", true).unwrap();
    let target = f.app.state.projects[0].deployments[&id]["claude"]
        .target
        .clone();
    remove(Path::new(&target)).unwrap();
    assert_eq!(
        f.app.snapshot().unwrap().projects[0].deployments[&id]["claude"].status,
        "missing"
    );
    f.app.reset_project_skill(&p, &id, "claude").unwrap();
    assert!(Path::new(&target).join("SKILL.md").is_file());
    remove(Path::new(&target)).unwrap();
    f.app
        .toggle_project_skill(&p, &id, "claude", false)
        .unwrap();
}
#[test]
fn project_usage_blocks_central_skill_delete() {
    let mut f = Fixture::new();
    let skill = f.skill("tdd");
    let p = f.project();
    f.app
        .toggle_project_skill(&p, &skill, "opencode", true)
        .unwrap();
    assert!(
        f.app
            .delete_skill(&skill)
            .unwrap_err()
            .to_string()
            .contains("项目")
    );
    f.app.delete_project(&p, true).unwrap();
    f.app.delete_skill(&skill).unwrap();
}
#[test]
fn project_removal_cleanup_retains_local_changes() {
    let mut f = Fixture::new();
    let skill = f.skill("tdd");
    let p = f.project();
    for h in ["claude", "codex"] {
        f.app.toggle_project_skill(&p, &skill, h, true).unwrap();
    }
    let clean = f.app.state.projects[0].deployments[&skill]["claude"]
        .target
        .clone();
    let modified = f.app.state.projects[0].deployments[&skill]["codex"]
        .target
        .clone();
    atomic_write(&Path::new(&modified).join("mine.md"), b"keep").unwrap();
    let retained = f.app.delete_project(&p, true).unwrap();
    assert_eq!(retained, vec![modified.clone()]);
    assert!(!exists(clean));
    assert_eq!(
        fs::read(Path::new(&modified).join("mine.md")).unwrap(),
        b"keep"
    );
    assert!(f.app.state.projects.is_empty());
}
#[test]
fn project_removal_without_cleanup_leaves_files() {
    let mut f = Fixture::new();
    let s = f.skill("tdd");
    let p = f.project();
    f.app.toggle_project_skill(&p, &s, "jcode", true).unwrap();
    let target = f.app.state.projects[0].deployments[&s]["jcode"]
        .target
        .clone();
    assert!(f.app.delete_project(&p, false).unwrap().is_empty());
    assert!(exists(target));
}
#[test]
fn conflicts_are_reported_and_never_overwritten() {
    let mut f = Fixture::new();
    let s = f.skill("tdd");
    let _ = f.project();
    let project = f.app.state.projects[0].clone();
    write_skill(
        &Path::new(&project.path).join(".claude/skills/tdd"),
        "tdd",
        "external",
    );
    let snapshot = f.app.snapshot().unwrap();
    assert_eq!(
        snapshot.projects[0].deployments[&s]["claude"].status,
        "conflict"
    );
    assert!(
        f.app
            .toggle_project_skill(&project.id, &s, "claude", true)
            .is_err()
    );
}
#[test]
fn rename_relocate_and_duplicate_directories() {
    let mut f = Fixture::new();
    let s = f.skill("tdd");
    let p = f.project();
    let old = f.app.state.projects[0].path.clone();
    assert!(f.app.add_project(&old).is_err());
    f.app.rename_project(&p, "Renamed").unwrap();
    assert!(f.app.rename_project(&p, "").is_err());
    f.app.toggle_project_skill(&p, &s, "claude", true).unwrap();
    let next = f.temp.path().join("next");
    mkdir(&next).unwrap();
    write_skill(&next.join(".claude/skills/tdd"), "tdd", "external");
    f.app.relocate_project(&p, next.to_str().unwrap()).unwrap();
    assert_eq!(
        f.app.state.projects[0].deployments[&s]["claude"].mode,
        "conflict"
    );
    assert!(f.app.reset_project_skill(&p, &s, "claude").is_err());
    assert!(
        Path::new(&old)
            .join(".claude/skills/tdd/SKILL.md")
            .is_file()
    );
    let reopened = App::open(&f.app.root, &f.app.home).unwrap();
    assert_eq!(reopened.state.projects[0].name, "Renamed");
}
#[test]
fn unavailable_project_does_not_recreate_user_directory() {
    let mut f = Fixture::new();
    let s = f.skill("tdd");
    let p = f.project();
    f.app.toggle_project_skill(&p, &s, "codex", true).unwrap();
    let path = f.app.state.projects[0].path.clone();
    remove(Path::new(&path)).unwrap();
    assert!(!f.app.snapshot().unwrap().projects[0].available);
    assert!(f.app.toggle_project_skill(&p, &s, "claude", true).is_err());
    assert!(f.app.reset_project_skill(&p, &s, "codex").is_err());
    assert!(!exists(path));
}
