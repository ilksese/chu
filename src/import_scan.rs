//! On-demand host scans. Public candidates contain labels/paths only, never raw MCP credentials.
use crate::{
    App,
    fs::*,
    mcp::{read_config, root_key},
    mcp_native,
    models::*,
};
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(crate) enum Resource {
    Skill { source: PathBuf, hash: String },
    Mcp(Box<StoredMcp>),
}
pub(crate) struct Entry {
    pub view: ImportCandidate,
    pub resource: Option<Resource>,
}
fn identity(kind: &str, host: &str, name: &str, fingerprint: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(kind, host, name, fingerprint)).unwrap())
    )
}
impl App {
    pub fn list_import_candidates(&self, kind: &str) -> Result<ImportScan> {
        Ok(self.scan_imports(kind)?.0)
    }
    pub(crate) fn scan_imports(&self, kind: &str) -> Result<(ImportScan, Vec<Entry>)> {
        if !["skills", "mcps"].contains(&kind) {
            bail!("只支持导入 Skills 或 MCP");
        }
        let mut scan = ImportScan::default();
        let mut entries = Vec::new();
        for host in self.hosts() {
            if kind == "skills" {
                let files = match fs::read_dir(&host.skill_path) {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => {
                        scan.warnings
                            .push(format!("{} 的 Skills 目录无法读取", host.name));
                        continue;
                    }
                };
                for file in files.flatten() {
                    let name = file.file_name().to_string_lossy().into_owned();
                    if name.starts_with(".env")
                        || !file.path().is_dir()
                        || !file.path().join("SKILL.md").is_file()
                    {
                        continue;
                    }
                    let Ok(source) = file.path().canonicalize() else {
                        continue;
                    };
                    let hash = hash_skill_path(&source);
                    let id = identifier(&name, "skill");
                    let reason = if safe_name(&name).is_err() {
                        "名称不合法"
                    } else if self.state.skills.iter().any(|s| s.name == name) {
                        "Chu 中已有同名资源"
                    } else if self.state.skills.iter().any(|s| s.id == id) {
                        "与 Chu 中的资源标识冲突"
                    } else {
                        ""
                    };
                    let view = ImportCandidate {
                        id: identity(kind, &host.id, &name, &hash),
                        host_id: host.id.clone(),
                        host_name: host.name.clone(),
                        name,
                        description: crate::sources::description(&source),
                        source: file.path().to_string_lossy().into(),
                        r#type: "skill".into(),
                        available: reason.is_empty(),
                        reason: reason.into(),
                        ..Default::default()
                    };
                    entries.push(Entry {
                        view,
                        resource: Some(Resource::Skill { source, hash }),
                    });
                }
            } else {
                let root = match read_config(Path::new(&host.config_path), &host.format) {
                    Ok(v) => v,
                    Err(_) => {
                        scan.warnings
                            .push(format!("{} 的 MCP 配置无法解析或读取", host.name));
                        continue;
                    }
                };
                let Some(value) = root.get(root_key(&host.id, &root)) else {
                    continue;
                };
                let Some(servers) = value.as_object() else {
                    scan.warnings
                        .push(format!("{} 的 MCP 节点不是对象", host.name));
                    continue;
                };
                for (name, raw) in servers {
                    let parsed = mcp_native::parse(name, raw);
                    let id = identifier(name, "mcp");
                    let reason = if self.state.mcps.iter().any(|s| s.name == *name) {
                        "Chu 中已有同名资源".into()
                    } else if self.state.mcps.iter().any(|s| s.id == id) {
                        "与 Chu 中的资源标识冲突".into()
                    } else {
                        parsed
                            .as_ref()
                            .err()
                            .map(ToString::to_string)
                            .unwrap_or_default()
                    };
                    let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(raw)?));
                    let view = ImportCandidate {
                        id: identity(kind, &host.id, name, &fingerprint),
                        host_id: host.id.clone(),
                        host_name: host.name.clone(),
                        name: name.clone(),
                        description: parsed
                            .as_ref()
                            .map(|s| s.description.clone())
                            .unwrap_or_default(),
                        source: host.config_path.clone(),
                        r#type: parsed
                            .as_ref()
                            .map(|s| s.r#type.clone())
                            .unwrap_or_default(),
                        has_credentials: parsed.as_ref().is_ok_and(|s| {
                            !s.secret.is_empty() || !s.env.is_empty() || !s.headers.is_empty()
                        }),
                        available: reason.is_empty(),
                        reason,
                    };
                    entries.push(Entry {
                        view,
                        resource: parsed.ok().map(|item| Resource::Mcp(Box::new(item))),
                    });
                }
            }
        }
        entries.sort_by(|a, b| {
            (!a.view.available, &a.view.host_name, &a.view.name).cmp(&(
                !b.view.available,
                &b.view.host_name,
                &b.view.name,
            ))
        });
        scan.candidates = entries.iter().map(|e| e.view.clone()).collect();
        Ok((scan, entries))
    }
}
