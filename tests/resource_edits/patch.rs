use super::*;

#[test]
fn authorization_header_patch_is_case_insensitive_and_secret_precedence_is_preserved() {
    let mut f = Fixture::new();
    import_mcp(
        &mut f,
        "Headers",
        json!({"url":"https://fixture.invalid","headers":{"Authorization":"Bearer original-header"},"token":"fallback-secret"}),
        &["claude", "opencode", "codex"],
    );
    f.app
        .update_mcp(
            "headers",
            McpPatch {
                headers: [("authorization".into(), Patch::Keep)].into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(f.app.state.mcps[0].headers.len(), 1);
    assert_eq!(
        f.app.state.mcps[0].headers["Authorization"],
        "Bearer original-header"
    );
    f.app
        .update_mcp(
            "headers",
            McpPatch {
                headers: [(
                    "AUTHORIZATION".into(),
                    Patch::Set("Bearer replacement-header".into()),
                )]
                .into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(f.app.state.mcps[0].headers.len(), 1);
    assert_eq!(
        f.app.state.mcps[0].headers["Authorization"],
        "Bearer replacement-header"
    );
    assert_eq!(
        config(&f, "codex")["mcp_servers"]["Headers"]["http_headers"]["Authorization"],
        "Bearer replacement-header"
    );
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "headers",
                McpPatch {
                    headers: [
                        ("Authorization".into(), Patch::Remove),
                        ("authorization".into(), Patch::Set("duplicate".into()))
                    ]
                    .into(),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
    f.app
        .update_mcp(
            "headers",
            McpPatch {
                headers: [("authorization".into(), Patch::Remove)].into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(f.app.state.mcps[0].headers.is_empty());
    assert_eq!(
        config(&f, "codex")["mcp_servers"]["Headers"]["http_headers"]["Authorization"],
        "Bearer fallback-secret"
    );
    f.app
        .update_mcp(
            "headers",
            McpPatch {
                secret: Patch::Remove,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        config(&f, "codex")["mcp_servers"]["Headers"]
            .get("http_headers")
            .is_none()
    );
}

#[test]
fn new_http_urls_and_inline_command_updates_are_rejected_without_discarding_old_args() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Local")).unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "local",
                McpPatch {
                    r#type: Some("http".into()),
                    endpoint: Some("http://new.invalid".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .update_mcp(
                "local",
                McpPatch {
                    command: Some("npx --token synthetic-inline".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
    assert_eq!(f.app.state.mcps[0].args, vec!["--fixture", "extra"]);
    import_mcp(
        &mut f,
        "Legacy HTTP",
        json!({"url":"http://old.invalid/mcp"}),
        &["codex"],
    );
    f.app
        .update_mcp(
            "legacy-http",
            McpPatch {
                endpoint: Some("http://old.invalid/mcp".into()),
                description: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_mcp(
                "legacy-http",
                McpPatch {
                    endpoint: Some("http://different.invalid".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
    f.app
        .update_mcp(
            "legacy-http",
            McpPatch {
                endpoint: Some("https://secure.invalid/mcp".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        config(&f, "codex")["mcp_servers"]["Legacy HTTP"]["url"],
        "https://secure.invalid/mcp"
    );
}

#[test]
fn patch_json_defaults_and_actions_are_compatible() {
    for value in [
        json!({}),
        json!({"args":null,"env":null,"headers":null,"secret":null}),
    ] {
        let patch: McpPatch = serde_json::from_value(value).unwrap();
        assert!(matches!(patch.args, Patch::Keep));
        assert!(matches!(patch.secret, Patch::Keep));
        assert!(patch.env.is_empty() && patch.headers.is_empty());
    }
    let patch: McpPatch = serde_json::from_value(json!({"args":{"action":"set","value":["private"]},"env":{"TOKEN":{"action":"remove"}},"secret":{"action":"keep"}})).unwrap();
    assert!(matches!(patch.args, Patch::Set(_)));
    assert!(matches!(patch.env["TOKEN"], Patch::Remove));
    assert!(!format!("{:?}", patch.args).contains("private"));
    assert_eq!(
        serde_json::to_value(ResourceDeleteInput::default()).unwrap(),
        json!({"version":null})
    );
    assert_eq!(
        serde_json::to_value(McpTestResult {
            status: McpTestStatus::CommandMissing,
            http_status: None
        })
        .unwrap(),
        json!({"status":"commandMissing","httpStatus":null})
    );
}

#[test]
fn safe_edit_and_snapshot_hide_url_credentials_and_embedded_command_secrets() {
    let mut f = Fixture::new();
    f.app.add_mcp(McpInput { name: "Remote".into(), r#type: "http".into(), endpoint: "https://user:synthetic-password@fixture.invalid/mcp?token=synthetic-query#synthetic-fragment".into(), ..Default::default() }).unwrap();
    let remote = f.app.read_mcp_edit("remote").unwrap();
    assert!(remote.endpoint.is_empty());
    let mut item = f.app.state.mcps[0].clone();
    item.command = "env TOKEN=synthetic-inline run".into();
    assert!(safe_mcp_view(&item).command.is_empty());
    item.command = "synthetic-private".into();
    item.secret = "synthetic-private".into();
    assert!(safe_mcp_view(&item).command.is_empty());
    assert_eq!(
        safe_command("/usr/local/bin/mcp-server"),
        "/usr/local/bin/mcp-server"
    );
    assert_eq!(
        safe_endpoint("https://fixture.invalid/mcp"),
        "https://fixture.invalid/mcp"
    );
    assert!(
        !serde_json::to_string(&f.app.snapshot().unwrap())
            .unwrap()
            .contains("synthetic-password")
    );
    let mut root = config(&f, "claude");
    root["mcpServers"]["Discovered"] = json!({"command":"env TOKEN=synthetic-discovered run"});
    set_config(&f, "claude", &root);
    assert!(
        !serde_json::to_string(&f.app.snapshot().unwrap())
            .unwrap()
            .contains("synthetic-discovered")
    );
}

#[test]
fn typed_mcp_test_results_do_not_expose_command_credentials() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Available")).unwrap();
    assert_eq!(
        f.app.test_mcp_result("available").unwrap().status,
        McpTestStatus::CommandAvailable
    );
    f.app
        .add_mcp(McpInput {
            name: "Missing".into(),
            r#type: "stdio".into(),
            command: "missing-synthetic-secret-executable".into(),
            ..Default::default()
        })
        .unwrap();
    let result = f.app.test_mcp_result("missing").unwrap();
    assert_eq!(result.status, McpTestStatus::CommandMissing);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("synthetic-secret")
    );
    assert!(
        !f.app
            .test_mcp("missing")
            .unwrap()
            .contains("synthetic-secret")
    );
}
