use super::*;

#[test]
fn agent_shared_copy_and_recorded_targets_keep_stable_id_paths() {
    let mut f = Fixture::new();
    let host = f.app.host("claude").unwrap();
    f.app
        .update_host_paths(
            "jcode",
            &f.app.host("jcode").unwrap().config_path,
            &host.skill_path,
            &host.agent_path,
        )
        .unwrap();
    let id = add_agent(&mut f, "Shared agent");
    f.app.toggle_agent(&id, "claude", true).unwrap();
    let deployment = f.app.state.agents[0].deployments["claude"].clone();
    let old_target = deployment.target.clone();
    f.app.state.agents[0]
        .deployments
        .insert("jcode".into(), deployment);
    f.app.save().unwrap();
    let new_agents = f.temp.path().join("new/agents");
    f.app
        .update_host_paths(
            "claude",
            &host.config_path,
            &host.skill_path,
            new_agents.to_str().unwrap(),
        )
        .unwrap();
    f.app
        .update_agent(
            &id,
            AgentUpdateInput {
                input: agent("Renamed shared"),
                version: None,
            },
        )
        .unwrap();
    assert!(!exists(new_agents.join(format!("{id}.md"))));
    for deployment in f.app.state.agents[0].deployments.values() {
        assert_eq!(deployment.target, old_target);
        assert_eq!(deployment.last_hash, hash_path(Path::new(&old_target)));
    }
    f.app
        .delete_agent(&id, ResourceDeleteInput::default())
        .unwrap();
    assert!(!exists(&old_target));
}

#[test]
fn central_agent_mismatch_reports_only_the_file_path_not_prompt_content() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Mismatch");
    let path = f.app.root.join("agents/mismatch.md");
    atomic_write(&path, b"synthetic-private-prompt").unwrap();
    let error = f.app.read_agent_edit(&id).err().unwrap().to_string();
    assert!(error.contains(path.to_str().unwrap()));
    assert!(!error.contains("synthetic-private-prompt"));
}

#[test]
fn agent_rename_keeps_central_and_host_id_filenames_then_deletes_all() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Original");
    for host in HOST_IDS {
        f.app.toggle_agent(&id, host, true).unwrap();
    }
    let view = f.app.read_agent_edit(&id).unwrap();
    assert_eq!(view.input, agent("Original"));
    let updated = agent("Renamed");
    f.app
        .update_agent(
            &id,
            AgentUpdateInput {
                input: updated.clone(),
                version: Some(view.version),
            },
        )
        .unwrap();
    assert_eq!(f.app.state.agents[0].id, id);
    assert_eq!(
        fs::read(f.app.root.join("agents/original.md")).unwrap(),
        content(&updated).as_bytes()
    );
    assert!(!exists(f.app.root.join("agents/renamed.md")));
    for host in HOST_IDS {
        let deployment = &f.app.state.agents[0].deployments[host];
        assert_eq!(
            Path::new(&deployment.target).file_name().unwrap(),
            "original.md"
        );
        assert_eq!(
            fs::read(&deployment.target).unwrap(),
            content(&updated).as_bytes()
        );
        assert_eq!(
            deployment.last_hash,
            hash_path(Path::new(&deployment.target))
        );
    }
    let targets: Vec<_> = f.app.state.agents[0]
        .deployments
        .values()
        .map(|deployment| deployment.target.clone())
        .collect();
    f.app
        .delete_agent(
            &id,
            ResourceDeleteInput {
                version: Some(f.app.read_agent_edit(&id).unwrap().version),
            },
        )
        .unwrap();
    assert!(f.app.state.agents.is_empty());
    assert!(!exists(f.app.root.join("agents/original.md")));
    assert!(targets.iter().all(|target| !exists(target)));
}

#[test]
fn central_agent_drift_is_protected_even_when_no_host_is_enabled() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Guarded");
    atomic_write(
        &f.app.root.join("agents/guarded.md"),
        b"externally edited central",
    )
    .unwrap();
    no_agent_write(&mut f, &id);
}

#[test]
fn agent_missing_or_modified_target_is_not_implicitly_detached() {
    for missing in [false, true] {
        let mut f = Fixture::new();
        let id = add_agent(&mut f, "Guarded");
        for host in HOST_IDS {
            f.app.toggle_agent(&id, host, true).unwrap();
        }
        let target = f.app.state.agents[0].deployments["jcode"].target.clone();
        if missing {
            remove(Path::new(&target)).unwrap();
        } else {
            atomic_write(Path::new(&target), b"outside change").unwrap();
        }
        let before = files(&f);
        let state = f.app.state.clone();
        assert!(
            f.app
                .update_agent(
                    &id,
                    AgentUpdateInput {
                        input: agent("Changed"),
                        version: None
                    }
                )
                .is_err()
        );
        assert!(
            f.app
                .delete_agent(&id, ResourceDeleteInput::default())
                .is_err()
        );
        assert_eq!(files(&f), before);
        assert_eq!(f.app.state, state);
    }
}

#[test]
fn agent_versions_and_name_conflicts_are_protected() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Versioned");
    add_agent(&mut f, "Other");
    let view = f.app.read_agent_edit(&id).unwrap();
    f.app
        .update_agent(
            &id,
            AgentUpdateInput {
                input: agent("Changed"),
                version: Some(view.version.clone()),
            },
        )
        .unwrap();
    let before = files(&f);
    assert!(
        f.app
            .update_agent(
                &id,
                AgentUpdateInput {
                    input: agent("Again"),
                    version: Some(view.version.clone())
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .delete_agent(
                &id,
                ResourceDeleteInput {
                    version: Some(view.version)
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .update_agent(
                &id,
                AgentUpdateInput {
                    input: agent("Other"),
                    version: None
                }
            )
            .is_err()
    );
    assert_eq!(files(&f), before);
}
