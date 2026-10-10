use super::*;

#[test]
fn legacy_mcp_rename_keeps_id_and_updates_then_deletes_all_four_hosts() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Original")).unwrap();
    for host in HOST_IDS {
        f.app.toggle_mcp("original", host, true).unwrap();
    }
    let view = f.app.read_mcp_edit("original").unwrap();
    assert_eq!(view.enabled_hosts.len(), 4);
    f.app
        .update_mcp(
            "original",
            McpPatch {
                name: Some("Renamed".into()),
                version: Some(view.version),
                ..Default::default()
            },
        )
        .unwrap();
    let item = &f.app.state.mcps[0];
    assert_eq!(item.id, "original");
    assert!(!item.native_formats);
    for host in HOST_IDS {
        let root = config(&f, host);
        let servers = &root[root_key(host, &root)];
        assert!(servers.get("Original").is_none());
        assert_eq!(servers["Renamed"]["command"], "echo");
        assert_eq!(servers["Renamed"]["args"], json!(["--fixture", "extra"]));
        assert_eq!(
            item.deployments[host].last_hash,
            hash_path(Path::new(&item.deployments[host].target))
        );
    }
    f.app
        .delete_mcp(
            "original",
            ResourceDeleteInput {
                version: Some(f.app.read_mcp_edit("original").unwrap().version),
            },
        )
        .unwrap();
    assert!(f.app.state.mcps.is_empty());
    for host in HOST_IDS {
        let root = config(&f, host);
        assert!(root[root_key(host, &root)].get("Renamed").is_none());
        assert!(if host == "codex" {
            root["model"] == "keep"
        } else {
            root["preserve"]["nested"] == "keep"
        });
    }
}

#[test]
fn native_formats_extra_options_and_jcode_alias_survive_rename() {
    let mut f = Fixture::new();
    let mut root = config(&f, "jcode");
    root["mcpServers"] = json!({});
    set_config(&f, "jcode", &root);
    import_mcp(
        &mut f,
        "Native",
        json!({"command":"echo","args":["synthetic-argument"],"env":{"TOKEN":"synthetic-env"},"timeout":12000,"custom":{"retain":true}}),
        &HOST_IDS,
    );
    f.app
        .update_mcp(
            "native",
            McpPatch {
                name: Some("Renamed native".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(f.app.state.mcps[0].native_formats);
    for host in HOST_IDS {
        let root = config(&f, host);
        let item = &root[root_key(host, &root)]["Renamed native"];
        assert_eq!(item["timeout"], 12000);
        assert_eq!(item["custom"]["retain"], true);
        if host == "opencode" {
            assert_eq!(item["command"], json!(["echo", "synthetic-argument"]));
            assert_eq!(item["environment"]["TOKEN"], "synthetic-env");
            assert_eq!(item["enabled"], true);
        } else {
            assert_eq!(item["command"], "echo");
            assert_eq!(item["env"]["TOKEN"], "synthetic-env");
        }
    }
    assert!(config(&f, "jcode").get("servers").is_none());
}

#[test]
fn native_remote_headers_are_verified_against_host_rendering_not_central_shape() {
    let mut f = Fixture::new();
    import_mcp(
        &mut f,
        "Remote",
        json!({"type":"sse","url":"http://fixture.invalid/mcp","token":"synthetic-token","headers":{"X-Custom":"synthetic-header"},"timeout":1200}),
        &["claude", "opencode", "codex"],
    );
    f.app
        .update_mcp(
            "remote",
            McpPatch {
                description: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let codex = config(&f, "codex");
    assert_eq!(
        codex["mcp_servers"]["Remote"]["http_headers"]["Authorization"],
        "Bearer synthetic-token"
    );
    assert_eq!(
        codex["mcp_servers"]["Remote"]["http_headers"]["X-Custom"],
        "synthetic-header"
    );
    assert_eq!(codex["mcp_servers"]["Remote"]["timeout"], 1200);
    f.app
        .delete_mcp("remote", ResourceDeleteInput::default())
        .unwrap();
}

#[test]
fn credentials_and_args_keep_set_remove_without_plaintext_edit_views() {
    let mut f = Fixture::new();
    let mut initial = stdio("Private");
    initial.args = vec!["synthetic-args".into()];
    initial.env = [
        ("TOKEN".into(), "synthetic-env".into()),
        ("KEEP".into(), "synthetic-keep".into()),
    ]
    .into();
    initial.headers = [("X-Token".into(), "synthetic-header".into())].into();
    initial.secret = "synthetic-secret".into();
    f.app.add_mcp(initial).unwrap();
    for host in HOST_IDS {
        f.app.toggle_mcp("private", host, true).unwrap();
    }
    let view = f.app.read_mcp_edit("private").unwrap();
    let serialized = serde_json::to_string(&view).unwrap();
    for private in [
        "synthetic-args",
        "synthetic-env",
        "synthetic-header",
        "synthetic-secret",
    ] {
        assert!(!serialized.contains(private));
    }
    assert_eq!(view.args_count, 2);
    assert_eq!(view.env_keys, vec!["KEEP", "TOKEN"]);
    assert!(view.has_secret);
    let before = f.app.state.mcps[0].clone();
    f.app.update_mcp("private", McpPatch::default()).unwrap();
    assert_eq!(f.app.state.mcps[0], before);
    f.app
        .update_mcp(
            "private",
            McpPatch {
                args: Patch::Set(vec!["replacement-args".into()]),
                env: [("TOKEN".into(), Patch::Set("replacement-env".into()))].into(),
                headers: [("X-Token".into(), Patch::Set("replacement-header".into()))].into(),
                secret: Patch::Set("replacement-secret".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let item = &f.app.state.mcps[0];
    assert_eq!(item.env["KEEP"], "synthetic-keep");
    assert_eq!(item.args, vec!["replacement-args"]);
    for host in HOST_IDS {
        let root = config(&f, host);
        let value = &root[root_key(host, &root)]["Private"];
        assert_eq!(value["env"]["TOKEN"], "replacement-env");
        assert_eq!(value["headers"]["X-Token"], "replacement-header");
        assert_eq!(value["token"], "replacement-secret");
    }
    f.app
        .update_mcp(
            "private",
            McpPatch {
                args: Patch::Remove,
                env: [("TOKEN".into(), Patch::Remove)].into(),
                headers: [("X-Token".into(), Patch::Remove)].into(),
                secret: Patch::Remove,
                ..Default::default()
            },
        )
        .unwrap();
    let item = &f.app.state.mcps[0];
    assert!(item.args.is_empty() && item.headers.is_empty() && item.secret.is_empty());
    assert!(!item.env.contains_key("TOKEN"));
    assert_eq!(item.env["KEEP"], "synthetic-keep");
}
