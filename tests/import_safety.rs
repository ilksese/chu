mod common;
use chu::{fs::*, mcp::*};
use common::*;
use serde_json::json;
use std::path::Path;
#[test]
fn import_only_resources_can_later_be_applied_to_their_original_hosts() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    let source = Path::new(&host.skill_path).join("native");
    write_skill(&source, "native", "Fixture");
    let selected = f
        .app
        .list_import_candidates("skills")
        .unwrap()
        .candidates
        .into_iter()
        .map(|r| r.id)
        .collect();
    f.app
        .import_host_resources("skills", selected, vec![])
        .unwrap();
    f.app.toggle_skill("native", "claude", true).unwrap();
    assert!(f.app.state.skills[0].deployments["claude"].enabled);
    let mut root = read_config(Path::new(&host.config_path), "json").unwrap();
    root["mcpServers"] = json!({"native":{"command":"echo","args":["fixture"]}});
    write_config(Path::new(&host.config_path), "json", &root).unwrap();
    let selected = f
        .app
        .list_import_candidates("mcps")
        .unwrap()
        .candidates
        .into_iter()
        .map(|r| r.id)
        .collect();
    f.app
        .import_host_resources("mcps", selected, vec![])
        .unwrap();
    f.app.toggle_mcp("native", "claude", true).unwrap();
    assert!(f.app.state.mcps[0].deployments["claude"].enabled);
}
#[test]
fn invalid_target_configuration_prevents_any_imported_mcp_from_being_committed() {
    let mut f = Fixture::new();
    let source = f.app.host("claude").unwrap();
    let mut root = read_config(Path::new(&source.config_path), "json").unwrap();
    root["mcpServers"] = json!({"native":{"command":"echo"}});
    write_config(Path::new(&source.config_path), "json", &root).unwrap();
    let selected = f
        .app
        .list_import_candidates("mcps")
        .unwrap()
        .candidates
        .into_iter()
        .map(|r| r.id)
        .collect();
    let codex = f.app.host("codex").unwrap();
    atomic_write(Path::new(&codex.config_path), b"broken = [").unwrap();
    assert!(
        f.app
            .import_host_resources("mcps", selected, vec!["opencode".into(), "codex".into()])
            .is_err()
    );
    assert!(f.app.state.mcps.is_empty());
    let open = f.app.host("opencode").unwrap();
    assert_eq!(
        read_config(Path::new(&open.config_path), "json").unwrap()["preserve"]["nested"],
        "keep"
    );
}
#[test]
fn same_slug_and_empty_mcp_names_do_not_overwrite_existing_resources() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    let mut root = read_config(Path::new(&host.config_path), "json").unwrap();
    root["mcpServers"] = json!({"same name":{"command":"echo"},"same-name":{"command":"echo"},"":{"command":"echo"}});
    write_config(Path::new(&host.config_path), "json", &root).unwrap();
    let scan = f.app.list_import_candidates("mcps").unwrap();
    assert!(
        scan.candidates
            .iter()
            .any(|r| r.name.is_empty() && !r.available)
    );
    let selected = scan
        .candidates
        .into_iter()
        .filter(|r| r.available)
        .map(|r| r.id)
        .collect();
    assert!(
        f.app
            .import_host_resources("mcps", selected, vec![])
            .is_err()
    );
    assert!(f.app.state.mcps.is_empty());
}
