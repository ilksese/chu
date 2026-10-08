mod common;
use chu::{App, fs::*, mcp::*, models::*, sources::*};
use common::*;
use serde_json::json;
use std::{fs, path::Path};
#[test]
fn legacy_null_collections_are_accepted() {
    let f = Fixture::new();
    atomic_write(&f.app.root.join("state.json"),br#"{"version":1,"skills":null,"mcps":null,"agents":null,"projects":null,"providers":null,"paths":null}"#).unwrap();
    let mut app = App::open(&f.app.root, &f.app.home).unwrap();
    assert!(app.snapshot().unwrap().skills.is_empty());
    app.save().unwrap();
    assert!(App::open(&app.root, &app.home).is_ok());
}
#[test]
fn corrupted_state_is_not_overwritten() {
    let f = Fixture::new();
    let path = f.app.root.join("state.json");
    atomic_write(&path, b"broken state").unwrap();
    assert!(App::open(&f.app.root, &f.app.home).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"broken state");
}
#[test]
fn corrupted_lock_is_not_overwritten() {
    let mut f = Fixture::new();
    let path = f.app.root.join("chu-lock.json");
    atomic_write(&path, b"broken lock").unwrap();
    assert!(f.app.snapshot().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"broken lock");
}
#[test]
fn slug_matches_legacy_and_non_latin_ids_are_distinct() {
    assert_eq!(slug("Code Review / v2"), "code-review-v2");
    assert_ne!(
        identifier("代码审查", "skill"),
        identifier("测试驱动", "skill")
    );
}
#[test]
fn atomic_writes_use_private_permissions() {
    let f = Fixture::new();
    let path = f.temp.path().join("nested/file");
    atomic_write(&path, b"one").unwrap();
    atomic_write(&path, b"two").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"two");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
#[test]
fn persisted_directory_hashes_preserve_traversal_order() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let root = f.temp.path().join("hash");
    atomic_write(&root.join("a/b"), b"child").unwrap();
    atomic_write(&root.join("z"), b"last").unwrap();
    let mut h = Sha256::new();
    h.update(b"a");
    h.update(Path::new("a").join("b").to_string_lossy().as_bytes());
    h.update(b"childzlast");
    assert_eq!(hash_path(&root), format!("{:x}", h.finalize()));
    assert_eq!(
        md5_file(&root.join("z")),
        "98bd1c45684cf587ac2347a92dd7bb51"
    );
}
#[test]
fn json_and_toml_preserve_unrelated_configuration() {
    let f = Fixture::new();
    for (ext, format) in [("json", "json"), ("toml", "toml")] {
        let path = f.temp.path().join(format!("config.{ext}"));
        let value = json!({"model":"keep","nested":{"v":42},"mcp_servers":{"fixture":{"command":"echo","args":["ok"]}}});
        write_config(&path, format, &value).unwrap();
        assert_eq!(read_config(&path, format).unwrap(), value);
        write_config(&path, format, &value).unwrap();
        assert!(exists(PathBufExt::suffix(&path, ".chu.bak")));
    }
}
#[test]
fn invalid_names_and_paths_are_rejected() {
    let f = Fixture::new();
    for n in ["", "..", ".", "../a", "a/b", "a\\b"] {
        assert!(safe_name(n).is_err());
    }
    assert_eq!(safe_name(" hello.md ").unwrap(), "hello.md");
    assert_eq!(markdown_name(" hello.md ").unwrap(), "hello");
    for p in ["../outside", "/outside", "a/../../outside"] {
        assert!(safe_relative(f.temp.path(), p).is_err());
    }
}
#[cfg(unix)]
#[test]
fn copies_preserve_links_and_remove_never_follows_them() {
    let f = Fixture::new();
    let source = f.temp.path().join("source");
    let external = f.temp.path().join("external");
    atomic_write(&external, b"preserve").unwrap();
    mkdir(&source).unwrap();
    symlink(&external, &source.join("link")).unwrap();
    let target = f.temp.path().join("copy");
    copy_dir(&source, &target).unwrap();
    assert!(
        fs::symlink_metadata(target.join("link"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    remove(&target).unwrap();
    assert_eq!(fs::read(external).unwrap(), b"preserve");
}
#[test]
fn skill_source_grammar_and_security() {
    let cases = [
        ("org/repo", "https://github.com/org/repo", "main", "", ""),
        (
            "github:org/repo#dev@tdd",
            "https://github.com/org/repo",
            "dev",
            "",
            "tdd",
        ),
        (
            "org/repo@tdd",
            "https://github.com/org/repo",
            "main",
            "",
            "tdd",
        ),
        (
            "org/repo/skills/design",
            "https://github.com/org/repo",
            "main",
            "skills/design",
            "",
        ),
        (
            "https://github.com/org/repo/tree/v1/skills/a",
            "https://github.com/org/repo",
            "v1",
            "skills/a",
            "",
        ),
        (
            "gitlab:group/repo#release",
            "https://gitlab.com/group/repo",
            "release",
            "",
            "",
        ),
        (
            "https://gitlab.com/group/sub/repo/-/tree/dev/skills",
            "https://gitlab.com/group/sub/repo",
            "dev",
            "skills",
            "",
        ),
        (
            "https://example.invalid/repo.git#dev",
            "https://example.invalid/repo",
            "dev",
            "",
            "",
        ),
    ];
    for (input, url, reference, subpath, filter) in cases {
        let s = parse_source(input).unwrap();
        assert_eq!(
            (&*s.url, &*s.reference, &*s.subpath, &*s.filter),
            (url, reference, subpath, filter)
        );
    }
    for input in [
        "",
        "/tmp/repo",
        "../repo",
        "file:///tmp/a",
        "http://example.com/r.git",
        "git@example.com:org/repo",
        "https://user:pass@example.com/a.git",
    ] {
        assert!(parse_source(input).is_err());
    }
}
#[test]
fn frontmatter_supports_blocks_and_heading_fallback() {
    let f = Fixture::new();
    let dir = f.temp.path().join("skill");
    atomic_write(
        &dir.join("SKILL.md"),
        b"---\nname: 'design'\ndescription: >-\n  First line\n  second line\n---\n# Heading",
    )
    .unwrap();
    assert_eq!(
        frontmatter(&dir).unwrap(),
        ("design".into(), "First line second line".into())
    );
    atomic_write(&dir.join("SKILL.md"), b"# Fallback title\nBody").unwrap();
    assert_eq!(description(&dir), "Fallback title");
}
#[test]
fn discovery_deduplicates_host_layouts_and_ignores_internal() {
    let f = Fixture::new();
    let root = f.temp.path().join("repo");
    write_skill(&root.join(".agents/skills/design"), "design", "Agent copy");
    write_skill(&root.join(".claude/skills/design"), "design", "Claude copy");
    write_skill(&root.join("skills/productivity/tdd"), "tdd", "Nested");
    atomic_write(
        &root.join("skills/internal/SKILL.md"),
        b"---\nname: hidden\ndescription: internal\ninternal: true\n---",
    )
    .unwrap();
    let found = discover(&root);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].path, ".agents/skills/design");
    assert!(found.iter().all(|c| c.name != "hidden"));
}
#[test]
fn discovery_root_and_plugin_manifest() {
    let f = Fixture::new();
    let root = f.temp.path().join("repo");
    write_skill(&root, "single", "Root skill");
    assert_eq!(discover(&root)[0].path, ".");
    remove(&root.join("SKILL.md")).unwrap();
    let dir = root.join("deep/a/b/c/d/e/f/custom");
    write_skill(&dir, "plugin", "Plugin skill");
    atomic_write(
        &root.join(".claude-plugin/marketplace.json"),
        br#"{"plugins":[{"skills":["./deep/a/b/c/d/e/f/custom"]}]}"#,
    )
    .unwrap();
    assert_eq!(discover(&root)[0].name, "plugin");
}
#[test]
fn source_repository_display_and_path_resolution() {
    let f = Fixture::new();
    let item = SkillLock {
        source: "https://github.com/org/repo".into(),
        r#ref: "dev".into(),
        skill_path: ".".into(),
        ..SkillLock::default()
    };
    assert_eq!(repository(&item), "https://github.com/org/repo#dev");
    let found = vec![SkillCandidate {
        name: "taste".into(),
        path: "skills/taste-skill".into(),
        ..SkillCandidate::default()
    }];
    assert_eq!(
        resolve(&item, "taste-skill", f.temp.path(), &found),
        Some("skills/taste-skill".into())
    );
}
#[test]
fn host_paths_and_visibility_persist() {
    let mut f = Fixture::new();
    let base = f.temp.path().join("overrides");
    f.app
        .update_host_paths(
            "codex",
            base.join("config.toml").to_str().unwrap(),
            base.join("skills").to_str().unwrap(),
            base.join("agents").to_str().unwrap(),
        )
        .unwrap();
    f.app.set_host_visible("codex", false).unwrap();
    let app = App::open(&f.app.root, &f.app.home).unwrap();
    assert_eq!(
        app.host("codex").unwrap().config_path,
        base.join("config.toml").to_string_lossy()
    );
    assert_eq!(app.preferences.hidden_hosts, vec!["codex"]);
    assert!(f.app.update_host_paths("missing", "a", "b", "c").is_err());
}
#[test]
fn native_command_boundary_exposes_all_supported_operations() {
    let mut f = Fixture::new();
    for method in [
        "GetSnapshot",
        "ReadSkill",
        "ReadAgent",
        "SetHostVisible",
        "ListImportCandidates",
        "ImportHostResources",
        "Refresh",
        "PreviewSkills",
        "InstallSkills",
        "CheckSkillUpdates",
        "UpdateSkill",
        "RemoveSkill",
        "DeleteSkill",
        "ImportSkill",
        "ToggleSkill",
        "AddMCP",
        "ToggleMCP",
        "TestMCP",
        "AddAgent",
        "ToggleAgent",
        "RestoreBackup",
        "UpdateHostPaths",
        "CreatePrompt",
        "ReadPrompt",
        "UpdatePrompt",
        "DeletePrompt",
        "TogglePrompt",
        "CreateReference",
        "ReadReference",
        "UpdateReference",
        "DeleteReference",
        "CreateProvider",
        "UpdateProvider",
        "DeleteProvider",
        "RefreshProviderModels",
        "AddProject",
        "RenameProject",
        "RelocateProject",
        "ToggleProjectSkill",
        "ResetProjectSkill",
        "DeleteProject",
    ] {
        let args = if method == "AddProject" {
            json!([f.temp.path().join("missing-project")])
        } else {
            json!([])
        };
        if let Err(error) = f.app.execute(method, args) {
            assert!(
                !error.to_string().starts_with("未知操作:"),
                "native operation is missing: {method}"
            );
        }
    }
    assert!(
        f.app
            .execute("Unknown", json!([]))
            .unwrap_err()
            .to_string()
            .starts_with("未知操作:")
    );
    assert_eq!(f.app.snapshot().unwrap().hosts.len(), 4);
}

#[cfg(unix)]
#[test]
fn writing_does_not_change_existing_directory_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let directory = f.temp.path().join("existing");
    mkdir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
    atomic_write(&directory.join("file"), b"private").unwrap();
    assert_eq!(
        fs::metadata(directory).unwrap().permissions().mode() & 0o777,
        0o755
    );
}
#[test]
fn malicious_prompt_lock_file_paths_are_rejected() {
    let mut f = Fixture::new();
    let mut lock = f.app.read_lock().unwrap();
    lock.prompts.insert(
        "escape".into(),
        PromptLock {
            name: "escape".into(),
            file: "../state.json".into(),
            ..PromptLock::default()
        },
    );
    f.app.write_lock(&lock).unwrap();
    assert!(f.app.snapshot().is_err());
}

#[test]
fn search_covers_repository_paths_and_mcp_technical_fields() {
    let mut state = chu::presenter::ViewState {
        page: 2,
        query: "owner/repo".into(),
        ..Default::default()
    };
    state.snapshot.skills.push(SkillView {
        id: "tdd".into(),
        managed: true,
        name: "tdd".into(),
        repository: "https://github.com/owner/repo".into(),
        source: "/isolated/skills/tdd".into(),
        ..SkillView::default()
    });
    assert_eq!(chu::search::rows(&state).len(), 1);
    state.query = "/isolated".into();
    assert_eq!(chu::search::rows(&state).len(), 1);
    state.page = 3;
    state.query = "npx".into();
    state.snapshot.mcps.push(McpView {
        id: "server".into(),
        managed: true,
        name: "server".into(),
        command: "npx".into(),
        endpoint: "https://fixture.invalid/mcp".into(),
        ..McpView::default()
    });
    assert_eq!(chu::search::rows(&state).len(), 1);
    state.query = "fixture.invalid".into();
    assert_eq!(chu::search::rows(&state).len(), 1);
}
