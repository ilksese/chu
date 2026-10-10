use super::*;
fn candidate(id: &str, name: &str, host: &str) -> ImportCandidate {
    ImportCandidate {
        id: id.into(),
        name: name.into(),
        host_id: host.into(),
        host_name: host.into(),
        available: true,
        r#type: "skill".into(),
        ..Default::default()
    }
}
#[test]
fn filtering_preserves_invisible_selections_and_detects_conflicts() {
    let mut state = ImportState::default();
    state.merge(ImportScan {
        candidates: vec![
            candidate("a", "same", "claude"),
            candidate("b", "same", "codex"),
            candidate("c", "other", "codex"),
        ],
        ..Default::default()
    });
    state.selected.extend(["a".into(), "b".into()]);
    state.query = "other".into();
    assert_eq!(state.visible().len(), 1);
    assert_eq!(state.selected_candidates().len(), 2);
    assert!(!state.conflicts().is_empty());
    state.selected.remove("b");
    assert!(state.conflicts().is_empty());
    assert_eq!(state.selected_candidates()[0].id, "a");
}
#[test]
fn rescan_retains_fingerprints_and_host_identity_not_filter_index() {
    let mut state = ImportState::default();
    state.merge(ImportScan {
        candidates: vec![
            candidate("stable", "one", "codex"),
            candidate("old", "two", "jcode"),
        ],
        ..Default::default()
    });
    state.host_filter = 2;
    state.selected.extend(["stable".into(), "old".into()]);
    let (_, removed) = state.merge(ImportScan {
        candidates: vec![
            candidate("new-host", "three", "claude"),
            candidate("stable", "one", "codex"),
            candidate("changed", "two", "jcode"),
        ],
        ..Default::default()
    });
    assert_eq!(removed, 1);
    assert_eq!(state.host_filter, 3);
    assert!(state.selected.contains("stable"));
    assert!(!state.selected.contains("changed"));
    assert_eq!(state.visible()[0].host_id, "jcode");
}
