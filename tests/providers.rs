mod common;
use chu::{credentials, fs::*, models::*, providers::fetch_models};
use common::*;
use std::fs;
#[test]
fn credential_records_preserve_comments_and_special_values() {
    let original = b"# header\r\nOTHER=keep\r\nTOKEN=\"old\" # inline\r\n# tail\r\n";
    for key in [
        "000123",
        "a\"b\\c$literal",
        "line one\nline two",
        "hash # in value",
        "中文凭证测试",
        "first\nsecond\rthird",
        "tab\tkey",
        r#"quotes'\"$HOME\backslash# =!"#,
    ] {
        let updated = credentials::update(original, "TOKEN", key).unwrap();
        let text = String::from_utf8(updated).unwrap();
        assert!(text.contains("# header\r\nOTHER=keep\r\n"));
        assert!(text.contains("# inline"));
        assert!(text.ends_with("# tail\r\n"));
    }
}
#[test]
fn credential_duplicate_and_multiline_records_are_replaced_once() {
    let data =
        b"# start\nTOKEN=\"old\nmultiline\" # keep\nexport TOKEN=again # duplicate\nOTHER=ok\n";
    let updated = credentials::update(data, "TOKEN", "new").unwrap();
    let text = String::from_utf8(updated).unwrap();
    assert_eq!(text.matches("TOKEN=").count(), 1);
    assert!(text.contains("# duplicate"));
    assert!(text.contains("OTHER=ok"));
}
#[test]
fn credential_invalid_input_is_not_rewritten() {
    assert!(credentials::update(b"TOKEN=\"unfinished", "TOKEN", "new").is_err());
    let text = String::from_utf8(credentials::update(b"# only comment", "TOKEN", "0001").unwrap())
        .unwrap();
    assert!(text.starts_with("# only comment\n"));
    assert!(text.contains("0001"));
}
#[test]
fn providers_crud_models_and_secret_free_snapshots() {
    let mut f = Fixture::new();
    let mut server = Server::new(vec![
        (200, r#"{"data":[{"id":"model-a"}]}"#.into(), vec![]),
        (200, r#"{"data":[{"id":"model-b"}]}"#.into(), vec![]),
    ]);
    f.app
        .save_provider(
            "",
            ProviderInput {
                name: "Fixture".into(),
                api_key: "fixture-secret".into(),
                env_api_key: "FIXTURE_API_KEY".into(),
                base_url: server.url.clone(),
            },
        )
        .unwrap();
    let p = f.app.state.providers[0].clone();
    assert_eq!(p.models, vec!["model-a"]);
    let serialized = serde_json::to_string(&f.app.snapshot().unwrap()).unwrap();
    assert!(!serialized.contains("fixture-secret"));
    f.app
        .save_provider(
            &p.id,
            ProviderInput {
                name: "Renamed".into(),
                api_key: String::new(),
                env_api_key: "FIXTURE_API_KEY".into(),
                base_url: server.url.clone(),
            },
        )
        .unwrap();
    f.app.refresh_provider_models(&p.id).unwrap();
    assert_eq!(f.app.state.providers[0].models, vec!["model-b"]);
    server.finish();
    assert_eq!(server.requests.lock().unwrap().len(), 2);
    assert!(server.requests.lock().unwrap().iter().all(|r| {
        r.to_lowercase()
            .contains("authorization: bearer fixture-secret")
    }));
    f.app.delete_provider(&p.id).unwrap();
    assert!(f.app.state.providers.is_empty());
}
#[test]
fn provider_validation_covers_urls_env_names_and_duplicates() {
    let mut f = Fixture::new();
    for (name, base, env, key) in [
        ("", "https://example.invalid", "", ""),
        ("bad", "file:///tmp/a", "", ""),
        ("bad", "https://user:pass@example.invalid", "", ""),
        ("bad", "https://example.invalid?x=1", "", ""),
        ("bad", "https://example.invalid#frag", "", ""),
        ("bad", "https://example.invalid", "1INVALID", ""),
        ("bad", "https://example.invalid", "", "secret"),
    ] {
        assert!(
            f.app
                .save_provider(
                    "",
                    ProviderInput {
                        name: name.into(),
                        base_url: base.into(),
                        env_api_key: env.into(),
                        api_key: key.into()
                    }
                )
                .is_err()
        );
    }
    let mut s = Server::models();
    f.app
        .save_provider(
            "",
            ProviderInput {
                name: "Fixture".into(),
                base_url: s.url.clone(),
                ..ProviderInput::default()
            },
        )
        .unwrap();
    s.finish();
    assert!(
        f.app
            .save_provider(
                "",
                ProviderInput {
                    name: "fixture".into(),
                    base_url: s.url.clone(),
                    ..ProviderInput::default()
                }
            )
            .is_err()
    );
    assert!(
        f.app
            .save_provider("missing", ProviderInput::default())
            .is_err()
    );
}
#[test]
fn missing_credential_is_visible_without_erasing_provider() {
    let mut f = Fixture::new();
    f.app
        .save_provider(
            "",
            ProviderInput {
                name: "Missing".into(),
                base_url: "https://example.invalid/v1".into(),
                env_api_key: "CHU_TEST_UNDEFINED_738426".into(),
                ..ProviderInput::default()
            },
        )
        .unwrap();
    assert!(f.app.state.providers[0].models_error.contains("未定义"));
}
#[test]
fn failure_preserves_last_successful_model_cache() {
    let mut f = Fixture::new();
    let mut s = Server::new(vec![
        (200, r#"{"data":[{"id":"cached"}]}"#.into(), vec![]),
        (401, "denied".into(), vec![]),
    ]);
    f.app
        .save_provider(
            "",
            ProviderInput {
                name: "Fixture".into(),
                base_url: s.url.clone(),
                ..ProviderInput::default()
            },
        )
        .unwrap();
    let id = f.app.state.providers[0].id.clone();
    let time = f.app.state.providers[0].models_fetched_at.clone();
    f.app.refresh_provider_models(&id).unwrap();
    s.finish();
    assert_eq!(f.app.state.providers[0].models, vec!["cached"]);
    assert_eq!(f.app.state.providers[0].models_fetched_at, time);
    assert!(f.app.state.providers[0].models_error.contains("401"));
}
#[test]
fn model_response_requires_valid_envelope_and_nonempty_ids() {
    for body in [
        "not json",
        r#"{"data":null}"#,
        r#"{"data":{}}"#,
        r#"{"data":[{}]}"#,
        r#"{"data":[{"id":"  "}]}"#,
    ] {
        let mut s = Server::new(vec![(200, body.into(), vec![])]);
        assert!(fetch_models(&s.url, "").is_err());
        s.finish();
    }
}
#[test]
fn model_response_limit_and_redirects_never_forward_credentials() {
    let mut s = Server::new(vec![(200, " ".repeat((4 << 20) + 1), vec![])]);
    assert!(
        fetch_models(&s.url, "")
            .unwrap_err()
            .to_string()
            .contains("4 MiB")
    );
    s.finish();
    let mut redirect = Server::new(vec![(
        302,
        String::new(),
        vec![("Location".into(), "http://127.0.0.1:1/leak".into())],
    )]);
    assert!(
        fetch_models(&redirect.url, "fixture-secret")
            .unwrap_err()
            .to_string()
            .contains("302")
    );
    redirect.finish();
    assert_eq!(redirect.requests.lock().unwrap().len(), 1);
}
#[test]
fn credential_and_state_failure_rolls_back_consistently() {
    let mut f = Fixture::new();
    let root = f.app.root.clone();
    mkdir(&root.join("state.json")).unwrap();
    let state = f.app.state.clone();
    assert!(
        f.app
            .save_provider(
                "",
                ProviderInput {
                    name: "Failure".into(),
                    api_key: "fixture-secret".into(),
                    env_api_key: "TEST_KEY".into(),
                    base_url: "http://127.0.0.1:1".into()
                }
            )
            .is_err()
    );
    assert_eq!(f.app.state, state);
    assert!(credentials::key(&root, "TEST_KEY").is_err());
}
#[test]
fn cache_save_failure_preserves_memory_state() {
    let mut f = Fixture::new();
    let mut s = Server::models();
    f.app
        .save_provider(
            "",
            ProviderInput {
                name: "Fixture".into(),
                base_url: s.url.clone(),
                ..ProviderInput::default()
            },
        )
        .unwrap();
    s.finish();
    let previous = f.app.state.clone();
    remove(&f.app.root.join("state.json")).unwrap();
    mkdir(&f.app.root.join("state.json")).unwrap();
    assert!(
        f.app
            .refresh_provider_models(&previous.providers[0].id)
            .is_err()
    );
    assert_eq!(f.app.state, previous);
}
#[cfg(unix)]
#[test]
fn credential_link_is_rejected_without_reading_external_content() {
    let mut f = Fixture::new();
    let outside = f.temp.path().join("outside");
    atomic_write(&outside, b"TOKEN=private").unwrap();
    mkdir(&f.app.root).unwrap();
    symlink(&outside, &f.app.root.join(".env")).unwrap();
    assert!(
        f.app
            .save_provider(
                "",
                ProviderInput {
                    name: "Fixture".into(),
                    base_url: "http://127.0.0.1:1".into(),
                    env_api_key: "TOKEN".into(),
                    api_key: "new".into()
                }
            )
            .is_err()
    );
    assert_eq!(fs::read(outside).unwrap(), b"TOKEN=private");
}
