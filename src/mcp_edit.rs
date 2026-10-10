//! MCP 编辑先合并全部已记录部署路径，再作为一个运行时事务提交。
use crate::{
    App,
    fs::{display_name, hash_path},
    import_transaction::Journal,
    mcp::{PathBufExt, item_config, read_config, root_key, write_config},
    mcp_native,
    models::*,
    resource_edit::{self, McpEditView, McpPatch, Patch, ResourceDeleteInput},
};
use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

struct ConfigPlan {
    path: PathBuf,
    format: String,
    original: Value,
    root: Value,
    hash: String,
}

fn render(item: &StoredMcp, host: &str) -> Result<Value> {
    if host == "jcode" && item.r#type != "stdio" {
        bail!("jcode 只支持 stdio MCP，请先停用 jcode 部署");
    }
    if item.native_formats {
        mcp_native::for_host(item, host).map_err(|_| anyhow!("MCP 与启用宿主不兼容，已阻止操作"))
    } else {
        Ok(item_config(item))
    }
}

fn read_target(host: &Host, path: &std::path::Path) -> Result<(String, Value)> {
    if std::fs::canonicalize(&host.config_path).is_ok_and(|current| current == path) {
        return read_config(path, &host.format)
            .map(|root| (host.format.clone(), root))
            .map_err(|_| anyhow!("无法解析宿主配置，已阻止操作"));
    }
    // 宿主路径改过后仍操作部署记录；只有唯一可解析的原格式才能继续。
    match (read_config(path, "json"), read_config(path, "toml")) {
        (Ok(root), Err(_)) => Ok(("json".into(), root)),
        (Err(_), Ok(root)) => Ok(("toml".into(), root)),
        _ => bail!("已记录宿主配置的格式不明确，已阻止操作"),
    }
}

fn matches(old: &StoredMcp, actual: &Value, expected: &Value) -> bool {
    let Ok(actual_item) = mcp_native::parse(&old.name, actual) else {
        return false;
    };
    let Ok(expected_item) = mcp_native::parse(&old.name, expected) else {
        return false;
    };
    // 原生解析刻意不包含 enabled；显式停用或无效值均视为漂移。
    if actual
        .get("enabled")
        .is_some_and(|enabled| enabled != &Value::Bool(true))
    {
        return false;
    }
    let rendered_matches = mcp_native::equivalent(&actual_item, &expected_item)
        && (expected.get("enabled").is_none() || actual.get("enabled").is_some());
    // 原位导入会保留宿主原始字段，token 与 Authorization 等形态不必等同于再次渲染。
    rendered_matches || (old.native_formats && mcp_native::equivalent(&actual_item, old))
}

fn patch_keys(values: &mut Map<String>, patches: Map<Patch<String>>) -> Result<()> {
    for (key, patch) in patches {
        if key.trim().is_empty() || key.contains(['\0', '\r', '\n']) {
            bail!("环境变量或请求头名称无效");
        }
        match patch {
            Patch::Keep => {}
            Patch::Set(value) => {
                values.insert(key, value);
            }
            Patch::Remove => {
                values.remove(&key);
            }
        }
    }
    Ok(())
}

fn patch_headers(values: &mut Map<String>, patches: Map<Patch<String>>) -> Result<()> {
    let mut patched = std::collections::BTreeSet::new();
    for (key, patch) in patches {
        if !patched.insert(key.to_ascii_lowercase()) {
            bail!("请求头修改中存在大小写重复名称，已阻止操作");
        }
        if key.trim().is_empty() || key.contains(['\0', '\r', '\n']) {
            bail!("请求头名称无效");
        }
        let existing: Vec<_> = values
            .keys()
            .filter(|old| old.eq_ignore_ascii_case(&key))
            .cloned()
            .collect();
        match patch {
            Patch::Keep => {}
            Patch::Remove => values.retain(|old, _| !old.eq_ignore_ascii_case(&key)),
            Patch::Set(value) => {
                values.retain(|old, _| !old.eq_ignore_ascii_case(&key));
                values.insert(existing.into_iter().next().unwrap_or(key), value);
            }
        }
    }
    let mut keys = std::collections::BTreeSet::new();
    if values
        .keys()
        .any(|key| !keys.insert(key.to_ascii_lowercase()))
    {
        bail!("请求头名称大小写重复，已阻止操作");
    }
    Ok(())
}

fn apply_patch(mut item: StoredMcp, patch: McpPatch) -> Result<StoredMcp> {
    if let Some(name) = patch.name {
        item.name = display_name(&name)?;
    }
    if let Some(description) = patch.description {
        item.description = description;
    }
    if let Some(kind) = patch.r#type {
        item.r#type = kind.trim().to_ascii_lowercase();
    }
    if let Some(endpoint) = patch.endpoint {
        if endpoint != item.endpoint
            && !url::Url::parse(&endpoint)
                .is_ok_and(|url| url.scheme() == "https" && url.host_str().is_some())
        {
            bail!("新的远程 MCP 地址必须是有效 HTTPS URL");
        }
        item.endpoint = endpoint;
    }
    if let Some(command) = patch.command {
        if command != item.command && command.chars().any(char::is_whitespace) {
            bail!("启动命令与参数必须分开填写，请使用参数列表");
        }
        item.command = command;
    }
    patch.args.apply(&mut item.args);
    patch_keys(&mut item.env, patch.env)?;
    patch_headers(&mut item.headers, patch.headers)?;
    patch.secret.apply(&mut item.secret);
    if !["stdio", "http", "sse"].contains(&item.r#type.as_str()) {
        bail!("MCP 类型必须是 stdio、http 或 sse");
    }
    if item.r#type == "stdio" {
        if item.command.trim().is_empty() || item.command.contains('\0') {
            bail!("stdio MCP 必须填写有效启动命令");
        }
    } else if !url::Url::parse(&item.endpoint)
        .is_ok_and(|url| ["http", "https"].contains(&url.scheme()) && url.host_str().is_some())
    {
        bail!("远程 MCP 必须填写有效 HTTP/HTTPS URL");
    }
    Ok(item)
}

impl App {
    pub fn read_mcp_edit(&self, id: &str) -> Result<McpEditView> {
        self.edit_state_hash()?;
        let item = self
            .state
            .mcps
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("未知的 MCP"))?;
        let safe = resource_edit::safe_mcp_view(item);
        Ok(McpEditView {
            id: item.id.clone(),
            name: item.name.clone(),
            description: item.description.clone(),
            r#type: item.r#type.clone(),
            endpoint: safe.endpoint,
            command: safe.command,
            version: resource_edit::version(item)?,
            args_count: item.args.len(),
            env_keys: item.env.keys().cloned().collect(),
            header_keys: item.headers.keys().cloned().collect(),
            has_secret: !item.secret.is_empty(),
            enabled_hosts: resource_edit::enabled_hosts(&item.deployments),
        })
    }

    pub fn update_mcp(&mut self, id: &str, patch: McpPatch) -> Result<()> {
        let old = self
            .state
            .mcps
            .iter()
            .find(|item| item.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("未知的 MCP"))?;
        resource_edit::check_version(&old, patch.version.as_deref())?;
        let updated = apply_patch(old, patch)?;
        if self
            .state
            .mcps
            .iter()
            .any(|item| item.id != id && item.name == updated.name)
        {
            bail!("MCP 名称已存在，已阻止操作");
        }
        self.edit_mcp(id, Some(updated))
    }

    pub fn delete_mcp(&mut self, id: &str, input: ResourceDeleteInput) -> Result<()> {
        let old = self
            .state
            .mcps
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("未知的 MCP"))?;
        resource_edit::check_version(old, input.version.as_deref())?;
        self.edit_mcp(id, None)
    }

    fn mcp_edit_plans(
        &self,
        old: &StoredMcp,
        updated: Option<&StoredMcp>,
    ) -> Result<BTreeMap<PathBuf, ConfigPlan>> {
        let mut plans = BTreeMap::<PathBuf, ConfigPlan>::new();
        let mut slots = BTreeMap::<(PathBuf, String), Option<Value>>::new();
        for (host_id, deployment) in old.deployments.iter().filter(|(_, d)| d.enabled) {
            let host = self.host(host_id)?;
            if deployment.mode != "native" {
                bail!("MCP 部署模式不兼容，已阻止操作");
            }
            let path = resource_edit::file_target(&deployment.target)?;
            let hash = hash_path(&path);
            if deployment.last_hash.is_empty() || hash != deployment.last_hash {
                bail!("宿主配置在 Chu 外发生变化，已阻止操作");
            }
            let (format, original) = read_target(&host, &path)?;
            if !plans.contains_key(&path) {
                plans.insert(
                    path.clone(),
                    ConfigPlan {
                        path: path.clone(),
                        format: format.clone(),
                        original: original.clone(),
                        root: original,
                        hash,
                    },
                );
            }
            let plan = plans.get_mut(&path).unwrap();
            if plan.format != format {
                bail!("共享配置路径的宿主文件格式不兼容，已阻止操作");
            }
            let key = root_key(host_id, &plan.original).to_owned();
            let servers = plan
                .original
                .get(&key)
                .and_then(Value::as_object)
                .ok_or_else(|| anyhow!("受管 MCP 配置节点缺失或不兼容，已阻止操作"))?;
            let actual = servers
                .get(&old.name)
                .ok_or_else(|| anyhow!("受管 MCP 已从宿主配置中移除，已阻止操作"))?;
            let expected = render(old, host_id)?;
            if !matches(old, actual, &expected) {
                bail!("受管 MCP 内容或启用状态与记录不同，已阻止操作");
            }
            let replacement = updated.map(|item| render(item, host_id)).transpose()?;
            if let Some(updated) = updated
                && updated.name != old.name
                && servers.contains_key(&updated.name)
            {
                bail!("宿主配置中已存在同名 MCP，已阻止覆盖");
            }
            let slot = (path, key.clone());
            if let Some(existing) = slots.get(&slot) {
                if existing != &replacement {
                    bail!("共享配置路径的宿主格式不兼容，已阻止操作");
                }
                continue;
            }
            slots.insert(slot, replacement.clone());
            let mut replacement = replacement;
            if let Some(value) = replacement.as_mut() {
                // 保留不参与标准化的宿主描述与显式 enabled 形式。
                if let Some(description) = actual.get("description") {
                    value["description"] =
                        if updated.is_some_and(|item| item.description != old.description) {
                            Value::String(updated.unwrap().description.clone())
                        } else {
                            description.clone()
                        };
                }
                if actual.get("enabled").is_some() {
                    value["enabled"] = Value::Bool(true);
                }
            }
            let servers = plan
                .root
                .get_mut(&key)
                .and_then(Value::as_object_mut)
                .unwrap();
            servers.remove(&old.name);
            if let (Some(item), Some(value)) = (updated, replacement) {
                servers.insert(item.name.clone(), value);
            }
        }
        for plan in plans.values() {
            // 同一实际配置文件的全部受管部署必须认可现有指纹。
            for deployment in self
                .state
                .mcps
                .iter()
                .flat_map(|item| item.deployments.values())
                .filter(|d| d.enabled)
            {
                if std::fs::canonicalize(&deployment.target).is_ok_and(|path| path == plan.path)
                    && (deployment.last_hash.is_empty() || deployment.last_hash != plan.hash)
                {
                    bail!("共享宿主配置的受管记录已漂移，已阻止操作");
                }
            }
            crate::mcp::config_bytes(&plan.format, &plan.root)
                .map_err(|_| anyhow!("目标宿主配置无法序列化，已阻止操作"))?;
        }
        Ok(plans)
    }

    fn edit_mcp(&mut self, id: &str, updated: Option<StoredMcp>) -> Result<()> {
        let state_hash = self.edit_state_hash()?;
        let index = self
            .state
            .mcps
            .iter()
            .position(|item| item.id == id)
            .unwrap();
        let plans = self.mcp_edit_plans(&self.state.mcps[index], updated.as_ref())?;
        let mut state = self.state.clone();
        if let Some(item) = updated {
            state.mcps[index] = item;
        } else {
            state.mcps.remove(index);
        }
        let mut journal = Journal::new();
        let prepared = (|| -> Result<()> {
            resource_edit::watch(&mut journal, &self.root.join("state.json"))?;
            for plan in plans.values().filter(|p| p.root != p.original) {
                resource_edit::watch(&mut journal, &plan.path)?;
                resource_edit::watch(&mut journal, &PathBufExt::suffix(&plan.path, ".chu.bak"))?;
            }
            self.check_edit_state(&state_hash)?;
            for plan in plans.values() {
                if hash_path(&plan.path) != plan.hash {
                    bail!("宿主配置在确认后发生变化，已阻止操作");
                }
            }
            crate::operation::begin_commit()
        })();
        if let Err(error) = prepared {
            journal.commit();
            return Err(error);
        }
        self.resource_transaction(journal, move |app| {
            for plan in plans.values().filter(|p| p.root != p.original) {
                write_config(&plan.path, &plan.format, &plan.root)
                    .map_err(|_| anyhow!("写入宿主配置失败，正在恢复原有文件"))?;
            }
            for deployment in state
                .mcps
                .iter_mut()
                .flat_map(|item| item.deployments.values_mut())
                .filter(|d| d.enabled)
            {
                if let Ok(path) = std::fs::canonicalize(&deployment.target)
                    && plans.contains_key(&path)
                {
                    deployment.last_hash = hash_path(&path);
                }
            }
            app.state = state;
            app.save()
                .map_err(|_| anyhow!("保存中央状态失败，正在恢复原有文件"))
        })
    }
}
