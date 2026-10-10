mod common;
use chu::{
    App, McpInput, State, StoredMcp,
    editor::{self, Dialog},
    mcp::{item_config, read_config, root_key},
    presenter::{ViewState, model},
    ui::{AppWindow, FormField, KeyValueRow},
};
use common::Fixture;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

const ARG: &str = "draft-argument-sentinel";
const SECRET: &str = "draft-secret-sentinel";
const BAD: &str = "wrong-type-secret-sentinel";

fn row(key: &str, action: i32, stored: bool) -> KeyValueRow {
    KeyValueRow {
        id: if stored { key } else { "" }.into(),
        key: key.into(),
        value: "draft-map-value".into(),
        action,
        stored,
    }
}
fn rows(headers: bool, editing: bool) -> Vec<KeyValueRow> {
    let prefix = if headers { "X-" } else { "" };
    [("Keep", 0), ("Set", 1), ("Remove", 2)]
        .into_iter()
        .map(|(key, action)| row(&format!("{prefix}{key}"), action, editing))
        .chain([row(&format!("{prefix}NewKeep"), 0, false)])
        .collect()
}
fn old_map(headers: bool) -> BTreeMap<String, String> {
    let prefix = if headers { "X-" } else { "" };
    ["Keep", "Set", "Remove"]
        .into_iter()
        .map(|key| (format!("{prefix}{key}"), "old-map-value".into()))
        .collect()
}
fn base(name: &str, kind: &str) -> McpInput {
    McpInput {
        name: name.into(),
        r#type: kind.into(),
        command: "echo".into(),
        endpoint: "https://fixture.invalid/mcp".into(),
        args: vec!["old-argument".into()],
        env: old_map(false),
        headers: old_map(true),
        secret: "old-secret".into(),
        ..Default::default()
    }
}
fn form(ui: &AppWindow, f: &mut Fixture, kind: &str, editing: bool) -> Dialog {
    let mut dialog = editor::open(
        ui,
        &ViewState::default(),
        if editing { "UpdateMCP" } else { "AddMCP" },
        "fixture",
        Value::Null,
    );
    if editing {
        f.app.add_mcp(base("fixture", kind)).unwrap();
        dialog.id = f.app.state.mcps[0].id.clone();
        dialog.version = Some(f.app.read_mcp_edit(&dialog.id).unwrap().version);
        let hosts: Vec<_> = f
            .app
            .hosts()
            .into_iter()
            .filter(|host| kind == "stdio" || host.id != "jcode")
            .map(|host| host.id)
            .collect();
        for host in hosts {
            f.app.toggle_mcp(&dialog.id, &host, true).unwrap();
        }
        dialog.version = Some(f.app.read_mcp_edit(&dialog.id).unwrap().version);
    }
    ui.set_fields(model(vec![FormField {
        key: "name".into(),
        label: "名称".into(),
        value: "fixture".into(),
        ..Default::default()
    }]));
    ui.set_mcp_type(match kind {
        "http" => 1,
        "sse" => 2,
        _ => 0,
    });
    ui.set_mcp_command("echo".into());
    ui.set_mcp_endpoint("https://fixture.invalid/mcp".into());
    ui.set_mcp_env(model(rows(false, editing)));
    ui.set_mcp_headers(model(rows(true, editing)));
    dialog.loaded = true;
    dialog.rebase(ui);
    dialog
}
fn assert_persisted(f: &mut Fixture, item: &StoredMcp) {
    let bytes = fs::read(f.app.root.join("state.json")).unwrap();
    let state: State = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(state.mcps, f.app.state.mcps);
    let reopened = App::open(&f.app.root, &f.app.home).unwrap();
    assert_eq!(reopened.state.mcps, state.mcps);
    assert!(!item.native_formats);
    let expected = item_config(item);
    for (host_id, deployment) in &item.deployments {
        assert!(deployment.enabled);
        assert!(Path::new(&deployment.target).starts_with(f.temp.path()));
        let host = f.app.host(host_id).unwrap();
        let config = read_config(Path::new(&deployment.target), &host.format).unwrap();
        assert_eq!(config[root_key(host_id, &config)][&item.name], expected);
        assert_eq!(
            deployment.last_hash,
            chu::fs::hash_path(Path::new(&deployment.target))
        );
    }
    let response = serde_json::to_string(&f.app.snapshot().unwrap()).unwrap();
    for sentinel in [ARG, SECRET, "draft-map-value", "old-secret", "old-argument"] {
        assert!(!response.contains(sentinel));
    }
}
fn expected_map(headers: bool, applicable: bool, editing: bool) -> BTreeMap<String, String> {
    if !applicable {
        return if editing {
            old_map(headers)
        } else {
            BTreeMap::new()
        };
    }
    let prefix = if headers { "X-" } else { "" };
    let mut expected = BTreeMap::from([(format!("{prefix}Set"), "draft-map-value".into())]);
    if editing {
        expected.insert(format!("{prefix}Keep"), "old-map-value".into());
    }
    expected
}
fn action_matrix(ui: &AppWindow) {
    for editing in [false, true] {
        for kind in ["stdio", "http", "sse"] {
            for args_action in 0..=2 {
                for secret_action in 0..=2 {
                    let mut f = Fixture::new();
                    let dialog = form(ui, &mut f, kind, editing);
                    ui.set_mcp_args(model(vec![ARG.into()]));
                    ui.set_mcp_args_action(args_action);
                    ui.set_mcp_secret(SECRET.into());
                    ui.set_mcp_secret_action(secret_action);
                    let (method, args) = editor::command(ui, &dialog).unwrap();
                    let serialized = serde_json::to_string(&args).unwrap();
                    assert_eq!(
                        serialized.contains(ARG),
                        kind == "stdio" && args_action == 1
                    );
                    assert_eq!(serialized.contains(SECRET), secret_action == 1);
                    assert!(!serialized.contains("NewKeep\":\"draft-map-value"));
                    f.app.execute(&method, args).unwrap();
                    let item = f.app.state.mcps[0].clone();
                    let expected_args: Vec<String> =
                        if editing && (kind != "stdio" || args_action == 0) {
                            vec!["old-argument".into()]
                        } else if kind == "stdio" && args_action == 1 {
                            vec![ARG.into()]
                        } else {
                            Vec::new()
                        };
                    assert_eq!(item.args, expected_args);
                    assert_eq!(
                        item.secret,
                        if secret_action == 1 {
                            SECRET
                        } else if editing && secret_action == 0 {
                            "old-secret"
                        } else {
                            ""
                        }
                    );
                    assert_eq!(item.env, expected_map(false, kind == "stdio", editing));
                    assert_eq!(item.headers, expected_map(true, kind != "stdio", editing));
                    assert_persisted(&mut f, &item);
                }
            }
        }
    }
}
fn assert_safe(error: anyhow::Error, message: &str) {
    assert_eq!(error.to_string(), message);
    assert_eq!(error.chain().count(), 1);
    for rendered in [
        format!("{error:?}"),
        format!("{error:#?}"),
        format!("{error:#}"),
    ] {
        assert!(!rendered.contains(BAD));
    }
    for cause in error.chain() {
        assert_eq!(cause.to_string(), message);
    }
}
fn invalid_fields() -> Vec<(&'static str, Value)> {
    vec![
        ("args", json!(BAD)),
        ("args", json!([[BAD]])),
        ("args", Value::Null),
        ("secret", json!({"token": BAD})),
        ("env", json!(BAD)),
        ("env", json!({"TOKEN": [BAD]})),
        ("headers", json!({"Authorization": {"token": BAD}})),
        ("headers", json!({"X-Token": 42})),
    ]
}
fn strict_errors(ui: &AppWindow) {
    for editing in [false, true] {
        for kind in ["stdio", "http", "sse"] {
            let mut f = Fixture::new();
            let dialog = form(ui, &mut f, kind, editing);
            let before = fs::read(f.app.root.join("state.json")).ok();
            ui.set_mcp_show_advanced(true);
            for (field, bad) in invalid_fields() {
                ui.set_mcp_advanced(json!({field: bad}).to_string().into());
                let error = editor::command(ui, &dialog).unwrap_err();
                assert_safe(error, &format!("高级配置 {field} 格式无效"));
            }
            for (text, message) in [
                (
                    format!("{{\"secret\":\"{BAD}\""),
                    "高级配置必须是有效的 JSON 对象；输入内容未被写入。",
                ),
                (json!([BAD]).to_string(), "高级配置必须是 JSON 对象。"),
                (
                    json!({BAD: "value"}).to_string(),
                    "高级配置仅支持 args、env、headers、secret。",
                ),
            ] {
                ui.set_mcp_advanced(text.into());
                assert_safe(editor::command(ui, &dialog).unwrap_err(), message);
            }
            assert_eq!(fs::read(f.app.root.join("state.json")).ok(), before);
        }
    }
    let mut f = Fixture::new();
    f.app.add_mcp(base("fixture", "stdio")).unwrap();
    let id = f.app.state.mcps[0].id.clone();
    let before = fs::read(f.app.root.join("state.json")).unwrap();
    for (field, bad) in invalid_fields()
        .into_iter()
        .filter(|(_, bad)| !bad.is_null())
    {
        let mut input = serde_json::to_value(base("invalid", "stdio")).unwrap();
        input[field] = bad.clone();
        assert_safe(
            f.app.execute("AddMCP", json!([input])).unwrap_err(),
            "MCP 新建输入 格式无效",
        );
        let patch = if ["args", "secret"].contains(&field) {
            json!({field: {"action": "set", "value": bad}})
        } else {
            json!({field: bad})
        };
        assert_safe(
            f.app.execute("UpdateMCP", json!([id, patch])).unwrap_err(),
            "MCP 编辑输入 格式无效",
        );
        assert_eq!(fs::read(f.app.root.join("state.json")).unwrap(), before);
    }
}
fn advanced_precedence_and_compatibility(ui: &AppWindow) {
    for editing in [false, true] {
        for kind in ["stdio", "http", "sse"] {
            let mut f = Fixture::new();
            let dialog = form(ui, &mut f, kind, editing);
            ui.set_mcp_args_action(2);
            ui.set_mcp_secret_action(2);
            ui.set_mcp_show_advanced(true);
            ui.set_mcp_advanced(
                json!({
                    "args": ["space separated", "引号\"与反斜线\\"],
                    "env": {"ADV_ENV": "advanced-env"},
                    "headers": {"X-Advanced": "advanced-header"},
                    "secret": "advanced-secret"
                })
                .to_string()
                .into(),
            );
            let (method, args) = editor::command(ui, &dialog).unwrap();
            f.app.execute(&method, args).unwrap();
            let item = f.app.state.mcps[0].clone();
            assert_eq!(item.args, ["space separated", "引号\"与反斜线\\"]);
            assert_eq!(item.secret, "advanced-secret");
            assert_eq!(item.env["ADV_ENV"], "advanced-env");
            assert_eq!(item.headers["X-Advanced"], "advanced-header");
            assert_eq!(item.env.contains_key("Remove"), editing);
            assert_eq!(item.headers.contains_key("X-Remove"), editing);
            assert!(!item.env.contains_key("NewKeep"));
            assert_persisted(&mut f, &item);
        }
    }
    let mut f = Fixture::new();
    f.app
        .execute(
            "AddMCP",
            json!([{
                "name": "legacy-command", "type": "stdio", "command": "echo --legacy",
                "args": ["extra"], "env": null, "headers": null, "secret": null
            }]),
        )
        .unwrap();
    let item = &f.app.state.mcps[0];
    assert_eq!(item.command, "echo");
    assert_eq!(item.args, ["--legacy", "extra"]);
    f.app
        .execute(
            "UpdateMCP",
            json!([item.id, {"args": null, "secret": null, "env": null, "headers": null}]),
        )
        .unwrap();
    assert_eq!(f.app.state.mcps[0].args, ["--legacy", "extra"]);
    f.app
        .execute(
            "AddMCP",
            json!([{
                "name": "remote-headers", "type": "http", "endpoint": "https://fixture.invalid/mcp",
                "headers": {"Authorization": "Bearer fixture", "X-Test": "value"}
            }]),
        )
        .unwrap();
    let item = f.app.state.mcps[1].clone();
    assert_eq!(item.headers["Authorization"], "Bearer fixture");
    f.app.execute("UpdateMCP", json!([item.id, {"headers": {"authorization": {"action": "set", "value": "Bearer replacement"}}}])).unwrap();
    assert_eq!(
        f.app.state.mcps[1].headers["Authorization"],
        "Bearer replacement"
    );
    assert_eq!(f.app.state.mcps[1].headers.len(), 2);
}

#[test]
fn mcp_actions_and_sensitive_json_keep_safe_persistence_and_legacy_compatibility() {
    i_slint_backend_testing::init_no_event_loop();
    let ui = AppWindow::new().unwrap();
    action_matrix(&ui);
    strict_errors(&ui);
    advanced_precedence_and_compatibility(&ui);
}
