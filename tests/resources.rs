mod common;
use chu::{fs::*, mcp::*, models::*, sources::Source};
use common::*;
use std::{fs, path::Path};
#[test]
fn skills_deploy_and_remove_for_all_four_hosts() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    for host in chu::hosts::HOST_IDS {
        f.app.toggle_skill(&id, host, true).unwrap();
        let h = f.app.host(host).unwrap();
        assert!(Path::new(&h.skill_path).join("tdd/SKILL.md").is_file());
        f.app.toggle_skill(&id, host, false).unwrap();
        assert!(!exists(Path::new(&h.skill_path).join("tdd")));
    }
}
#[test]
fn skill_conflicts_and_modified_copies_are_protected() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    let h = f.app.host("claude").unwrap();
    let target = Path::new(&h.skill_path).join("tdd");
    write_skill(&target, "tdd", "External");
    assert!(f.app.toggle_skill(&id, "claude", true).is_err());
    let hash = hash_path(&target);
    f.app.state.skills[0].deployments.insert(
        "claude".into(),
        Deployment {
            enabled: true,
            mode: "copy".into(),
            target: target.to_string_lossy().into(),
            last_hash: hash,
        },
    );
    atomic_write(&target.join("local.md"), b"mine").unwrap();
    assert!(f.app.toggle_skill(&id, "claude", false).is_err());
    assert!(f.app.delete_skill(&id).is_err());
    assert_eq!(fs::read(target.join("local.md")).unwrap(), b"mine");
}
#[cfg(unix)]
#[test]
fn replaced_links_are_protected_on_disable_and_delete() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    f.app.toggle_skill(&id, "codex", true).unwrap();
    let target = Path::new(&f.app.host("codex").unwrap().skill_path).join("tdd");
    remove(&target).unwrap();
    write_skill(&target, "external", "User data");
    assert!(f.app.toggle_skill(&id, "codex", false).is_err());
    assert!(f.app.delete_skill(&id).is_err());
    assert!(target.join("SKILL.md").is_file());
}
#[test]
fn discovered_skill_can_be_imported_without_loss() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    let source = Path::new(&host.skill_path).join("external");
    write_skill(&source, "external", "Existing skill");
    let s = f.app.snapshot().unwrap();
    assert!(!s.skills[0].managed);
    f.app.import_skill("claude", "external").unwrap();
    let s = f.app.snapshot().unwrap();
    assert!(s.skills[0].managed);
    assert!(s.skills[0].enabled_on["claude"]);
    assert!(source.join("SKILL.md").is_file());
    assert!(f.app.import_skill("claude", "../escape").is_err());
}
#[test]
fn delete_skill_removes_sources_and_lock_entries() {
    let mut f = Fixture::new();
    let id = f.skill("tdd");
    f.app.toggle_skill(&id, "opencode", true).unwrap();
    let source = f.app.state.skills[0].source_dir.clone();
    let mut lock = f.app.read_lock().unwrap();
    lock.skills.insert("tdd".into(), SkillLock::default());
    f.app.write_lock(&lock).unwrap();
    f.app.delete_skill(&id).unwrap();
    assert!(!exists(&source));
    assert!(f.app.read_lock().unwrap().skills.is_empty());
}
#[test]
fn install_from_preview_paths_rejects_traversal_and_tracks_origin() {
    let mut f = Fixture::new();
    let repo = repo();
    let source = Source {
        kind: "git".into(),
        url: repo.path().to_string_lossy().into(),
        reference: "main".into(),
        subpath: String::new(),
        filter: String::new(),
    };
    assert!(
        f.app
            .install_from_cache(&source, repo.path(), &["../escape".into()])
            .is_err()
    );
    f.app
        .install_from_cache(&source, repo.path(), &["skills/tdd".into()])
        .unwrap();
    let lock = f.app.read_lock().unwrap();
    assert_eq!(lock.skills["tdd"].skill_path, "skills/tdd");
    assert!(f.app.snapshot().unwrap().skills[0].tracked);
}
#[test]
fn git_updates_sync_clean_project_copies_but_preserve_modified_ones() {
    let mut f = Fixture::new();
    let repo = repo();
    let source = Source {
        kind: "git".into(),
        url: repo.path().to_string_lossy().into(),
        reference: "main".into(),
        subpath: String::new(),
        filter: String::new(),
    };
    f.app
        .install_from_cache(&source, repo.path(), &["skills/tdd".into()])
        .unwrap();
    let project = f.project();
    f.app
        .toggle_project_skill(&project, "tdd", "claude", true)
        .unwrap();
    f.app
        .toggle_project_skill(&project, "tdd", "codex", true)
        .unwrap();
    let copy = f.app.state.projects[0].deployments["tdd"]["codex"]
        .target
        .clone();
    atomic_write(&Path::new(&copy).join("mine.md"), b"local").unwrap();
    assert!(f.app.check_skill_updates().unwrap().is_empty());
    write_skill(&repo.path().join("skills/tdd"), "tdd", "Version two");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "updated"]);
    assert_eq!(f.app.check_skill_updates().unwrap()[0].status, "update");
    f.app.update_skill("tdd").unwrap();
    let clean = f.app.state.projects[0].deployments["tdd"]["claude"]
        .target
        .clone();
    assert!(
        fs::read_to_string(Path::new(&clean).join("SKILL.md"))
            .unwrap()
            .contains("Version two")
    );
    assert_eq!(
        fs::read(Path::new(&copy).join("mine.md")).unwrap(),
        b"local"
    );
    assert!(f.app.check_skill_updates().unwrap().is_empty());
}
#[test]
fn source_deleted_update_status_and_legacy_migration() {
    let mut f = Fixture::new();
    let repo = repo();
    let source = Source {
        kind: "git".into(),
        url: repo.path().to_string_lossy().into(),
        reference: "main".into(),
        subpath: String::new(),
        filter: String::new(),
    };
    f.app
        .install_from_cache(&source, repo.path(), &["skills/tdd".into()])
        .unwrap();
    remove(&repo.path().join("skills/tdd")).unwrap();
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "removed"]);
    assert_eq!(f.app.check_skill_updates().unwrap()[0].status, "deleted");
    assert!(f.app.update_skill("tdd").is_err());
    let path = f.app.root.join("state.json");
    let mut state = serde_json::to_value(&f.app.state).unwrap();
    state["skills"][0]["repository"] = serde_json::json!("org/repo#dev");
    atomic_write(&path, &serde_json::to_vec(&state).unwrap()).unwrap();
    remove(&f.app.root.join("chu-lock.json")).unwrap();
    let app = chu::App::open(&f.app.root, &f.app.home).unwrap();
    assert_eq!(app.read_lock().unwrap().skills["tdd"].r#ref, "dev");
}
#[test]
fn mcp_deployment_preserves_native_formats_and_hashes() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("one")).unwrap();
    f.app.add_mcp(stdio("two")).unwrap();
    for host in chu::hosts::HOST_IDS {
        let h = f.app.host(host).unwrap();
        f.app.toggle_mcp("one", host, true).unwrap();
        f.app.toggle_mcp("two", host, true).unwrap();
        let root = read_config(Path::new(&h.config_path), &h.format).unwrap();
        assert_eq!(
            root[root_key(host, &root)]["one"]["args"],
            serde_json::json!(["--fixture", "extra"])
        );
        if host == "codex" {
            assert_eq!(root["model"], "keep");
        } else {
            assert_eq!(root["preserve"]["nested"], "keep");
        }
        f.app.toggle_mcp("one", host, false).unwrap();
        f.app.toggle_mcp("two", host, false).unwrap();
    }
}
#[test]
fn mcp_external_edits_conflicts_and_backup_are_safe() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("fixture")).unwrap();
    f.app.toggle_mcp("fixture", "claude", true).unwrap();
    let h = f.app.host("claude").unwrap();
    let path = Path::new(&h.config_path);
    let original = fs::read(path).unwrap();
    atomic_write(path, b"{\"user\":\"changed\"}").unwrap();
    assert!(f.app.toggle_mcp("fixture", "claude", false).is_err());
    f.app.restore_backup("claude").unwrap();
    assert!(exists(PathBufExt::suffix(path, ".chu.before-restore")));
    assert_eq!(
        read_config(path, "json").unwrap()["preserve"]["nested"],
        "keep"
    );
    assert_ne!(fs::read(path).unwrap(), original);
}
#[test]
fn jcode_alias_is_preserved_and_remote_service_is_rejected() {
    let mut f = Fixture::new();
    let h = f.app.host("jcode").unwrap();
    atomic_write(
        Path::new(&h.config_path),
        br#"{"mcpServers":{"existing":{"command":"keep"}}}"#,
    )
    .unwrap();
    f.app.add_mcp(stdio("fixture")).unwrap();
    f.app.toggle_mcp("fixture", "jcode", true).unwrap();
    let root = read_config(Path::new(&h.config_path), "json").unwrap();
    assert!(root.get("servers").is_none());
    assert_eq!(root["mcpServers"]["existing"]["command"], "keep");
    f.app
        .add_mcp(McpInput {
            name: "remote".into(),
            r#type: "http".into(),
            endpoint: "https://example.invalid/mcp".into(),
            ..McpInput::default()
        })
        .unwrap();
    assert!(f.app.toggle_mcp("remote", "jcode", true).is_err());
}
#[test]
fn mcp_discovery_validation_and_non_launching_test() {
    let mut f = Fixture::new();
    assert!(
        f.app
            .add_mcp(McpInput {
                name: "bad".into(),
                r#type: "http".into(),
                endpoint: "http://example.invalid".into(),
                ..McpInput::default()
            })
            .is_err()
    );
    f.app.add_mcp(stdio("echo-test")).unwrap();
    assert!(!f.app.test_mcp("echo-test").unwrap().is_empty());
    let h = f.app.host("opencode").unwrap();
    atomic_write(
        Path::new(&h.config_path),
        br#"{"mcp":{"external":{"url":"https://example.invalid","type":"sse"}}}"#,
    )
    .unwrap();
    let snapshot = f.app.snapshot().unwrap();
    assert!(
        snapshot
            .mcps
            .iter()
            .any(|m| !m.managed && m.r#type == "sse")
    );
}
#[test]
fn agents_restore_content_and_protect_local_edits() {
    let mut f = Fixture::new();
    f.app
        .add_agent(AgentInput {
            name: "review".into(),
            description: "Code review".into(),
            prompt: "Review carefully.".into(),
            model: "model-a".into(),
        })
        .unwrap();
    for host in chu::hosts::HOST_IDS {
        f.app.toggle_agent("review", host, true).unwrap();
        let h = f.app.host(host).unwrap();
        let path = Path::new(&h.agent_path).join("review.md");
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("model: model-a")
        );
        f.app.toggle_agent("review", host, false).unwrap();
        assert!(!exists(path));
    }
    f.app.toggle_agent("review", "codex", true).unwrap();
    let h = f.app.host("codex").unwrap();
    let path = Path::new(&h.agent_path).join("review.md");
    atomic_write(&path, b"local change").unwrap();
    assert!(f.app.toggle_agent("review", "codex", false).is_err());
    assert_eq!(fs::read(path).unwrap(), b"local change");
}

#[test]
fn remote_mcp_test_uses_headers_and_bearer_credentials() {
    let mut f = Fixture::new();
    let mut server = Server::new(vec![(503, String::new(), vec![])]);
    f.app.state.mcps.push(StoredMcp {
        id: "remote-test".into(),
        name: "remote-test".into(),
        r#type: "sse".into(),
        endpoint: server.url.clone(),
        secret: "synthetic-test-token".into(),
        headers: [("X-Fixture".into(), "enabled".into())].into(),
        ..StoredMcp::default()
    });
    assert!(f.app.test_mcp("remote-test").unwrap().contains("503"));
    server.finish();
    let requests = server.requests.lock().unwrap();
    let request = requests[0].to_lowercase();
    assert!(request.contains("x-fixture: enabled"));
    assert!(request.contains("authorization: bearer synthetic-test-token"));
}

#[test]
fn root_skill_installation_ignores_git_metadata_and_tracks_name_changes() {
    let mut f = Fixture::new();
    let repo = repo();
    remove(&repo.path().join("skills")).unwrap();
    write_skill(repo.path(), "root-skill", "Root version one");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "root skill"]);
    let source = Source {
        kind: "git".into(),
        url: repo.path().to_string_lossy().into(),
        reference: "main".into(),
        subpath: String::new(),
        filter: String::new(),
    };
    f.app
        .install_from_cache(&source, repo.path(), &[".".into()])
        .unwrap();
    assert!(!exists(
        Path::new(&f.app.state.skills[0].source_dir).join(".git")
    ));
    assert!(f.app.check_skill_updates().unwrap().is_empty());
    assert!(f.app.check_skill_updates().unwrap().is_empty());
    write_skill(repo.path(), "renamed-root", "Root version two");
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-m", "renamed"]);
    assert_eq!(f.app.check_skill_updates().unwrap()[0].status, "update");
    f.app.update_skill("root-skill").unwrap();
    assert_eq!(f.app.state.skills[0].name, "renamed-root");
    assert!(f.app.check_skill_updates().unwrap().is_empty());
}
#[test]
fn display_names_may_contain_slashes_without_becoming_paths() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Code Review / v2")).unwrap();
    assert_eq!(f.app.state.mcps[0].id, "code-review-v2");
    f.app
        .add_agent(AgentInput {
            name: "Code Review / v2".into(),
            prompt: "Review".into(),
            ..AgentInput::default()
        })
        .unwrap();
    assert!(f.app.root.join("agents/code-review-v2.md").is_file());
}
