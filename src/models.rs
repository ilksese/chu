use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

pub type Map<T> = BTreeMap<String, T>;
// Existing state may encode uninitialized slices/maps as null. Preserve upgrade compatibility.
fn nonnull<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
macro_rules! model {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
        #[serde(default, rename_all = "camelCase")]
        pub struct $name { $(#[serde(deserialize_with = "nonnull")] pub $field: $ty),* }
    }
}
model!(Deployment {
    enabled: bool,
    mode: String,
    target: String,
    last_hash: String
});
model!(HostPaths {
    config: String,
    skills: String,
    agents: String
});
model!(StoredSkill { id: String, name: String, description: String, source_dir: String, deployments: Map<Deployment> });
model!(StoredMcp { id: String, name: String, description: String, r#type: String, endpoint: String, command: String, args: Vec<String>, env: Map<String>, headers: Map<String>, secret: String, deployments: Map<Deployment>, native_formats: bool, extra: Map<serde_json::Value> });
model!(StoredAgent { id: String, name: String, description: String, prompt: String, model: String, source: String, deployments: Map<Deployment> });
model!(StoredProject { id: String, name: String, path: String, created_at: String, deployments: Map<Map<Deployment>> });
model!(Provider { id: String, name: String, env_api_key: String, base_url: String, models: Vec<String>, models_fetched_at: String, models_error: String });
model!(State { version: i32, paths: Map<HostPaths>, skills: Vec<StoredSkill>, mcps: Vec<StoredMcp>, agents: Vec<StoredAgent>, projects: Vec<StoredProject>, providers: Vec<Provider> });
model!(SkillLock {
    source: String,
    source_type: String,
    r#ref: String,
    skill_path: String,
    content_hash: String,
    installed_at: String,
    updated_at: String
});
model!(PromptDeployment {
    mode: String,
    target: String,
    last_hash: String,
    backup: bool
});
model!(PromptLock { name: String, file: String, content_hash: String, updated_at: String, deployments: Map<PromptDeployment> });
model!(LockFile { version: i32, skills: Map<SkillLock>, prompts: Map<PromptLock> });
model!(Host {
    id: String,
    name: String,
    description: String,
    installed: bool,
    status: String,
    config_path: String,
    skill_path: String,
    agent_path: String,
    format: String
});
model!(SkillView { id: String, name: String, description: String, tracked: bool, repository: String, source: String, managed: bool, enabled_on: Map<bool>, mode_by_host: Map<String> });
model!(McpView { id: String, name: String, description: String, r#type: String, endpoint: String, command: String, has_credentials: bool, managed: bool, enabled_on: Map<bool> });
model!(AgentView { id: String, name: String, description: String, model: String, source: String, managed: bool, enabled_on: Map<bool> });
model!(PromptView { id: String, name: String, source: String, preview: String, enabled_on: Map<bool>, mode_by_host: Map<String> });
model!(ReferenceView {
    id: String,
    name: String,
    source: String,
    preview: String
});
model!(ProjectDeploymentView {
    enabled: bool,
    status: String
});
model!(ProjectView { id: String, name: String, path: String, available: bool, created_at: String, deployments: Map<Map<ProjectDeploymentView>> });
model!(Snapshot { root: String, hosts: Vec<Host>, projects: Vec<ProjectView>, skills: Vec<SkillView>, mcps: Vec<McpView>, agents: Vec<AgentView>, prompts: Vec<PromptView>, references: Vec<ReferenceView>, providers: Vec<Provider>, last_scan: String });
model!(SkillCandidate {
    name: String,
    description: String,
    path: String,
    installed: bool
});
model!(SkillUpdate {
    id: String,
    name: String,
    status: String
});
model!(McpInput { name: String, description: String, r#type: String, endpoint: String, command: String, args: Vec<String>, env: Map<String>, headers: Map<String>, secret: String });
model!(AgentInput {
    name: String,
    description: String,
    prompt: String,
    model: String
});
model!(ProviderInput {
    name: String,
    api_key: String,
    env_api_key: String,
    base_url: String
});
model!(Preferences { hidden_hosts: Vec<String> });

model!(ImportCandidate {
    id: String,
    host_id: String,
    host_name: String,
    name: String,
    description: String,
    source: String,
    r#type: String,
    has_credentials: bool,
    available: bool,
    reason: String
});
model!(ImportScan { candidates: Vec<ImportCandidate>, warnings: Vec<String> });
