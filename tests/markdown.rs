mod common;
use chu::{fs::*, models::*};
use common::*;
use std::fs;
#[test]
fn prompt_crud_rename_and_external_discovery() {
    let mut f = Fixture::new();
    f.app
        .create_prompt("review", "# Rules\nReview carefully.")
        .unwrap();
    assert_eq!(
        f.app.read_prompt("review").unwrap(),
        "# Rules\nReview carefully."
    );
    f.app
        .update_prompt("review", "review-v2", "new body")
        .unwrap();
    assert!(f.app.read_prompt("review").is_err());
    assert_eq!(f.app.read_prompt("review-v2").unwrap(), "new body");
    atomic_write(&f.app.root.join("prompts/external.md"), b"external prompt").unwrap();
    assert_eq!(f.app.snapshot().unwrap().prompts.len(), 2);
    f.app.delete_prompt("review-v2").unwrap();
    assert_eq!(f.app.snapshot().unwrap().prompts.len(), 1);
}
#[test]
fn prompt_targets_match_four_native_hosts() {
    let f = Fixture::new();
    for (h, file) in [
        ("opencode", "AGENTS.md"),
        ("claude", "CLAUDE.md"),
        ("codex", "AGENTS.md"),
        ("jcode", "prompt-overlay.md"),
    ] {
        assert_eq!(f.app.prompt_target(h).unwrap().file_name().unwrap(), file);
    }
}
#[test]
fn switching_prompt_preserves_original_backup_and_restores_it() {
    let mut f = Fixture::new();
    f.app.create_prompt("one", "first").unwrap();
    f.app.create_prompt("two", "second").unwrap();
    let target = f.app.prompt_target("claude").unwrap();
    atomic_write(&target, b"original").unwrap();
    f.app.toggle_prompt("one", "claude", true).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "first");
    f.app.toggle_prompt("two", "claude", true).unwrap();
    let snap = f.app.snapshot().unwrap();
    assert!(
        !snap
            .prompts
            .iter()
            .find(|p| p.id == "one")
            .unwrap()
            .enabled_on["claude"]
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "second");
    f.app.toggle_prompt("two", "claude", false).unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "original");
}
#[test]
fn prompt_source_changes_and_rename_follow_deployments() {
    let mut f = Fixture::new();
    f.app.create_prompt("rules", "old").unwrap();
    f.app.toggle_prompt("rules", "codex", true).unwrap();
    let target = f.app.prompt_target("codex").unwrap();
    atomic_write(&f.app.root.join("prompts/rules.md"), b"external edit").unwrap();
    f.app.snapshot().unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "external edit");
    f.app
        .update_prompt("rules", "new-rules", "renamed")
        .unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "renamed");
    assert!(!exists(f.app.root.join("prompts/rules.md")));
    f.app.delete_prompt("new-rules").unwrap();
    assert!(!exists(target));
}
#[test]
fn prompt_copy_drift_releases_management_without_overwrite() {
    let mut f = Fixture::new();
    f.app.create_prompt("rules", "central").unwrap();
    let target = f.app.prompt_target("claude").unwrap();
    atomic_write(&target, b"copy").unwrap();
    let mut lock = f.app.read_lock().unwrap();
    lock.prompts.get_mut("rules").unwrap().deployments.insert(
        "claude".into(),
        PromptDeployment {
            mode: "copy".into(),
            target: target.to_string_lossy().into(),
            last_hash: md5_file(&target),
            backup: false,
        },
    );
    f.app.write_lock(&lock).unwrap();
    atomic_write(&target, b"user edited").unwrap();
    f.app.snapshot().unwrap();
    assert!(
        f.app.read_lock().unwrap().prompts["rules"]
            .deployments
            .is_empty()
    );
    f.app.toggle_prompt("rules", "claude", false).unwrap();
    assert_eq!(fs::read_to_string(target).unwrap(), "user edited");
}
#[test]
fn clean_prompt_copies_sync_on_source_change() {
    let mut f = Fixture::new();
    f.app.create_prompt("rules", "central").unwrap();
    let target = f.app.prompt_target("jcode").unwrap();
    atomic_write(&target, b"central").unwrap();
    let mut lock = f.app.read_lock().unwrap();
    lock.prompts.get_mut("rules").unwrap().deployments.insert(
        "jcode".into(),
        PromptDeployment {
            mode: "copy".into(),
            target: target.to_string_lossy().into(),
            last_hash: md5_file(&target),
            backup: false,
        },
    );
    f.app.write_lock(&lock).unwrap();
    atomic_write(&f.app.root.join("prompts/rules.md"), b"updated").unwrap();
    f.app.snapshot().unwrap();
    assert_eq!(fs::read_to_string(target).unwrap(), "updated");
}
#[test]
fn chinese_prompt_names_are_unique_and_editable() {
    let mut f = Fixture::new();
    f.app.create_prompt("代码审查", "内容").unwrap();
    let id = f.app.snapshot().unwrap().prompts[0].id.clone();
    f.app.update_prompt(&id, "测试规范", "新内容").unwrap();
    assert_eq!(f.app.snapshot().unwrap().prompts[0].name, "测试规范");
}
#[test]
fn reference_crud_discovery_and_path_traversal() {
    let mut f = Fixture::new();
    f.app.create_reference("guide", "# Guide").unwrap();
    assert_eq!(f.app.read_reference("guide.md").unwrap(), "# Guide");
    f.app.update_reference("guide.md", "new", "body").unwrap();
    assert!(f.app.read_reference("guide.md").is_err());
    atomic_write(&f.app.root.join("references/external.md"), b"External").unwrap();
    assert_eq!(f.app.snapshot().unwrap().references.len(), 2);
    assert!(f.app.read_reference("../state.json").is_err());
    assert!(
        f.app
            .update_reference("new.md", "../escape", "bad")
            .is_err()
    );
    f.app.delete_reference("new.md").unwrap();
    assert_eq!(f.app.snapshot().unwrap().references.len(), 1);
}
#[cfg(unix)]
#[test]
fn references_do_not_follow_external_links() {
    let mut f = Fixture::new();
    let outside = f.temp.path().join("outside.md");
    atomic_write(&outside, b"private").unwrap();
    mkdir(&f.app.root.join("references")).unwrap();
    symlink(&outside, &f.app.root.join("references/link.md")).unwrap();
    assert!(f.app.read_reference("link.md").is_err());
    assert!(f.app.delete_reference("link.md").is_err());
    assert!(f.app.snapshot().unwrap().references.is_empty());
}
#[test]
fn markdown_preview_is_unicode_safe() {
    let f = Fixture::new();
    let path = f.temp.path().join("preview.md");
    atomic_write(&path, "汉".repeat(90).as_bytes()).unwrap();
    assert_eq!(preview(&path).chars().count(), 80);
}

#[test]
fn prompt_target_directories_are_never_recursively_deleted() {
    let mut f = Fixture::new();
    f.app.create_prompt("rules", "central").unwrap();
    let target = f.app.prompt_target("claude").unwrap();
    atomic_write(&target.join("user-file"), b"keep").unwrap();
    atomic_write(
        &chu::mcp::PathBufExt::suffix(&target, ".chubak"),
        b"original",
    )
    .unwrap();
    assert!(f.app.toggle_prompt("rules", "claude", true).is_err());
    assert_eq!(fs::read(target.join("user-file")).unwrap(), b"keep");
}

#[test]
fn markdown_names_strip_one_extension_and_same_file_renames_work() {
    let mut f = Fixture::new();
    f.app.create_reference("guide.md.md", "body").unwrap();
    assert_eq!(f.app.snapshot().unwrap().references[0].name, "guide.md");
    let old = f.app.root.join("references/guide.md.md");
    let new = f.app.root.join("references/alias.md");
    fs::hard_link(&old, &new).unwrap();
    f.app
        .update_reference("guide.md.md", "alias", "updated")
        .unwrap();
    assert_eq!(f.app.read_reference("alias.md").unwrap(), "updated");
    assert!(!exists(old));
    f.app.create_prompt("rules.md.md", "body").unwrap();
    assert_eq!(f.app.snapshot().unwrap().prompts[0].name, "rules.md");
}
