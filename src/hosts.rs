use crate::{fs::exists, models::*};
use std::{
    env,
    path::{Path, PathBuf},
};
pub const HOST_IDS: [&str; 4] = ["opencode", "claude", "codex", "jcode"];
pub fn command_present(name: &str) -> bool {
    let Ok(paths) = env::var("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|p| {
        #[cfg(windows)]
        {
            ["", ".exe", ".cmd", ".bat"]
                .iter()
                .any(|ext| p.join(format!("{name}{ext}")).is_file())
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::metadata(p.join(name))
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
    })
}
fn pick(paths: Vec<PathBuf>) -> String {
    paths
        .iter()
        .find(|p| exists(p))
        .unwrap_or(&paths[0])
        .to_string_lossy()
        .into()
}
pub fn hosts(home: &Path, state: &State, environment: bool) -> Vec<Host> {
    let config = if environment {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
    } else {
        home.join(".config")
    };
    #[cfg(windows)]
    let config = if environment {
        env::var_os("APPDATA").map(PathBuf::from).unwrap_or(config)
    } else {
        config
    };
    let jcode = if environment {
        env::var_os("JCODE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".jcode"))
    } else {
        home.join(".jcode")
    };
    let defaults = [
        (
            "opencode",
            "OpenCode",
            "本地开发 Agent",
            vec![
                config.join("opencode/opencode.json"),
                config.join("opencode.json"),
            ],
            vec![
                config.join("opencode/skills"),
                home.join(".opencode/skills"),
            ],
            vec![
                config.join("opencode/agents"),
                home.join(".opencode/agents"),
            ],
        ),
        (
            "claude",
            "Claude Code",
            "Anthropic coding Agent",
            vec![home.join(".claude/settings.json")],
            vec![home.join(".claude/skills")],
            vec![home.join(".claude/agents")],
        ),
        (
            "codex",
            "Codex",
            "OpenAI coding Agent",
            vec![home.join(".codex/config.toml")],
            vec![home.join(".codex/skills")],
            vec![home.join(".codex/agents")],
        ),
        (
            "jcode",
            "jcode",
            "Rust coding Agent",
            vec![jcode.join("mcp.json")],
            vec![jcode.join("skills")],
            vec![jcode.join("agents")],
        ),
    ];
    defaults
        .into_iter()
        .map(|(id, name, description, config, skills, agents)| {
            let paths = state.paths.get(id).cloned().unwrap_or_else(|| HostPaths {
                config: pick(config),
                skills: pick(skills),
                agents: pick(agents),
            });
            let installed = exists(&paths.config) || (environment && command_present(id));
            Host {
                id: id.into(),
                name: name.into(),
                description: description.into(),
                installed,
                status: if installed { "ready" } else { "not-found" }.into(),
                format: if paths.config.ends_with(".toml") {
                    "toml"
                } else {
                    "json"
                }
                .into(),
                config_path: paths.config,
                skill_path: paths.skills,
                agent_path: paths.agents,
            }
        })
        .collect()
}
pub fn deployment_views(deps: &Map<Deployment>) -> (Map<bool>, Map<String>) {
    (
        HOST_IDS
            .iter()
            .map(|id| (id.to_string(), deps.get(*id).is_some_and(|d| d.enabled)))
            .collect(),
        deps.iter()
            .filter(|(_, d)| d.enabled)
            .map(|(h, d)| (h.clone(), d.mode.clone()))
            .collect(),
    )
}
pub fn project_dir(host: &str) -> Option<&'static str> {
    match host {
        "opencode" => Some(".opencode/skills"),
        "claude" => Some(".claude/skills"),
        "codex" => Some(".agents/skills"),
        "jcode" => Some(".jcode/skills"),
        _ => None,
    }
}
