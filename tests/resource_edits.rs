mod common;
use chu::{
    agents::content,
    fs::*,
    hosts::HOST_IDS,
    mcp::{PathBufExt, read_config, root_key, write_config},
    models::*,
    operation::{Operation, Phase, with_operation},
    resource_edit::*,
};
use common::*;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn agent(name: &str) -> AgentInput {
    AgentInput {
        name: name.into(),
        description: "Fixture".into(),
        prompt: "Original prompt".into(),
        model: "fixture-model".into(),
    }
}
fn add_agent(f: &mut Fixture, name: &str) -> String {
    f.app.add_agent(agent(name)).unwrap();
    f.app.state.agents.last().unwrap().id.clone()
}
fn config(f: &Fixture, host: &str) -> Value {
    let host = f.app.host(host).unwrap();
    read_config(Path::new(&host.config_path), &host.format).unwrap()
}
fn set_config(f: &Fixture, host: &str, value: &Value) {
    let host = f.app.host(host).unwrap();
    write_config(Path::new(&host.config_path), &host.format, value).unwrap();
}
fn import_mcp(f: &mut Fixture, name: &str, value: Value, hosts: &[&str]) {
    let mut root = config(f, "claude");
    root["mcpServers"][name] = value;
    set_config(f, "claude", &root);
    let selected = f
        .app
        .list_import_candidates("mcps")
        .unwrap()
        .candidates
        .into_iter()
        .filter(|candidate| candidate.name == name)
        .map(|candidate| candidate.id)
        .collect();
    f.app
        .import_host_resources(
            "mcps",
            selected,
            hosts.iter().map(|host| (*host).into()).collect(),
        )
        .unwrap();
}
fn files(f: &Fixture) -> BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(f.temp.path())
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| (entry.path().to_path_buf(), fs::read(entry.path()).unwrap()))
        .collect()
}
fn no_mcp_write(f: &mut Fixture, id: &str) {
    let before = files(f);
    let state = f.app.state.clone();
    assert!(
        f.app
            .update_mcp(
                id,
                McpPatch {
                    description: Some("Changed".into()),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .delete_mcp(id, ResourceDeleteInput::default())
            .is_err()
    );
    assert_eq!(files(f), before);
    assert_eq!(f.app.state, state);
}
fn no_agent_write(f: &mut Fixture, id: &str) {
    let before = files(f);
    let state = f.app.state.clone();
    assert!(f.app.read_agent_edit(id).is_err());
    assert!(
        f.app
            .update_agent(
                id,
                AgentUpdateInput {
                    input: agent("Changed"),
                    version: None
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .delete_agent(id, ResourceDeleteInput::default())
            .is_err()
    );
    assert_eq!(files(f), before);
    assert_eq!(f.app.state, state);
}

#[path = "resource_edits/agent.rs"]
mod agent;
#[path = "resource_edits/mcp.rs"]
mod mcp;
#[path = "resource_edits/mcp_deployments.rs"]
mod mcp_deployments;
#[path = "resource_edits/patch.rs"]
mod patch;
#[path = "resource_edits/transaction.rs"]
mod transaction;
