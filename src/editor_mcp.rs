//! MCP 草稿只保存新输入；已有秘密以显式 patch 操作保留或移除。
use crate::{
    editor::Dialog,
    mcp_input::advanced_fields,
    presenter::model,
    resource_edit::{McpEditView, Patch},
    ui::{AppWindow, KeyValueRow},
};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use slint::Model;
use std::collections::BTreeMap;
pub fn reset(ui: &AppWindow) {
    ui.set_mcp_type(0);
    ui.set_mcp_command("".into());
    ui.set_mcp_endpoint("".into());
    ui.set_mcp_args(model(Vec::new()));
    ui.set_mcp_env(model(Vec::new()));
    ui.set_mcp_headers(model(Vec::new()));
    ui.set_mcp_args_action(1);
    ui.set_mcp_args_stored_count(0);
    ui.set_mcp_secret_action(0);
    ui.set_mcp_secret_stored(false);
    ui.set_mcp_secret("".into());
    ui.set_mcp_advanced("".into());
    ui.set_mcp_show_advanced(false);
}
pub fn load(ui: &AppWindow, view: &McpEditView) {
    let mut fields: Vec<_> = ui.get_fields().iter().collect();
    for f in &mut fields {
        if f.key == "name" {
            f.value = view.name.clone().into();
        } else if f.key == "description" {
            f.value = view.description.clone().into();
        }
    }
    ui.set_fields(model(fields));
    ui.set_mcp_type(match view.r#type.as_str() {
        "http" => 1,
        "sse" => 2,
        _ => 0,
    });
    ui.set_mcp_endpoint(view.endpoint.clone().into());
    ui.set_mcp_command(view.command.clone().into());
    ui.set_mcp_args_action(0);
    ui.set_mcp_args_stored_count(view.args_count as i32);
    ui.set_mcp_secret_action(0);
    ui.set_mcp_secret_stored(view.has_secret);
    let rows = |keys: &[String]| {
        keys.iter()
            .map(|key| KeyValueRow {
                id: key.clone().into(),
                key: key.clone().into(),
                value: "".into(),
                action: 0,
                stored: true,
            })
            .collect()
    };
    ui.set_mcp_env(model(rows(&view.env_keys)));
    ui.set_mcp_headers(model(rows(&view.header_keys)));
}
pub fn draft(ui: &AppWindow) -> Value {
    let rows = |entries: slint::ModelRc<KeyValueRow>| {
        entries.iter().map(|r| json!({"id":r.id.as_str(),"key":r.key.as_str(),"value":r.value.as_str(),"action":r.action,"stored":r.stored})).collect::<Vec<_>>()
    };
    json!({"type":ui.get_mcp_type(),"command":ui.get_mcp_command().as_str(),"endpoint":ui.get_mcp_endpoint().as_str(),
        "argsAction":ui.get_mcp_args_action(),"args":ui.get_mcp_args().iter().map(|s|s.to_string()).collect::<Vec<_>>(),
        "env":rows(ui.get_mcp_env()),"headers":rows(ui.get_mcp_headers()),"secretAction":ui.get_mcp_secret_action(),"secret":ui.get_mcp_secret().as_str(),
        "advanced":ui.get_mcp_advanced().as_str(),"showAdvanced":ui.get_mcp_show_advanced()})
}
fn resolve<T>(action: i32, value: T) -> Patch<T> {
    match action {
        1 => Patch::Set(value),
        2 => Patch::Remove,
        _ => Patch::Keep,
    }
}
fn action_input<T: Default + serde::Serialize>(action: i32, value: T, editing: bool) -> Value {
    let patch = resolve(action, value);
    if editing {
        json!(patch)
    } else {
        let mut value = T::default();
        patch.apply(&mut value);
        json!(value)
    }
}
fn key_values(entries: slint::ModelRc<KeyValueRow>, headers: bool, editing: bool) -> Result<Value> {
    let mut seen = std::collections::BTreeSet::new();
    let mut values = serde_json::Map::new();
    for row in entries.iter() {
        let key = row.key.trim();
        if key.is_empty() {
            bail!("键名不能为空，请填写或移除该行。");
        }
        let canonical = if headers {
            key.to_ascii_lowercase()
        } else {
            key.into()
        };
        if !seen.insert(canonical) {
            bail!("键名重复，请为每项使用唯一键名。");
        }
        if editing && row.stored && row.key != row.id {
            values.insert(row.id.to_string(), json!(Patch::<String>::Remove));
        }
        let patch = resolve(row.action, row.value.to_string());
        if editing {
            values.insert(key.into(), json!(patch));
        } else if let Patch::Set(value) = patch {
            values.insert(key.into(), json!(value));
        }
    }
    Ok(Value::Object(values))
}
pub fn command(
    ui: &AppWindow,
    dialog: &Dialog,
    fields: &BTreeMap<String, String>,
) -> Result<(String, Value)> {
    let get = |key: &str| fields.get(key).map(String::as_str).unwrap_or("");
    let editing = dialog.method == "UpdateMCP";
    let kind = match ui.get_mcp_type() {
        1 => "http",
        2 => "sse",
        _ => "stdio",
    };
    if get("name").trim().is_empty() {
        bail!("名称不能为空。");
    }
    let args: Vec<_> = ui.get_mcp_args().iter().map(|s| s.to_string()).collect();
    let mut input = json!({"name":get("name"),"description":get("description"),"type":kind});
    let baseline_kind = dialog
        .baseline
        .get("mcp")
        .and_then(|v| v.get("type"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if editing && baseline_kind != ui.get_mcp_type() as i64 {
        if kind == "stdio" && ui.get_mcp_command().trim().is_empty() {
            bail!("切换到 stdio 时必须填写启动命令。");
        }
        if kind != "stdio" && ui.get_mcp_endpoint().trim().is_empty() {
            bail!("切换到远程类型时必须填写服务地址。");
        }
    }
    if editing {
        input["version"] = json!(dialog.version);
        let baseline = dialog.baseline.get("mcp").unwrap_or(&Value::Null);
        for (key, value) in [
            ("command", ui.get_mcp_command()),
            ("endpoint", ui.get_mcp_endpoint()),
        ] {
            if baseline.get(key).and_then(Value::as_str) != Some(value.as_str()) {
                input[key] = json!(value.as_str());
            }
        }
    } else {
        input["command"] = json!(ui.get_mcp_command().as_str());
        input["endpoint"] = json!(ui.get_mcp_endpoint().as_str());
        if kind == "stdio" && ui.get_mcp_command().trim().is_empty() {
            bail!("启动命令不能为空。");
        }
        if kind != "stdio" && ui.get_mcp_endpoint().trim().is_empty() {
            bail!("服务地址不能为空。");
        }
    }
    input["args"] = action_input(
        if kind == "stdio" {
            ui.get_mcp_args_action()
        } else {
            0
        },
        args,
        editing,
    );
    input["env"] = if kind == "stdio" {
        key_values(ui.get_mcp_env(), false, editing)?
    } else {
        json!({})
    };
    input["headers"] = if kind != "stdio" {
        key_values(ui.get_mcp_headers(), true, editing)?
    } else {
        json!({})
    };
    input["secret"] = action_input(
        ui.get_mcp_secret_action(),
        ui.get_mcp_secret().to_string(),
        editing,
    );
    if ui.get_mcp_show_advanced() && !ui.get_mcp_advanced().trim().is_empty() {
        let object = advanced_fields(ui.get_mcp_advanced().as_str())?;
        for (key, value) in object {
            if editing {
                if ["env", "headers"].contains(&key.as_str()) {
                    let entries = value.as_object().expect("高级字段已通过严格类型校验");
                    input[&key] = Value::Object(
                        entries
                            .iter()
                            .map(|(k, v)| (k.clone(), json!(resolve(1, v.clone()))))
                            .collect(),
                    );
                } else {
                    input[&key] = json!(resolve(1, value));
                }
            } else {
                input[&key] = value;
            }
        }
    }
    Ok((
        dialog.method.clone(),
        if editing {
            json!([dialog.id, input])
        } else {
            json!([input])
        },
    ))
}

pub(crate) fn add_entry(ui: &AppWindow, kind: &str) {
    let mut rows: Vec<_> = if kind == "env" {
        ui.get_mcp_env()
    } else {
        ui.get_mcp_headers()
    }
    .iter()
    .collect();
    rows.push(KeyValueRow {
        id: "".into(),
        key: "".into(),
        value: "".into(),
        action: 1,
        stored: false,
    });
    if kind == "env" {
        ui.set_mcp_env(model(rows));
    } else {
        ui.set_mcp_headers(model(rows));
    }
}
pub(crate) fn remove_entry(ui: &AppWindow, kind: &str, index: i32) {
    let mut rows: Vec<_> = if kind == "env" {
        ui.get_mcp_env()
    } else {
        ui.get_mcp_headers()
    }
    .iter()
    .collect();
    if let Some(row) = rows.get_mut(index.max(0) as usize) {
        if row.stored {
            row.action = 2;
            row.value = "".into();
        } else {
            rows.remove(index.max(0) as usize);
        }
    }
    if kind == "env" {
        ui.set_mcp_env(model(rows));
    } else {
        ui.set_mcp_headers(model(rows));
    }
}
