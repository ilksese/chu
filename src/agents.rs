use crate::{App, fs::*, models::*};
use anyhow::{Context, Result, bail};
use std::path::Path;
pub fn content(input: &AgentInput) -> String {
    format!(
        "---\nname: {}\ndescription: {}{}\n---\n\n{}\n",
        input.name,
        input.description.replace('\n', " "),
        if input.model.is_empty() {
            String::new()
        } else {
            format!("\nmodel: {}", input.model)
        },
        input.prompt
    )
}
impl App {
    pub fn add_agent(&mut self, mut input: AgentInput) -> Result<()> {
        input.name = display_name(&input.name)?;
        if input.prompt.trim().is_empty() {
            bail!("Agent 名称和提示词不能为空");
        }
        let id = identifier(&input.name, "agent");
        if self
            .state
            .agents
            .iter()
            .any(|a| a.name == input.name || a.id == id)
        {
            bail!("Agent 已存在");
        }
        atomic_write(
            &self.root.join("agents").join(format!("{id}.md")),
            content(&input).as_bytes(),
        )?;
        self.state.agents.push(StoredAgent {
            id,
            name: input.name,
            description: input.description,
            prompt: input.prompt,
            model: input.model,
            source: "chu".into(),
            deployments: Map::new(),
        });
        self.save()
    }
    pub fn toggle_agent(&mut self, id: &str, host: &str, enabled: bool) -> Result<()> {
        let host = self.host(host)?;
        let index = self
            .state
            .agents
            .iter()
            .position(|a| a.id == id)
            .context("未知的自定义 Agent")?;
        let agent = self.state.agents[index].clone();
        let target = Path::new(&host.agent_path).join(format!("{id}.md"));
        let dep = agent.deployments.get(&host.id).filter(|d| d.enabled);
        if enabled {
            if dep.is_some() {
                return Ok(());
            }
            if exists(&target) {
                bail!("目标 Agent 已存在，请先处理冲突");
            }
            atomic_write(
                &target,
                content(&AgentInput {
                    name: agent.name,
                    description: agent.description,
                    prompt: agent.prompt,
                    model: agent.model,
                })
                .as_bytes(),
            )?;
            self.state.agents[index].deployments.insert(
                host.id,
                Deployment {
                    enabled: true,
                    mode: "copy".into(),
                    target: target.to_string_lossy().into(),
                    last_hash: hash_path(&target),
                },
            );
        } else {
            let Some(dep) = dep else {
                return Ok(());
            };
            if hash_path(Path::new(&dep.target)) != dep.last_hash {
                bail!("目标 Agent 已被修改，已保护用户改动");
            }
            remove(Path::new(&dep.target))?;
            self.state.agents[index].deployments.remove(&host.id);
        }
        self.save()
    }
}
