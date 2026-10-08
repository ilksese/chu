mod common;
use chu::{fs::*, mcp::*, presenter::ViewState};
use common::*;
use serde_json::{Value, json};
use std::{fs, path::Path};
fn host_skill(f: &Fixture, host: &str, name: &str, description: &str) -> std::path::PathBuf {
    let h = f.app.host(host).unwrap();
    let path = Path::new(&h.skill_path).join(name);
    write_skill(&path, name, description);
    path
}
fn host_mcp(f: &Fixture, host: &str, servers: Value) {
    let h = f.app.host(host).unwrap();
    let mut root = read_config(Path::new(&h.config_path), &h.format).unwrap();
    let key = root_key(host, &root);
    root[key] = servers;
    write_config(Path::new(&h.config_path), &h.format, &root).unwrap();
}
fn ids(f: &Fixture, kind: &str, names: &[&str]) -> Vec<String> {
    f.app
        .list_import_candidates(kind)
        .unwrap()
        .candidates
        .into_iter()
        .filter(|r| names.contains(&r.name.as_str()) && r.available)
        .map(|r| r.id)
        .collect()
}
fn native(f: &Fixture, host: &str) -> Value {
    let h = f.app.host(host).unwrap();
    read_config(Path::new(&h.config_path), &h.format).unwrap()
}
#[test]
fn resource_pages_show_only_chu_managed_rows() {
    let mut f = Fixture::new();
    host_skill(&f, "claude", "host-only", "External");
    host_mcp(&f, "claude", json!({"host-mcp":{"command":"echo"}}));
    f.skill("managed");
    f.app.add_mcp(stdio("managed-mcp")).unwrap();
    let mut state = ViewState {
        snapshot: f.app.snapshot().unwrap(),
        page: 2,
        ..Default::default()
    };
    let rows = chu::search::rows(&state);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "managed");
    state.page = 3;
    let rows = chu::search::rows(&state);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "managed-mcp");
    assert_eq!(
        f.app.list_import_candidates("skills").unwrap().candidates[0].name,
        "host-only"
    );
    assert_eq!(
        f.app.list_import_candidates("mcps").unwrap().candidates[0].name,
        "host-mcp"
    );
}
#[test]
fn selected_skills_are_imported_and_applied_to_selected_hosts_only() {
    let mut f = Fixture::new();
    let source = host_skill(&f, "claude", "alpha", "First");
    host_skill(&f, "claude", "beta", "Second");
    host_skill(&f, "claude", "skip", "Not selected");
    let original = hash_path(&source);
    let selected = ids(&f, "skills", &["alpha", "beta"]);
    f.app
        .import_host_resources("skills", selected, vec!["claude".into(), "codex".into()])
        .unwrap();
    assert_eq!(f.app.state.skills.len(), 2);
    assert_eq!(hash_path(&source), original);
    for skill in &f.app.state.skills {
        assert!(Path::new(&skill.source_dir).join("SKILL.md").is_file());
        assert!(skill.deployments["claude"].enabled);
        assert!(skill.deployments["codex"].enabled);
        assert!(!skill.deployments.contains_key("opencode"));
    }
    let scan = f.app.list_import_candidates("skills").unwrap();
    assert!(
        scan.candidates
            .iter()
            .filter(|r| r.name != "skip")
            .all(|r| !r.available)
    );
}
#[test]
fn importing_without_apply_hosts_keeps_the_source_unmanaged() {
    let mut f = Fixture::new();
    let path = host_skill(&f, "claude", "alpha", "Original");
    let hash = hash_path(&path);
    let selected = ids(&f, "skills", &["alpha"]);
    f.app
        .import_host_resources("skills", selected, vec![])
        .unwrap();
    assert_eq!(hash_path(&path), hash);
    assert!(f.app.state.skills[0].deployments.is_empty());
}
#[cfg(unix)]
#[test]
fn symlink_sources_can_be_imported_and_applied_without_removing_external_files() {
    let mut f = Fixture::new();
    let external = f.temp.path().join("external-skill");
    write_skill(&external, "linked", "External");
    let h = f.app.host("claude").unwrap();
    mkdir(Path::new(&h.skill_path)).unwrap();
    let link = Path::new(&h.skill_path).join("linked");
    symlink(&external, &link).unwrap();
    let selected = ids(&f, "skills", &["linked"]);
    f.app
        .import_host_resources("skills", selected, vec!["claude".into()])
        .unwrap();
    assert!(external.join("SKILL.md").is_file());
    assert_eq!(
        link.canonicalize().unwrap(),
        Path::new(&f.app.state.skills[0].source_dir)
            .canonicalize()
            .unwrap()
    );
    f.app.toggle_skill("linked", "claude", false).unwrap();
    assert!(!exists(link));
    assert!(external.join("SKILL.md").is_file());
}
#[test]
fn skill_conflict_preflight_prevents_partial_import() {
    let mut f = Fixture::new();
    host_skill(&f, "claude", "alpha", "Source");
    host_skill(&f, "claude", "beta", "Source");
    let target = host_skill(&f, "codex", "beta", "Different user version");
    let original = hash_path(&target);
    let selected = ids(&f, "skills", &["alpha", "beta"])
        .into_iter()
        .filter(|id| {
            f.app
                .list_import_candidates("skills")
                .unwrap()
                .candidates
                .iter()
                .any(|r| r.id == *id && r.host_id == "claude")
        })
        .collect();
    assert!(
        f.app
            .import_host_resources("skills", selected, vec!["codex".into()])
            .is_err()
    );
    assert!(f.app.state.skills.is_empty());
    assert!(!exists(f.app.root.join("skills/alpha")));
    assert_eq!(hash_path(&target), original);
}
#[test]
fn duplicate_names_and_stale_preview_ids_are_rejected() {
    let mut f = Fixture::new();
    let source = host_skill(&f, "claude", "same", "A");
    host_skill(&f, "codex", "same", "B");
    assert!(
        f.app
            .import_host_resources("skills", ids(&f, "skills", &["same"]), vec![])
            .unwrap_err()
            .to_string()
            .contains("同名")
    );
    let old = f
        .app
        .list_import_candidates("skills")
        .unwrap()
        .candidates
        .into_iter()
        .find(|r| r.host_id == "claude")
        .unwrap()
        .id;
    atomic_write(&source.join("new.md"), b"Changed after preview").unwrap();
    assert!(
        f.app
            .import_host_resources("skills", vec![old], vec![])
            .unwrap_err()
            .to_string()
            .contains("重新扫描")
    );
    assert!(f.app.state.skills.is_empty());
}
#[test]
fn mcp_import_preserves_credentials_and_uses_native_target_formats() {
    let mut f = Fixture::new();
    host_mcp(
        &f,
        "claude",
        json!({"local":{"command":"npx","args":["-y","fixture"],"env":{"TOKEN":"synthetic-secret"}},"remote":{"type":"http","url":"https://fixture.invalid/mcp","headers":{"Authorization":"Bearer synthetic-header"}}}),
    );
    let source = native(&f, "claude");
    let scan = f.app.list_import_candidates("mcps").unwrap();
    let preview = serde_json::to_string(&scan).unwrap();
    assert!(!preview.contains("synthetic-secret"));
    assert!(!preview.contains("synthetic-header"));
    assert!(scan.candidates.iter().all(|r| r.has_credentials));
    f.app
        .import_host_resources(
            "mcps",
            scan.candidates.into_iter().map(|r| r.id).collect(),
            vec!["opencode".into(), "codex".into()],
        )
        .unwrap();
    assert_eq!(native(&f, "claude"), source);
    let open = native(&f, "opencode");
    assert_eq!(open["mcp"]["local"]["type"], "local");
    assert_eq!(
        open["mcp"]["local"]["command"],
        json!(["npx", "-y", "fixture"])
    );
    assert_eq!(
        open["mcp"]["local"]["environment"]["TOKEN"],
        "synthetic-secret"
    );
    assert_eq!(open["mcp"]["remote"]["type"], "remote");
    let codex = native(&f, "codex");
    assert_eq!(
        codex["mcp_servers"]["local"]["args"],
        json!(["-y", "fixture"])
    );
    assert_eq!(
        codex["mcp_servers"]["remote"]["http_headers"]["Authorization"],
        "Bearer synthetic-header"
    );
    assert_eq!(codex["model"], "keep");
    assert_eq!(open["preserve"]["nested"], "keep");
    assert!(
        f.app
            .state
            .mcps
            .iter()
            .all(|m| m.native_formats && m.deployments.len() == 2)
    );
}
#[test]
fn native_opencode_command_arrays_and_codex_header_aliases_are_importable() {
    let mut f = Fixture::new();
    host_mcp(
        &f,
        "opencode",
        json!({"native-local":{"type":"local","command":["npx","-y","fixture"],"environment":{"FIXTURE":"yes"},"enabled":false}}),
    );
    host_mcp(
        &f,
        "codex",
        json!({"native-remote":{"url":"https://fixture.invalid","http_headers":{"X-Fixture":"yes"}}}),
    );
    f.app
        .import_host_resources(
            "mcps",
            ids(&f, "mcps", &["native-local", "native-remote"]),
            vec!["claude".into()],
        )
        .unwrap();
    let root = native(&f, "claude");
    assert_eq!(root["mcpServers"]["native-local"]["command"], "npx");
    assert_eq!(
        root["mcpServers"]["native-local"]["args"],
        json!(["-y", "fixture"])
    );
    assert_eq!(root["mcpServers"]["native-local"]["env"]["FIXTURE"], "yes");
    assert_eq!(
        root["mcpServers"]["native-remote"]["headers"]["X-Fixture"],
        "yes"
    );
}
#[test]
fn applying_to_the_mcp_source_adopts_existing_options_and_enables_it() {
    let mut f = Fixture::new();
    host_mcp(
        &f,
        "opencode",
        json!({"native":{"type":"local","command":["echo","fixture"],"enabled":false,"timeout":12000}}),
    );
    f.app
        .import_host_resources(
            "mcps",
            ids(&f, "mcps", &["native"]),
            vec!["opencode".into()],
        )
        .unwrap();
    let root = native(&f, "opencode");
    assert_eq!(root["mcp"]["native"]["enabled"], true);
    assert_eq!(root["mcp"]["native"]["timeout"], 12000);
    f.app.toggle_mcp("native", "opencode", false).unwrap();
    f.app.toggle_mcp("native", "opencode", true).unwrap();
    assert_eq!(
        native(&f, "opencode")["mcp"]["native"]["command"],
        json!(["echo", "fixture"])
    );
}
#[test]
fn remote_mcp_cannot_be_applied_to_jcode_and_the_batch_is_unchanged() {
    let mut f = Fixture::new();
    host_mcp(
        &f,
        "claude",
        json!({"remote":{"type":"sse","url":"https://fixture.invalid"}}),
    );
    let original = native(&f, "opencode");
    assert!(
        f.app
            .import_host_resources(
                "mcps",
                ids(&f, "mcps", &["remote"]),
                vec!["opencode".into(), "jcode".into()]
            )
            .is_err()
    );
    assert!(f.app.state.mcps.is_empty());
    assert_eq!(native(&f, "opencode"), original);
}
#[test]
fn mcp_conflicts_and_external_config_drift_are_protected() {
    let mut f = Fixture::new();
    host_mcp(&f, "claude", json!({"same":{"command":"echo"}}));
    host_mcp(&f, "codex", json!({"same":{"command":"user-command"}}));
    let original = native(&f, "codex");
    let selected = f
        .app
        .list_import_candidates("mcps")
        .unwrap()
        .candidates
        .into_iter()
        .filter(|r| r.host_id == "claude")
        .map(|r| r.id)
        .collect();
    assert!(
        f.app
            .import_host_resources("mcps", selected, vec!["codex".into()])
            .is_err()
    );
    assert_eq!(native(&f, "codex"), original);
    assert!(f.app.state.mcps.is_empty());
    f.app.add_mcp(stdio("managed")).unwrap();
    f.app.toggle_mcp("managed", "opencode", true).unwrap();
    let h = f.app.host("opencode").unwrap();
    atomic_write(Path::new(&h.config_path), br#"{"outside":"changed"}"#).unwrap();
    let selected = f
        .app
        .list_import_candidates("mcps")
        .unwrap()
        .candidates
        .into_iter()
        .filter(|r| r.host_id == "claude")
        .map(|r| r.id)
        .collect();
    assert!(
        f.app
            .import_host_resources("mcps", selected, vec!["opencode".into()])
            .is_err()
    );
    assert_eq!(native(&f, "opencode")["outside"], "changed");
}
#[test]
fn mcp_state_commit_failure_restores_configuration_and_previous_backup() {
    let mut f = Fixture::new();
    host_mcp(&f, "claude", json!({"native":{"command":"echo"}}));
    let target = f.app.host("codex").unwrap();
    let path = Path::new(&target.config_path);
    let original = fs::read(path).unwrap();
    let backup = PathBufExt::suffix(path, ".chu.bak");
    atomic_write(&backup, b"previous backup").unwrap();
    mkdir(&f.app.root.join("state.json")).unwrap();
    assert!(
        f.app
            .import_host_resources("mcps", ids(&f, "mcps", &["native"]), vec!["codex".into()])
            .is_err()
    );
    assert!(f.app.state.mcps.is_empty());
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(fs::read(backup).unwrap(), b"previous backup");
}
#[cfg(unix)]
#[test]
fn skill_state_commit_failure_restores_source_links_and_removes_new_deployments() {
    let mut f = Fixture::new();
    let external = f.temp.path().join("external");
    write_skill(&external, "native", "Keep me");
    let h = f.app.host("claude").unwrap();
    mkdir(Path::new(&h.skill_path)).unwrap();
    let link = Path::new(&h.skill_path).join("native");
    symlink(&external, &link).unwrap();
    mkdir(&f.app.root.join("state.json")).unwrap();
    assert!(
        f.app
            .import_host_resources(
                "skills",
                ids(&f, "skills", &["native"]),
                vec!["claude".into(), "codex".into()]
            )
            .is_err()
    );
    assert!(f.app.state.skills.is_empty());
    assert_eq!(fs::read_link(link).unwrap(), external);
    assert!(!exists(f.app.root.join("skills/native")));
    let codex = f.app.host("codex").unwrap();
    assert!(!exists(Path::new(&codex.skill_path).join("native")));
}
#[test]
fn scans_report_invalid_hosts_without_leaking_values_or_blocking_other_hosts() {
    let f = Fixture::new();
    host_mcp(
        &f,
        "claude",
        json!({"valid":{"command":"echo"},"invalid":{"command":123,"env":{"SECRET":"synthetic-private"}}}),
    );
    let codex = f.app.host("codex").unwrap();
    atomic_write(Path::new(&codex.config_path), b"broken = [").unwrap();
    let scan = f.app.list_import_candidates("mcps").unwrap();
    assert_eq!(scan.warnings.len(), 1);
    assert!(
        scan.candidates
            .iter()
            .any(|r| r.name == "valid" && r.available)
    );
    assert!(
        scan.candidates
            .iter()
            .any(|r| r.name == "invalid" && !r.available)
    );
    assert!(
        !serde_json::to_string(&scan)
            .unwrap()
            .contains("synthetic-private")
    );
}
#[test]
fn import_api_rejects_empty_stale_unknown_or_already_managed_selections() {
    let mut f = Fixture::new();
    host_skill(&f, "claude", "native", "Fixture");
    let selected = ids(&f, "skills", &["native"]);
    assert!(
        f.app
            .import_host_resources("skills", vec![], vec![])
            .is_err()
    );
    assert!(
        f.app
            .import_host_resources("skills", selected.clone(), vec!["unknown".into()])
            .is_err()
    );
    assert!(f.app.list_import_candidates("agents").is_err());
    let scan = f
        .app
        .execute("ListImportCandidates", json!(["skills"]))
        .unwrap();
    assert_eq!(scan["candidates"].as_array().unwrap().len(), 1);
    f.app
        .execute("ImportHostResources", json!(["skills", selected, []]))
        .unwrap();
    let scan = f.app.list_import_candidates("skills").unwrap();
    assert!(!scan.candidates[0].available);
    assert!(
        f.app
            .import_host_resources("skills", vec![scan.candidates[0].id.clone()], vec![])
            .is_err()
    );
}
