//! Repository grammar, frontmatter and candidate discovery, independent of UI and HOME.
use crate::{fs::*, models::*};
use anyhow::{Result, bail};
use std::{collections::BTreeSet, fs, path::Path};
use url::Url;
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub kind: String,
    pub url: String,
    pub reference: String,
    pub subpath: String,
    pub filter: String,
}
pub fn parse_source(input: &str) -> Result<Source> {
    let input = input.trim();
    if input.is_empty() {
        bail!("请填写 skill 来源");
    }
    let (base, fragment) = input.split_once('#').unwrap_or((input, ""));
    let (reference, filter) = fragment.split_once('@').unwrap_or((fragment, ""));
    let reference = if reference.is_empty() {
        "main"
    } else {
        reference
    };
    let base = base.strip_prefix("github:").unwrap_or(base);
    let base = if let Some(s) = base.strip_prefix("gitlab:") {
        format!("https://gitlab.com/{s}")
    } else {
        base.to_owned()
    };
    let mut source = Source {
        kind: "github".into(),
        url: String::new(),
        reference: reference.into(),
        subpath: String::new(),
        filter: filter.into(),
    };
    if !base.contains(':') && !base.starts_with(['.', '/']) {
        let (base, selector) = base.split_once('@').unwrap_or((&base, ""));
        let parts: Vec<_> = base.trim_matches('/').split('/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            source.url = format!(
                "https://github.com/{}/{}",
                parts[0],
                parts[1].trim_end_matches(".git")
            );
            source.subpath = clean_subpath(&parts[2..].join("/"));
            if source.filter.is_empty() {
                source.filter = selector.into();
            }
            return Ok(source);
        }
    }
    if let Ok(u) = Url::parse(&base)
        && u.scheme() == "https"
        && u.host_str().is_some()
        && u.username().is_empty()
        && u.password().is_none()
    {
        let parts: Vec<_> = u.path().trim_matches('/').split('/').collect();
        if u.host_str() == Some("github.com") && parts.len() >= 2 {
            source.url = format!(
                "https://github.com/{}/{}",
                parts[0],
                parts[1].trim_end_matches(".git")
            );
            if parts.len() >= 4 && parts[2] == "tree" {
                source.reference = parts[3].into();
                source.subpath = clean_subpath(&parts[4..].join("/"));
            }
            return Ok(source);
        }
        if u.host_str() == Some("gitlab.com") && parts.len() >= 2 {
            source.kind = "gitlab".into();
            let mut end = parts.len();
            if let Some(i) = parts.iter().position(|p| *p == "-")
                && i > 1
                && parts.get(i + 1) == Some(&"tree")
                && parts.len() > i + 2
            {
                end = i;
                source.reference = parts[i + 2].into();
                source.subpath = clean_subpath(&parts[i + 3..].join("/"));
            }
            source.url = format!(
                "https://gitlab.com/{}",
                parts[..end].join("/").trim_end_matches(".git")
            );
            return Ok(source);
        }
        if u.path().ends_with(".git") {
            source.kind = "git".into();
            source.url = base.trim_end_matches(".git").into();
            return Ok(source);
        }
    }
    bail!("skill 来源必须是公开 HTTPS Git 仓库或 owner/repo")
}
fn clean_subpath(s: &str) -> String {
    if s.split('/').any(|p| p == "..") {
        String::new()
    } else {
        s.trim_matches('/').trim_start_matches("./").to_owned()
    }
}
pub fn repository(item: &SkillLock) -> String {
    if item.r#ref.is_empty() || item.r#ref == "main" {
        item.source.clone()
    } else {
        format!("{}#{}", item.source, item.r#ref)
    }
}
pub fn frontmatter(path: &Path) -> Option<(String, String)> {
    let text = fs::read_to_string(path.join("SKILL.md"))
        .ok()?
        .replace("\r\n", "\n");
    let text = text.strip_prefix("---\n")?;
    let (header, _) = text.split_once("\n---")?;
    let lines: Vec<_> = header.lines().collect();
    let (mut name, mut description) = (String::new(), String::new());
    let mut i = 0;
    while i < lines.len() {
        if let Some((key, value)) = lines[i].trim().split_once(':') {
            let mut value = value.trim().trim_matches(['\'', '"']).to_owned();
            if ["|", ">", "|-", ">-"].contains(&value.as_str()) {
                let sep = if value.starts_with('>') { " " } else { "\n" };
                let mut block = Vec::new();
                while i + 1 < lines.len() && lines[i + 1].starts_with([' ', '\t']) {
                    i += 1;
                    block.push(lines[i].trim());
                }
                value = block.join(sep);
            }
            match key.trim().to_lowercase().as_str() {
                "name" => name = value,
                "description" => description = value,
                _ => {}
            }
        }
        i += 1;
    }
    if name.is_empty() || description.is_empty() {
        None
    } else {
        Some((name, description))
    }
}
pub fn description(path: &Path) -> String {
    if let Some((_, d)) = frontmatter(path) {
        return d;
    }
    fs::read_to_string(path.join("SKILL.md"))
        .unwrap_or_default()
        .lines()
        .find(|l| l.trim().starts_with('#'))
        .map(|l| l.trim().trim_start_matches('#').trim().to_owned())
        .unwrap_or_default()
}
fn internal(path: &Path) -> bool {
    fs::read_to_string(path.join("SKILL.md"))
        .unwrap_or_default()
        .contains("internal: true")
}
fn scan(
    root: &Path,
    dir: &Path,
    depth: usize,
    seen: &mut BTreeSet<String>,
    found: &mut Vec<SkillCandidate>,
) {
    if !dir.is_dir() {
        return;
    }
    if !internal(dir)
        && let Some((name, description)) = frontmatter(dir)
    {
        if seen.insert(name.clone()) {
            found.push(SkillCandidate {
                name,
                description,
                path: dir
                    .strip_prefix(root)
                    .map(|p| {
                        let s = p.to_string_lossy().replace('\\', "/");
                        if s.is_empty() { ".".into() } else { s }
                    })
                    .unwrap_or_default(),
                installed: false,
            });
        }
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        if e.file_type().is_ok_and(|t| t.is_dir())
            && name != ".git"
            && name != "node_modules"
            && !name.starts_with(".env")
        {
            scan(root, &e.path(), depth - 1, seen, found);
        }
    }
}
fn plugin_dirs(value: &serde_json::Value, root: &Path, result: &mut Vec<std::path::PathBuf>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(skills) = map.get("skills").and_then(|v| v.as_array()) {
                for s in skills
                    .iter()
                    .filter_map(|s| s.as_str())
                    .filter(|s| s.starts_with("./"))
                {
                    if let Ok(path) = safe_relative(root, s)
                        && let Some(p) = path.parent()
                    {
                        result.push(p.to_owned());
                    }
                }
            }
            for v in map.values() {
                plugin_dirs(v, root, result);
            }
        }
        serde_json::Value::Array(list) => {
            for v in list {
                plugin_dirs(v, root, result);
            }
        }
        _ => {}
    }
}
pub fn discover(root: &Path) -> Vec<SkillCandidate> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    if !internal(root)
        && let Some((name, description)) = frontmatter(root)
    {
        return vec![SkillCandidate {
            name,
            description,
            path: ".".into(),
            installed: false,
        }];
    }
    scan(root, root, 2, &mut seen, &mut found);
    for dir in [
        "skills",
        "skills/.curated",
        "skills/.experimental",
        "skills/.system",
    ] {
        scan(root, &root.join(dir), 4, &mut seen, &mut found);
    }
    for host in [
        "agent",
        "agents",
        "claude",
        "cline",
        "codebuddy",
        "codex",
        "commandcode",
        "continue",
        "cursor",
        "factory",
        "github",
        "goose",
        "grok",
        "hermes",
        "iflow",
        "junie",
        "kilo",
        "kilocode",
        "kimchi",
        "kiro",
        "minimax",
        "mux",
        "neovate",
        "opencode",
        "openhands",
        "pi",
        "qoder",
        "roo",
        "trae",
        "windsurf",
        "zcode",
        "zencoder",
    ] {
        scan(
            root,
            &root.join(format!(".{host}/skills")),
            4,
            &mut seen,
            &mut found,
        );
    }
    for manifest in [
        ".claude-plugin/marketplace.json",
        ".claude-plugin/plugin.json",
    ] {
        if let Ok(data) = fs::read(root.join(manifest))
            && let Ok(value) = serde_json::from_slice(&data)
        {
            let mut dirs = Vec::new();
            plugin_dirs(&value, root, &mut dirs);
            for d in dirs {
                scan(root, &d, 2, &mut seen, &mut found);
            }
        }
    }
    if found.is_empty() {
        scan(root, root, 6, &mut seen, &mut found);
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}
pub fn resolve(
    item: &SkillLock,
    name: &str,
    installed: &Path,
    found: &[SkillCandidate],
) -> Option<String> {
    if let Some(c) = found.iter().find(|c| c.path == item.skill_path) {
        return Some(c.path.clone());
    }
    let front = frontmatter(installed).map(|(n, _)| n).unwrap_or_default();
    for n in [front.as_str(), name] {
        let matches: Vec<_> = found.iter().filter(|c| c.name == n).collect();
        if matches.len() == 1 {
            return Some(matches[0].path.clone());
        }
    }
    let normalized = slug(name);
    let matches: Vec<_> = found
        .iter()
        .filter(|c| {
            slug(&c.name) == normalized
                || slug(&c.path) == normalized
                || slug(
                    Path::new(&c.path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .as_ref(),
                ) == normalized
        })
        .collect();
    if matches.len() == 1 {
        Some(matches[0].path.clone())
    } else {
        None
    }
}
