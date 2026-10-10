use super::*;

#[test]
fn shared_actual_config_with_multiple_root_keys_preserves_both_and_detects_drift() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    f.app
        .update_host_paths(
            "opencode",
            &host.config_path,
            &host.skill_path,
            &host.agent_path,
        )
        .unwrap();
    f.app.add_mcp(stdio("Multi root")).unwrap();
    for host in ["claude", "opencode"] {
        f.app.toggle_mcp("multi-root", host, true).unwrap();
    }
    let hash = hash_path(Path::new(&host.config_path));
    for deployment in f.app.state.mcps[0].deployments.values_mut() {
        deployment.last_hash = hash.clone();
    }
    f.app.save().unwrap();
    let original = fs::read(&host.config_path).unwrap();
    f.app
        .update_mcp(
            "multi-root",
            McpPatch {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let root = config(&f, "claude");
    assert!(root["mcpServers"].get("Renamed").is_some());
    assert!(root["mcp"].get("Renamed").is_some());
    assert_eq!(
        fs::read(PathBufExt::suffix(Path::new(&host.config_path), ".chu.bak")).unwrap(),
        original
    );
    f.app.state.mcps[0]
        .deployments
        .get_mut("opencode")
        .unwrap()
        .last_hash = "stale".into();
    f.app.save().unwrap();
    no_mcp_write(&mut f, "multi-root");
}

#[test]
fn an_enabled_shared_path_sibling_with_stale_hash_refuses_the_whole_edit() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Edited")).unwrap();
    f.app.add_mcp(stdio("Sibling")).unwrap();
    for id in ["edited", "sibling"] {
        f.app.toggle_mcp(id, "claude", true).unwrap();
    }
    f.app.state.mcps[1]
        .deployments
        .get_mut("claude")
        .unwrap()
        .last_hash = "stale".into();
    f.app.save().unwrap();
    no_mcp_write(&mut f, "edited");
}

#[test]
fn shared_config_root_rename_conflict_preserves_all_root_nodes() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    f.app
        .update_host_paths(
            "opencode",
            &host.config_path,
            &host.skill_path,
            &host.agent_path,
        )
        .unwrap();
    f.app.add_mcp(stdio("Original")).unwrap();
    for host in ["claude", "opencode"] {
        f.app.toggle_mcp("original", host, true).unwrap();
    }
    let mut root = config(&f, "claude");
    root["mcp"]["Conflict"] = json!({"command":"user"});
    set_config(&f, "claude", &root);
    let hash = hash_path(Path::new(&host.config_path));
    for deployment in f.app.state.mcps[0].deployments.values_mut() {
        deployment.last_hash = hash.clone();
    }
    f.app.save().unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "original",
                McpPatch {
                    name: Some("Conflict".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
}

#[test]
fn shared_config_is_written_once_and_every_shared_managed_hash_is_refreshed() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    f.app
        .update_host_paths(
            "jcode",
            &host.config_path,
            &host.skill_path,
            &host.agent_path,
        )
        .unwrap();
    f.app.add_mcp(stdio("Shared")).unwrap();
    f.app.add_mcp(stdio("Other")).unwrap();
    for host in ["claude", "jcode"] {
        for id in ["shared", "other"] {
            if id == "shared" && host == "jcode" {
                let deployment = f.app.state.mcps[0].deployments["claude"].clone();
                f.app.state.mcps[0]
                    .deployments
                    .insert("jcode".into(), deployment);
                f.app.save().unwrap();
            } else if id == "other" && host == "jcode" {
                let deployment = f.app.state.mcps[1].deployments["claude"].clone();
                f.app.state.mcps[1]
                    .deployments
                    .insert("jcode".into(), deployment);
                f.app.save().unwrap();
            } else {
                f.app.toggle_mcp(id, host, true).unwrap();
            }
        }
    }
    let path = Path::new(&host.config_path);
    let original = fs::read(path).unwrap();
    f.app
        .update_mcp(
            "shared",
            McpPatch {
                name: Some("Renamed shared".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        fs::read(PathBufExt::suffix(path, ".chu.bak")).unwrap(),
        original
    );
    for item in &f.app.state.mcps {
        for deployment in item.deployments.values() {
            assert_eq!(deployment.last_hash, hash_path(path));
        }
    }
    f.app
        .delete_mcp("shared", ResourceDeleteInput::default())
        .unwrap();
    for deployment in f.app.state.mcps[0].deployments.values() {
        assert_eq!(deployment.last_hash, hash_path(path));
    }
}

#[test]
fn edits_use_recorded_targets_not_new_host_paths() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Recorded")).unwrap();
    f.app.toggle_mcp("recorded", "claude", true).unwrap();
    let original = f.app.state.mcps[0].deployments["claude"].target.clone();
    let new = f.temp.path().join("new/settings.toml");
    f.app
        .update_host_paths(
            "claude",
            new.to_str().unwrap(),
            f.temp.path().join("new/skills").to_str().unwrap(),
            f.temp.path().join("new/agents").to_str().unwrap(),
        )
        .unwrap();
    f.app
        .update_mcp(
            "recorded",
            McpPatch {
                name: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!exists(&new));
    assert_eq!(f.app.state.mcps[0].deployments["claude"].target, original);
    assert!(
        read_config(Path::new(&original), "json").unwrap()["mcpServers"]
            .get("Changed")
            .is_some()
    );
}

#[test]
fn remote_type_with_jcode_enabled_is_rejected_before_any_write() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Local")).unwrap();
    for host in HOST_IDS {
        f.app.toggle_mcp("local", host, true).unwrap();
    }
    let before = files(&f);
    let error = f
        .app
        .update_mcp(
            "local",
            McpPatch {
                r#type: Some("http".into()),
                endpoint: Some("https://fixture.invalid".into()),
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("先停用"));
    assert_eq!(files(&f), before);
}

#[test]
fn mcp_target_drift_missing_enabled_state_and_semantic_drift_are_zero_write() {
    for mode in 0..4 {
        let mut f = Fixture::new();
        f.app.add_mcp(stdio("Guarded")).unwrap();
        for host in HOST_IDS {
            f.app.toggle_mcp("guarded", host, true).unwrap();
        }
        let target = f.app.state.mcps[0].deployments["codex"].target.clone();
        if mode == 0 {
            atomic_write(Path::new(&target), b"external = true\n").unwrap();
        }
        if mode == 1 {
            remove(Path::new(&target)).unwrap();
        }
        if mode >= 2 {
            let mut root = config(&f, "codex");
            root["mcp_servers"]["Guarded"][if mode == 2 { "enabled" } else { "command" }] =
                if mode == 2 {
                    json!(false)
                } else {
                    json!("external")
                };
            set_config(&f, "codex", &root);
            f.app.state.mcps[0]
                .deployments
                .get_mut("codex")
                .unwrap()
                .last_hash = hash_path(Path::new(&target));
            f.app.save().unwrap();
        }
        no_mcp_write(&mut f, "guarded");
    }
}

#[test]
fn central_state_drift_and_stale_versions_refuse_edits_and_deletes() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Versioned")).unwrap();
    let version = f.app.read_mcp_edit("versioned").unwrap().version;
    f.app
        .update_mcp(
            "versioned",
            McpPatch {
                description: Some("New".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "versioned",
                McpPatch {
                    version: Some(version.clone()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .delete_mcp(
                "versioned",
                ResourceDeleteInput {
                    version: Some(version)
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
    let mut disk = f.app.state.clone();
    disk.mcps[0].description = "External".into();
    write_json(&f.app.root.join("state.json"), &disk).unwrap();
    no_mcp_write(&mut f, "versioned");
}

#[test]
fn mcp_rename_conflicts_with_central_and_unmanaged_host_names() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Original")).unwrap();
    f.app.add_mcp(stdio("Central conflict")).unwrap();
    f.app.toggle_mcp("original", "claude", true).unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "original",
                McpPatch {
                    name: Some("Central conflict".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
    let mut root = config(&f, "claude");
    root["mcpServers"]["Host conflict"] = json!({"command":"user"});
    set_config(&f, "claude", &root);
    let target = f.app.state.mcps[0].deployments["claude"].target.clone();
    f.app.state.mcps[0]
        .deployments
        .get_mut("claude")
        .unwrap()
        .last_hash = hash_path(Path::new(&target));
    f.app.save().unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "original",
                McpPatch {
                    name: Some("Host conflict".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
}
