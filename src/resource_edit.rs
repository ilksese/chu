//! 安全编辑 DTO 与受管资源运行时事务边界。
use crate::{App, hosts, import_transaction::Journal, models::*};
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn nonnull<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", content = "value", rename_all = "lowercase")]
pub enum Patch<T> {
    #[default]
    Keep,
    Set(T),
    Remove,
}
impl<T> std::fmt::Debug for Patch<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Keep => "Keep",
            Self::Set(_) => "Set([已隐藏])",
            Self::Remove => "Remove",
        })
    }
}

impl<T: Default> Patch<T> {
    pub(crate) fn apply(self, value: &mut T) {
        match self {
            Self::Keep => {}
            Self::Set(next) => *value = next,
            Self::Remove => *value = T::default(),
        }
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpPatch {
    pub name: Option<String>,
    pub description: Option<String>,
    pub r#type: Option<String>,
    pub endpoint: Option<String>,
    pub command: Option<String>,
    pub version: Option<String>,
    #[serde(deserialize_with = "nonnull")]
    pub args: Patch<Vec<String>>,
    #[serde(deserialize_with = "nonnull")]
    pub env: Map<Patch<String>>,
    #[serde(deserialize_with = "nonnull")]
    pub headers: Map<Patch<String>>,
    #[serde(deserialize_with = "nonnull")]
    pub secret: Patch<String>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpEditView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub r#type: String,
    pub endpoint: String,
    pub command: String,
    pub version: String,
    pub args_count: usize,
    pub env_keys: Vec<String>,
    pub header_keys: Vec<String>,
    pub has_secret: bool,
    pub enabled_hosts: Vec<String>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentEditView {
    pub id: String,
    pub version: String,
    #[serde(deserialize_with = "nonnull")]
    pub input: AgentInput,
    #[serde(deserialize_with = "nonnull")]
    pub enabled_hosts: Vec<String>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AgentUpdateInput {
    #[serde(deserialize_with = "nonnull")]
    pub input: AgentInput,
    pub version: Option<String>,
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResourceDeleteInput {
    pub version: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum McpTestStatus {
    CommandAvailable,
    CommandMissing,
    Reachable,
    HttpError,
    #[default]
    ConnectionFailed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct McpTestResult {
    pub status: McpTestStatus,
    pub http_status: Option<u16>,
}

/// 含凭证的地址不能作为可回填字段；空展示值必须通过 Keep 保留。
pub fn safe_endpoint(value: &str) -> String {
    let Ok(url) = url::Url::parse(value) else {
        return String::new();
    };
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        String::new()
    } else {
        value.into()
    }
}

/// 仅显示普通可执行文件名或路径，内联命令可能携带凭证。
pub fn safe_command(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if value.chars().any(|c| c.is_whitespace() || c.is_control())
        || value.contains([
            '=', '@', ';', '&', '|', '<', '>', '$', '`', '\'', '"', '(', ')',
        ])
        || value.contains("://")
        || [
            "token",
            "secret",
            "password",
            "authorization",
            "api_key",
            "api-key",
            "apikey",
            "bearer",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        String::new()
    } else {
        value.into()
    }
}

fn contains_private(value: &str, item: &StoredMcp) -> bool {
    std::iter::once(&item.secret)
        .chain(item.env.values())
        .chain(item.headers.values())
        .chain(item.args.iter())
        .filter(|private| !private.is_empty())
        .any(|private| {
            value.contains(private.as_str())
                || private
                    .split_whitespace()
                    .filter(|s| s.len() >= 4)
                    .any(|part| value.contains(part))
        })
}

pub fn safe_mcp_view(item: &StoredMcp) -> McpView {
    let mut endpoint = safe_endpoint(&item.endpoint);
    let mut command = safe_command(&item.command);
    if contains_private(&endpoint, item) {
        endpoint.clear();
    }
    if contains_private(&command, item) {
        command.clear();
    }
    McpView {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        r#type: item.r#type.clone(),
        endpoint,
        command,
        has_credentials: !item.secret.is_empty()
            || !item.env.is_empty()
            || !item.headers.is_empty(),
        managed: true,
        enabled_on: hosts::deployment_views(&item.deployments).0,
    }
}

pub(crate) fn content_hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn version<T: Serialize>(item: &T) -> Result<String> {
    let bytes = serde_json::to_vec(item).map_err(|_| anyhow!("无法生成资源版本"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub(crate) fn check_version<T: Serialize>(item: &T, expected: Option<&str>) -> Result<()> {
    if let Some(expected) = expected
        && version(item)? != expected
    {
        bail!("资源在打开编辑后发生变化，请重新加载");
    }
    Ok(())
}

pub(crate) fn enabled_hosts(deployments: &Map<Deployment>) -> Vec<String> {
    deployments
        .iter()
        .filter(|(_, d)| d.enabled)
        .map(|(h, _)| h.clone())
        .collect()
}

pub(crate) fn file_target(target: &str) -> Result<PathBuf> {
    if target.trim().is_empty() {
        bail!("部署记录缺少目标路径，已阻止操作");
    }
    if !fs::symlink_metadata(target).is_ok_and(|metadata| metadata.is_file()) {
        bail!("受管目标缺失或不是普通文件，已阻止操作");
    }
    fs::canonicalize(target).map_err(|_| anyhow!("受管目标缺失或不可读取，已阻止操作"))
}

pub(crate) fn watch(journal: &mut Journal, path: &Path) -> Result<()> {
    journal
        .watch(path)
        .map_err(|_| anyhow!("无法保护待修改文件，已阻止操作"))
}

impl App {
    pub(crate) fn edit_state_hash(&self) -> Result<String> {
        let bytes = fs::read(self.root.join("state.json"))
            .map_err(|_| anyhow!("中央状态缺失或不可读取，已阻止操作"))?;
        let state: State =
            serde_json::from_slice(&bytes).map_err(|_| anyhow!("中央状态损坏，已阻止操作"))?;
        if state != self.state {
            bail!("中央状态在 Chu 外发生变化，请重新加载");
        }
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub(crate) fn check_edit_state(&self, hash: &str) -> Result<()> {
        if crate::fs::hash_path(&self.root.join("state.json")) != hash {
            bail!("中央状态在确认后发生变化，已阻止操作");
        }
        Ok(())
    }

    pub(crate) fn resource_transaction(
        &mut self,
        mut journal: Journal,
        commit: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        let old = self.state.clone();
        if let Err(error) = commit(self) {
            self.state = old;
            journal
                .rollback()
                .map_err(|_| anyhow!("资源操作失败，部分文件无法回滚，请检查文件权限"))?;
            return Err(error);
        }
        journal.commit();
        Ok(())
    }
}
