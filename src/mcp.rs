use crate::{App, fs::*, models::*};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, path::Path, time::Duration};

pub fn read_config(path: &Path, format: &str) -> Result<Value> {
    let data = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(json!({})),
        Err(e) => return Err(e.into()),
    };
    let value = if format == "toml" {
        serde_json::to_value(toml::from_str::<toml::Value>(&data)?)?
    } else if data.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&data)?
    };
    if !value.is_object() {
        bail!("宿主配置必须是对象");
    }
    Ok(value)
}
pub fn write_config(path: &Path, format: &str, value: &Value) -> Result<()> {
    let data = if format == "toml" {
        toml::to_string_pretty(value)?.into_bytes()
    } else {
        let mut d = serde_json::to_vec_pretty(value)?;
        d.push(b'\n');
        d
    };
    if exists(path) {
        copy_file(
            path,
            &path.with_file_name(format!(
                "{}.chu.bak",
                path.file_name().unwrap_or_default().to_string_lossy()
            )),
        )?;
    }
    atomic_write(path, &data)
}
pub fn root_key<'a>(host: &str, root: &Value) -> &'a str {
    match host {
        "claude" => "mcpServers",
        "codex" => "mcp_servers",
        "jcode" if root.get("mcpServers").is_some() => "mcpServers",
        "jcode" => "servers",
        _ => "mcp",
    }
}
pub fn item_config(s: &StoredMcp) -> Value {
    let mut value = if s.r#type == "stdio" {
        json!({"command": s.command})
    } else {
        json!({"url": s.endpoint, "type": s.r#type})
    };
    if !s.args.is_empty() && s.r#type == "stdio" {
        value["args"] = json!(s.args);
    }
    if !s.env.is_empty() {
        value["env"] = json!(s.env);
    }
    if !s.headers.is_empty() {
        value["headers"] = json!(s.headers);
    }
    if !s.secret.is_empty() {
        value["token"] = json!(s.secret);
    }
    value
}
impl App {
    pub fn add_mcp(&mut self, mut input: McpInput) -> Result<()> {
        input.name = display_name(&input.name)?;
        input.r#type = input.r#type.trim().to_lowercase();
        if !["stdio", "http", "sse"].contains(&input.r#type.as_str()) {
            bail!("MCP 类型必须是 stdio、http 或 sse");
        }
        if input.r#type == "stdio" {
            let parts: Vec<_> = input.command.split_whitespace().collect();
            if parts.is_empty() {
                bail!("stdio MCP 必须填写启动命令");
            }
            let mut args: Vec<_> = parts[1..].iter().map(|s| s.to_string()).collect();
            args.extend(input.args);
            input.args = args;
            input.command = parts[0].into();
        } else {
            let u = url::Url::parse(&input.endpoint).context("远程 MCP 必须填写有效 HTTPS URL")?;
            if u.scheme() != "https" || u.host_str().is_none() {
                bail!("远程 MCP 必须填写有效 HTTPS URL");
            }
        }
        let id = identifier(&input.name, "mcp");
        if self
            .state
            .mcps
            .iter()
            .any(|m| m.name == input.name || m.id == id)
        {
            bail!("MCP 已存在");
        }
        self.state.mcps.push(StoredMcp {
            id,
            name: input.name,
            description: input.description,
            r#type: input.r#type,
            endpoint: input.endpoint,
            command: input.command,
            args: input.args,
            env: input.env,
            headers: input.headers,
            secret: input.secret,
            deployments: Map::new(),
            native_formats: false,
            extra: Map::new(),
        });
        self.save()
    }
    pub fn toggle_mcp(&mut self, id: &str, host: &str, enabled: bool) -> Result<()> {
        let host = self.host(host)?;
        let path = Path::new(&host.config_path);
        let index = self
            .state
            .mcps
            .iter()
            .position(|m| m.id == id)
            .context("未知的 MCP")?;
        let item = self.state.mcps[index].clone();
        if enabled && host.id == "jcode" && item.r#type != "stdio" {
            bail!("jcode 目前只支持 stdio MCP");
        }
        let hash = hash_path(path);
        if self
            .state
            .mcps
            .iter()
            .filter_map(|m| m.deployments.get(&host.id))
            .any(|d| d.enabled && !d.last_hash.is_empty() && d.last_hash != hash)
        {
            bail!("宿主配置在 Chu 外发生变化，已阻止写入");
        }
        let deployed = item.deployments.get(&host.id).is_some_and(|d| d.enabled);
        if enabled == deployed {
            return Ok(());
        }
        let mut root = read_config(path, &host.format).context("无法解析宿主配置")?;
        let key = root_key(&host.id, &root);
        if root.get(key).is_none() {
            root[key] = json!({});
        }
        let servers = root[key]
            .as_object_mut()
            .context("MCP 配置节点不是对象，未覆盖原值")?;
        if enabled {
            if let Some(existing) = servers.get_mut(&item.name) {
                if !item.native_formats
                    || !crate::mcp_native::parse(&item.name, existing)
                        .is_ok_and(|v| crate::mcp_native::equivalent(&v, &item))
                {
                    bail!("目标配置中已存在不同内容的同名 MCP: {}", item.name);
                }
                if existing.get("enabled").is_some() {
                    existing["enabled"] = json!(true);
                }
            } else {
                servers.insert(
                    item.name.clone(),
                    if item.native_formats {
                        crate::mcp_native::for_host(&item, &host.id)?
                    } else {
                        item_config(&item)
                    },
                );
            }
        } else {
            servers.remove(&item.name);
        }
        write_config(path, &host.format, &root)?;
        if enabled {
            self.state.mcps[index].deployments.insert(
                host.id.clone(),
                Deployment {
                    enabled: true,
                    mode: "native".into(),
                    target: host.config_path.clone(),
                    last_hash: String::new(),
                },
            );
        } else {
            self.state.mcps[index].deployments.remove(&host.id);
        }
        let hash = hash_path(path);
        for item in &mut self.state.mcps {
            if let Some(dep) = item.deployments.get_mut(&host.id)
                && dep.enabled
            {
                dep.last_hash = hash.clone();
            }
        }
        self.save()
    }
    pub(crate) fn discovered_mcps(&self, hosts: &[Host]) -> Vec<McpView> {
        let mut found = Vec::new();
        for host in hosts {
            if let Ok(root) = read_config(Path::new(&host.config_path), &host.format)
                && let Some(servers) = root
                    .get(root_key(&host.id, &root))
                    .and_then(|v| v.as_object())
            {
                for (name, server) in servers {
                    if self.state.mcps.iter().any(|m| m.name == *name) {
                        continue;
                    }
                    let endpoint = server["url"].as_str().unwrap_or("");
                    found.push(McpView {
                        id: format!("discovered-mcp-{}-{}", host.id, slug(name)),
                        name: name.clone(),
                        r#type: if endpoint.is_empty() {
                            "stdio"
                        } else {
                            server["type"].as_str().unwrap_or("http")
                        }
                        .into(),
                        endpoint: endpoint.into(),
                        command: server["command"].as_str().unwrap_or("").into(),
                        enabled_on: [(host.id.clone(), true)].into(),
                        ..McpView::default()
                    });
                }
            }
        }
        found
    }
    pub fn test_mcp(&self, id: &str) -> Result<String> {
        let item = self
            .state
            .mcps
            .iter()
            .find(|m| m.id == id)
            .context("找不到 MCP")?;
        if item.r#type == "stdio" {
            return Ok(if crate::hosts::command_present(&item.command) {
                "命令可用。Chu 不会在测试时启动服务。".into()
            } else {
                format!("找不到命令 {:?}", item.command)
            });
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut request = client.get(&item.endpoint);
        for (key, value) in &item.headers {
            request = request.header(key, value);
        }
        if !item.secret.is_empty()
            && !item
                .headers
                .keys()
                .any(|k| k.eq_ignore_ascii_case("Authorization"))
        {
            request = request.bearer_auth(&item.secret);
        }
        match request.send() {
            Ok(r) => Ok(format!(
                "{}，HTTP {}",
                if r.status().as_u16() < 400 {
                    "连接成功"
                } else {
                    "服务返回"
                },
                r.status().as_u16()
            )),
            Err(_) => Ok("连接失败：网络连接异常或超时".into()),
        }
    }
    pub fn restore_backup(&mut self, host: &str) -> Result<()> {
        let host = self.host(host)?;
        let path = Path::new(&host.config_path);
        let backup = PathBufExt::suffix(path, ".chu.bak");
        if !backup.is_file() {
            bail!("没有可恢复的备份");
        }
        if exists(path) {
            copy_file(path, &PathBufExt::suffix(path, ".chu.before-restore"))?;
        }
        copy_file(&backup, path)?;
        let hash = hash_path(path);
        for m in &mut self.state.mcps {
            if let Some(d) = m.deployments.get_mut(&host.id)
                && d.enabled
            {
                d.last_hash = hash.clone();
            }
        }
        self.save()
    }
}
pub struct PathBufExt;
impl PathBufExt {
    pub fn suffix(path: &Path, suffix: &str) -> std::path::PathBuf {
        let mut p = path.as_os_str().to_os_string();
        p.push(suffix);
        p.into()
    }
}
