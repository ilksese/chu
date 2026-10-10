//! 只读总览投影：唯一输入为 Snapshot，不探测文件系统，也不执行写入。
use crate::{models::Snapshot, ui};

pub struct Overview {
    pub counts: Vec<ui::CountRow>,
    pub issues: Vec<ui::IssueRow>,
}

pub fn project(snapshot: &Snapshot) -> Overview {
    let counts = [
        (
            "Skills",
            snapshot.skills.iter().filter(|s| s.managed).count(),
            2,
        ),
        (
            "MCP 服务",
            snapshot.mcps.iter().filter(|m| m.managed).count(),
            3,
        ),
        (
            "自定义 Agent",
            snapshot.agents.iter().filter(|a| a.managed).count(),
            5,
        ),
        ("项目", snapshot.projects.len(), 1),
    ]
    .into_iter()
    .map(|(label, value, page)| ui::CountRow {
        label: label.into(),
        value: value.min(i32::MAX as usize) as i32,
        page,
    })
    .collect();
    let mut issues = Vec::new();
    for project in &snapshot.projects {
        if !project.available {
            issues.push(issue(
                "项目路径不可用",
                &format!("{} · 请重新定位项目目录。", project.name),
                1,
                &project.id,
                "",
            ));
        }
        for (skill_id, hosts) in &project.deployments {
            let skill = snapshot.skills.iter().find(|s| s.id == *skill_id);
            let skill_name = skill.map(|s| s.name.as_str()).unwrap_or(skill_id);
            for (host_id, deployment) in hosts {
                let Some(status) = problem(&deployment.status) else {
                    continue;
                };
                let host_name = snapshot
                    .hosts
                    .iter()
                    .find(|h| h.id == *host_id)
                    .map(|h| h.name.as_str())
                    .unwrap_or(host_id);
                issues.push(issue(
                    status,
                    &format!("{} · {} · {}", project.name, skill_name, host_name),
                    1,
                    &project.id,
                    host_id,
                ));
            }
        }
    }
    for host in &snapshot.hosts {
        if !host.installed {
            issues.push(issue(
                "未检测到宿主",
                &format!("{} · 查看路径配置与安装状态。", host.name),
                8,
                "",
                &host.id,
            ));
        }
    }
    for skill in snapshot.skills.iter().filter(|s| s.managed) {
        for (host, mode) in &skill.mode_by_host {
            if let Some(status) = problem(mode) {
                issues.push(issue(
                    status,
                    &format!("Skill · {}", skill.name),
                    2,
                    &skill.id,
                    host,
                ));
            }
        }
    }
    for prompt in &snapshot.prompts {
        for (host, mode) in &prompt.mode_by_host {
            if let Some(status) = problem(mode) {
                issues.push(issue(
                    status,
                    &format!("提示词 · {}", prompt.name),
                    6,
                    &prompt.id,
                    host,
                ));
            }
        }
    }
    for provider in &snapshot.providers {
        if !provider.models_error.is_empty() {
            // 错误可能包含传输地址，不在总览显示其中潜在的凭证。
            issues.push(issue(
                "模型目录刷新失败",
                &format!("{} · 进入详情重新刷新模型目录。", provider.name),
                4,
                &provider.id,
                "",
            ));
        }
    }
    Overview { counts, issues }
}
fn issue(title: &str, detail: &str, page: i32, id: &str, host: &str) -> ui::IssueRow {
    ui::IssueRow {
        title: title.into(),
        detail: detail.into(),
        page,
        id: id.into(),
        host: host.into(),
    }
}
fn problem(status: &str) -> Option<&'static str> {
    match status {
        "modified" => Some("副本已修改 · 保留用户更改"),
        "missing" => Some("部署副本缺失"),
        "conflict" => Some("部署存在冲突 · 需要人工处理"),
        _ => None,
    }
}
