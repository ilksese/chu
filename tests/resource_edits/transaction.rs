use super::*;

#[cfg(unix)]
#[test]
fn shared_config_hashes_refresh_symlink_aliases_without_replacing_the_alias() {
    let mut f = Fixture::new();
    f.app.add_mcp(stdio("Edited")).unwrap();
    f.app.add_mcp(stdio("Alias sibling")).unwrap();
    for id in ["edited", "alias-sibling"] {
        f.app.toggle_mcp(id, "claude", true).unwrap();
    }
    let target = f.app.state.mcps[0].deployments["claude"].target.clone();
    let alias = f.temp.path().join("config-alias.json");
    symlink(Path::new(&target), &alias).unwrap();
    let mut deployment = f.app.state.mcps[1].deployments["claude"].clone();
    deployment.target = alias.to_string_lossy().into();
    f.app.state.mcps[1]
        .deployments
        .insert("jcode".into(), deployment);
    f.app.save().unwrap();
    f.app
        .update_mcp(
            "edited",
            McpPatch {
                name: Some("Renamed".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        f.app.state.mcps[1].deployments["jcode"].last_hash,
        hash_path(Path::new(&target))
    );
    assert_eq!(fs::read_link(&alias).unwrap(), PathBuf::from(&target));
    f.app
        .delete_mcp("edited", ResourceDeleteInput::default())
        .unwrap();
    assert_eq!(
        f.app.state.mcps[0].deployments["jcode"].last_hash,
        hash_path(Path::new(&target))
    );
    assert_eq!(fs::read_link(alias).unwrap(), PathBuf::from(target));
}

#[test]
fn cancelled_crud_does_not_rewrite_watched_files() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Cancel agent");
    f.app.add_mcp(stdio("Cancel mcp")).unwrap();
    for host in HOST_IDS {
        f.app.toggle_agent(&id, host, true).unwrap();
        f.app.toggle_mcp("cancel-mcp", host, true).unwrap();
    }
    let before = files(&f);
    let times: BTreeMap<_, _> = before
        .keys()
        .map(|path| {
            (
                path.clone(),
                fs::metadata(path).unwrap().modified().unwrap(),
            )
        })
        .collect();
    let operation = Operation::new();
    assert!(operation.cancel());
    with_operation(&operation, || {
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
        assert!(
            f.app
                .update_mcp(
                    "cancel-mcp",
                    McpPatch {
                        name: Some("Changed".into()),
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert!(
            f.app
                .delete_mcp("cancel-mcp", ResourceDeleteInput::default())
                .is_err()
        );
    });
    assert_eq!(operation.phase(), Phase::Preparing);
    assert_eq!(files(&f), before);
    for (path, time) in times {
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), time);
    }
}

#[cfg(unix)]
#[test]
fn replaced_symlink_targets_are_refused_without_following_external_files() {
    let mut f = Fixture::new();
    let id = add_agent(&mut f, "Linked");
    f.app.toggle_agent(&id, "claude", true).unwrap();
    let target = f.app.state.agents[0].deployments["claude"].target.clone();
    let external = f.temp.path().join("external.md");
    copy_file(Path::new(&target), &external).unwrap();
    remove(Path::new(&target)).unwrap();
    symlink(&external, Path::new(&target)).unwrap();
    let before = files(&f);
    assert!(
        f.app
            .delete_agent(&id, ResourceDeleteInput::default())
            .is_err()
    );
    assert_eq!(files(&f), before);
    assert_eq!(fs::read_link(&target).unwrap(), external);
}

#[cfg(unix)]
#[test]
fn failed_state_commit_restores_config_backups_agents_permissions_and_memory() {
    use std::os::unix::fs::PermissionsExt;
    for delete in [false, true] {
        for is_agent in [false, true] {
            let mut f = Fixture::new();
            let id = if is_agent {
                add_agent(&mut f, "Rollback")
            } else {
                f.app.add_mcp(stdio("Rollback")).unwrap();
                "rollback".into()
            };
            for host in HOST_IDS {
                if is_agent {
                    f.app.toggle_agent(&id, host, true).unwrap();
                } else {
                    f.app.toggle_mcp(&id, host, true).unwrap();
                }
            }
            if !is_agent {
                for deployment in f.app.state.mcps[0].deployments.values() {
                    atomic_write(
                        &PathBufExt::suffix(Path::new(&deployment.target), ".chu.bak"),
                        b"old backup bytes",
                    )
                    .unwrap();
                }
            }
            let before = files(&f);
            for path in before.keys() {
                fs::set_permissions(path, fs::Permissions::from_mode(0o640)).unwrap();
            }
            let state = f.app.state.clone();
            let root_permissions = fs::metadata(&f.app.root).unwrap().permissions();
            fs::set_permissions(&f.app.root, fs::Permissions::from_mode(0o500)).unwrap();
            let probe = f.app.root.join("write-probe");
            if fs::write(&probe, b"probe").is_ok() {
                fs::remove_file(probe).unwrap();
                fs::set_permissions(&f.app.root, root_permissions).unwrap();
                continue;
            }
            let result = match (is_agent, delete) {
                (true, true) => f.app.delete_agent(&id, ResourceDeleteInput::default()),
                (true, false) => f.app.update_agent(
                    &id,
                    AgentUpdateInput {
                        input: agent("Changed"),
                        version: None,
                    },
                ),
                (false, true) => f.app.delete_mcp(&id, ResourceDeleteInput::default()),
                (false, false) => f.app.update_mcp(
                    &id,
                    McpPatch {
                        name: Some("Changed".into()),
                        ..Default::default()
                    },
                ),
            };
            fs::set_permissions(&f.app.root, root_permissions).unwrap();
            assert!(result.is_err());
            assert_eq!(f.app.state, state);
            assert_eq!(files(&f), before);
            for path in before.keys() {
                assert_eq!(
                    fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o640
                );
            }
        }
    }
}
