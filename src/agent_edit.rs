//! Agent 文件与全部启用副本作为一个运行时事务编辑，稳定 ID 不随名称变更。
use crate::{
    App,
    agents::content,
    fs::{atomic_write, display_name, hash_path, remove, safe_name},
    import_transaction::Journal,
    models::*,
    resource_edit::{self, AgentEditView, AgentUpdateInput, ResourceDeleteInput},
};
use anyhow::{Result, anyhow, bail};
use std::{collections::BTreeMap, fs, path::PathBuf};

struct AgentPlan {
    path: PathBuf,
    hash: String,
}

fn input(item: &StoredAgent) -> AgentInput {
    AgentInput {
        name: item.name.clone(),
        description: item.description.clone(),
        prompt: item.prompt.clone(),
        model: item.model.clone(),
    }
}

fn validate_input(mut input: AgentInput) -> Result<AgentInput> {
    input.name = display_name(&input.name)?;
    if input.prompt.trim().is_empty() {
        bail!("Agent 名称和提示词不能为空");
    }
    Ok(input)
}

impl App {
    fn agent_central(&self, item: &StoredAgent) -> Result<AgentPlan> {
        if safe_name(&item.id).is_err() {
            bail!("Agent 标识无效，已阻止操作");
        }
        let path = self.root.join("agents").join(format!("{}.md", item.id));
        if !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
            bail!("中央 Agent 缺失或不是普通文件，已阻止操作");
        }
        let bytes = fs::read(&path).map_err(|_| anyhow!("无法读取中央 Agent，已阻止操作"))?;
        if bytes != content(&input(item)).as_bytes() {
            bail!(
                "中央 Agent 文件与状态不同，已保护用户改动：{}",
                path.display()
            );
        }
        Ok(AgentPlan {
            path,
            hash: resource_edit::content_hash(&bytes),
        })
    }

    pub fn read_agent_edit(&self, id: &str) -> Result<AgentEditView> {
        self.edit_state_hash()?;
        let item = self
            .state
            .agents
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| anyhow!("未知的自定义 Agent"))?;
        self.agent_central(item)?;
        Ok(AgentEditView {
            id: item.id.clone(),
            version: resource_edit::version(item)?,
            input: input(item),
            enabled_hosts: resource_edit::enabled_hosts(&item.deployments),
        })
    }

    pub fn update_agent(&mut self, id: &str, update: AgentUpdateInput) -> Result<()> {
        let index = self
            .state
            .agents
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| anyhow!("未知的自定义 Agent"))?;
        resource_edit::check_version(&self.state.agents[index], update.version.as_deref())?;
        let updated = validate_input(update.input)?;
        if self
            .state
            .agents
            .iter()
            .any(|item| item.id != id && item.name == updated.name)
        {
            bail!("Agent 名称已存在，已阻止操作");
        }
        let mut item = self.state.agents[index].clone();
        item.name = updated.name;
        item.description = updated.description;
        item.prompt = updated.prompt;
        item.model = updated.model;
        self.edit_agent(index, Some(item))
    }

    pub fn delete_agent(&mut self, id: &str, delete: ResourceDeleteInput) -> Result<()> {
        let index = self
            .state
            .agents
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| anyhow!("未知的自定义 Agent"))?;
        resource_edit::check_version(&self.state.agents[index], delete.version.as_deref())?;
        self.edit_agent(index, None)
    }

    fn agent_edit_plans(&self, item: &StoredAgent) -> Result<BTreeMap<PathBuf, AgentPlan>> {
        let central = self.agent_central(item)?;
        let central_path = resource_edit::file_target(central.path.to_string_lossy().as_ref())?;
        let expected = content(&input(item));
        let mut plans = BTreeMap::new();
        plans.insert(
            central_path.clone(),
            AgentPlan {
                path: central_path,
                hash: central.hash,
            },
        );
        for (host, deployment) in item
            .deployments
            .iter()
            .filter(|(_, deployment)| deployment.enabled)
        {
            self.host(host)?;
            if deployment.mode != "copy" {
                bail!("Agent 部署模式不兼容，已阻止操作");
            }
            let path = resource_edit::file_target(&deployment.target)?;
            let hash = hash_path(&path);
            if deployment.last_hash.is_empty() || deployment.last_hash != hash {
                bail!("受管 Agent 副本已被修改，已保护用户改动");
            }
            if fs::read(&path).map_err(|_| anyhow!("无法读取受管 Agent 副本，已阻止操作"))?
                != expected.as_bytes()
            {
                bail!("受管 Agent 副本与中央内容不同，已保护用户改动");
            }
            for other in self.state.agents.iter().filter(|other| other.id != item.id) {
                for deployment in other
                    .deployments
                    .values()
                    .filter(|deployment| deployment.enabled)
                {
                    if resource_edit::file_target(&deployment.target)
                        .is_ok_and(|target| target == path)
                    {
                        bail!("Agent 部署目标被其他资源使用，已阻止操作");
                    }
                }
            }
            plans
                .entry(path.clone())
                .or_insert(AgentPlan { path, hash });
        }
        Ok(plans)
    }

    fn edit_agent(&mut self, index: usize, updated: Option<StoredAgent>) -> Result<()> {
        let state_hash = self.edit_state_hash()?;
        let plans = self.agent_edit_plans(&self.state.agents[index])?;
        let bytes = updated
            .as_ref()
            .map(|item| content(&input(item)).into_bytes());
        let mut state = self.state.clone();
        if let Some(item) = updated {
            state.agents[index] = item;
        } else {
            state.agents.remove(index);
        }
        let mut journal = Journal::new();
        let prepared = (|| -> Result<()> {
            resource_edit::watch(&mut journal, &self.root.join("state.json"))?;
            for plan in plans.values() {
                resource_edit::watch(&mut journal, &plan.path)?;
            }
            self.check_edit_state(&state_hash)?;
            for plan in plans.values() {
                if hash_path(&plan.path) != plan.hash {
                    bail!("Agent 文件在确认后发生变化，已阻止操作");
                }
            }
            crate::operation::begin_commit()
        })();
        if let Err(error) = prepared {
            // 预检和取消阶段尚未写入；解除空日志，避免 Drop 回滚重写原文件。
            journal.commit();
            return Err(error);
        }
        self.resource_transaction(journal, move |app| {
            for plan in plans.values() {
                if let Some(bytes) = &bytes {
                    atomic_write(&plan.path, bytes)
                        .map_err(|_| anyhow!("写入 Agent 文件失败，正在恢复原有文件"))?;
                } else {
                    remove(&plan.path)
                        .map_err(|_| anyhow!("删除 Agent 文件失败，正在恢复原有文件"))?;
                }
            }
            for deployment in state
                .agents
                .iter_mut()
                .flat_map(|item| item.deployments.values_mut())
                .filter(|deployment| deployment.enabled)
            {
                if let Ok(path) = resource_edit::file_target(&deployment.target)
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
