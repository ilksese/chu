//! Normalize imported host MCP formats without exposing arguments, headers or secrets in previews.
use crate::models::*;
use anyhow::{Result, bail};
use serde_json::{Value, json};

fn strings(value: Option<&Value>) -> Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow::anyhow!("参数必须为字符串数组"))
            })
            .collect(),
        _ => bail!("参数必须为字符串数组"),
    }
}
fn map(value: Option<&Value>) -> Result<Map<String>> {
    match value {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(values)) => values
            .iter()
            .map(|(k, v)| {
                v.as_str()
                    .map(|v| (k.clone(), v.into()))
                    .ok_or_else(|| anyhow::anyhow!("环境变量或请求头必须为字符串对象"))
            })
            .collect(),
        _ => bail!("环境变量或请求头必须为字符串对象"),
    }
}
pub fn parse(name: &str, raw: &Value) -> Result<StoredMcp> {
    crate::fs::display_name(name)?;
    let Some(object) = raw.as_object() else {
        bail!("MCP 配置必须为对象");
    };
    let endpoint = raw
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let declared = raw.get("type").and_then(Value::as_str).unwrap_or("");
    let kind = match declared {
        "stdio" | "local" => "stdio",
        "http" | "remote" => "http",
        "sse" => "sse",
        "" => {
            if endpoint.is_empty() {
                "stdio"
            } else {
                "http"
            }
        }
        _ => bail!("不支持的 MCP 类型"),
    };
    let mut args = strings(raw.get("args"))?;
    let command = match raw.get("command") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(_)) => {
            let mut parts = strings(raw.get("command"))?;
            if parts.is_empty() {
                bail!("启动命令不能为空");
            }
            let command = parts.remove(0);
            parts.extend(args);
            args = parts;
            command
        }
        None if kind != "stdio" => String::new(),
        _ => bail!("缺少可用的启动命令"),
    };
    if kind == "stdio" && command.trim().is_empty() {
        bail!("启动命令不能为空");
    }
    if kind != "stdio" {
        let valid = url::Url::parse(&endpoint)
            .is_ok_and(|u| ["https", "http"].contains(&u.scheme()) && u.host_str().is_some());
        if !valid {
            bail!("远程服务地址必须为有效 HTTP/HTTPS URL");
        }
    }
    let env = map(raw.get("env").or_else(|| raw.get("environment")))?;
    let headers = map(raw.get("headers").or_else(|| raw.get("http_headers")))?;
    let secret = match raw.get("token") {
        Some(Value::String(s)) => s.clone(),
        None | Some(Value::Null) => String::new(),
        _ => bail!("访问令牌必须为字符串"),
    };
    let known = [
        "command",
        "args",
        "url",
        "type",
        "env",
        "environment",
        "headers",
        "http_headers",
        "token",
        "description",
        "enabled",
    ];
    let extra = object
        .iter()
        .filter(|(k, _)| !known.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(StoredMcp {
        id: crate::fs::identifier(name, "mcp"),
        name: name.into(),
        description: raw
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        r#type: kind.into(),
        endpoint,
        command,
        args,
        env,
        headers,
        secret,
        native_formats: true,
        extra,
        deployments: Map::new(),
    })
}
pub fn equivalent(a: &StoredMcp, b: &StoredMcp) -> bool {
    a.name == b.name
        && a.r#type == b.r#type
        && a.endpoint == b.endpoint
        && a.command == b.command
        && a.args == b.args
        && a.env == b.env
        && a.headers == b.headers
        && a.secret == b.secret
        && a.extra == b.extra
}
pub fn for_host(item: &StoredMcp, host: &str) -> Result<Value> {
    if host == "jcode" && item.r#type != "stdio" {
        bail!("jcode 目前只支持 stdio MCP");
    }
    let mut root = serde_json::to_value(&item.extra)?;
    if item.r#type == "stdio" {
        if host == "opencode" {
            let mut command = vec![item.command.clone()];
            command.extend(item.args.clone());
            root["type"] = json!("local");
            root["command"] = json!(command);
            root["enabled"] = json!(true);
            if !item.env.is_empty() {
                root["environment"] = json!(item.env);
            }
        } else {
            root["command"] = json!(item.command);
            if !item.args.is_empty() {
                root["args"] = json!(item.args);
            }
            if !item.env.is_empty() {
                root["env"] = json!(item.env);
            }
        }
    } else {
        root["url"] = json!(item.endpoint);
        if host == "opencode" {
            root["type"] = json!("remote");
            root["enabled"] = json!(true);
        } else if host != "codex" {
            root["type"] = json!(item.r#type);
        }
        let mut headers = item.headers.clone();
        if !item.secret.is_empty()
            && !headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("Authorization"))
        {
            headers.insert("Authorization".into(), format!("Bearer {}", item.secret));
        }
        if !headers.is_empty() {
            root[if host == "codex" {
                "http_headers"
            } else {
                "headers"
            }] = json!(headers);
        }
    }
    Ok(root)
}
