use super::*;

#[test]
fn live_issue_raw_removal_is_owned_by_transaction_and_durable_boundary() {
    let sources = RawRemovalSources::read();
    assert!(raw_removal_policy_holds(&sources));
}

#[test]
fn live_issue_raw_removal_policy_rejects_new_callers_and_indirection() {
    let canonical = RawRemovalSources::read();
    assert!(raw_removal_policy_holds(&canonical));

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_control_window.rs",
        "impl O3RuntimeState {",
        "impl O3RuntimeState {\n    fn aliased_raw_removal(&mut self) {\n        let remove = O3LiveIssueState::remove_exact_at;\n        let _ = remove;\n    }",
    );
    assert_rejected("raw removal function-item alias", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/state.rs",
        "pub(super) fn remove_exact_at",
        "pub(in crate::o3_runtime) fn remove_exact_at",
    );
    assert_rejected("raw exact removal visibility widened", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/durable_cleanup.rs",
        "assert!(",
        "debug_assert!(",
    );
    assert_rejected(
        "release transaction assertion weakened",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/durable_cleanup.rs",
        "self.live_issue\n            .remove_durable_blocked_sequences_at_or_after(tick, sequences);",
        "let _ = (tick, sequences);",
    );
    assert_rejected("retained cleanup removed", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/transaction.rs",
        "debug_assert!(reservations.is_empty());",
        "debug_assert!(reservations.is_empty());\n    runtime.complete_committed_live_issue_removals_at(0, &[]);",
    );
    assert_rejected(
        "transaction mutates retained decisions",
        &canonical,
        mutation,
    );
}

#[derive(Clone)]
struct RawRemovalSources {
    files: Vec<(PathBuf, String)>,
}

impl RawRemovalSources {
    fn read() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let files = rust_source_files(&root.join("src"))
            .into_iter()
            .filter_map(|path| {
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                (!is_test_only_rust_source(&relative)).then(|| {
                    let source = fs::read_to_string(path).unwrap();
                    (relative, production_rust_source(&source))
                })
            })
            .collect();
        Self { files }
    }

    fn source(&self, relative: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(path, _)| path == Path::new(relative))
            .map(|(_, source)| source.as_str())
    }

    fn replace_in(&mut self, relative: &str, from: &str, to: &str) {
        let (_, source) = self
            .files
            .iter_mut()
            .find(|(path, _)| path == Path::new(relative))
            .unwrap_or_else(|| panic!("missing source {relative}"));
        let mutated = source.replacen(from, to, 1);
        assert_ne!(*source, mutated, "mutation did not apply in {relative}");
        *source = mutated;
    }
}

fn assert_rejected(description: &str, canonical: &RawRemovalSources, mutation: RawRemovalSources) {
    assert_ne!(
        canonical.files, mutation.files,
        "raw-removal mutation did not apply: {description}",
    );
    assert!(
        !raw_removal_policy_holds(&mutation),
        "accepted weakened raw-removal mutation: {description}",
    );
}

fn raw_removal_policy_holds(sources: &RawRemovalSources) -> bool {
    let Some(state) = sources.source("src/o3_runtime_issue/state.rs") else {
        return false;
    };
    let Some(owner) = sources.source("src/o3_runtime_issue/durable_cleanup.rs") else {
        return false;
    };
    let Some(transaction) = sources.source("src/o3_runtime_issue/transaction.rs") else {
        return false;
    };
    let Some(issue) = sources.source("src/o3_runtime_issue.rs") else {
        return false;
    };
    let Some(control) = sources.source("src/o3_runtime_control_window.rs") else {
        return false;
    };
    let Some(pending) = sources.source("src/o3_runtime_pending_address.rs") else {
        return false;
    };
    let Some(service) = sources.source("src/o3_runtime_issue/service.rs") else {
        return false;
    };

    let inventory = sources
        .files
        .iter()
        .filter_map(|(path, source)| {
            let chars = source.chars().collect::<Vec<_>>();
            let exact = rust_identifier_count(&chars, "remove_exact_at");
            let selected = rust_identifier_count(&chars, "remove_selected_at");
            ((exact != 0) || (selected != 0)).then(|| (path.clone(), exact, selected))
        })
        .collect::<Vec<_>>();
    let expected = vec![
        (
            PathBuf::from("src/o3_runtime_issue/durable_cleanup.rs"),
            1,
            1,
        ),
        (PathBuf::from("src/o3_runtime_issue/state.rs"), 2, 1),
        (PathBuf::from("src/o3_runtime_issue/transaction.rs"), 1, 1),
    ];
    let compact_state = compact_rust_code(state);
    let compact_owner = compact_rust_code(owner);

    inventory == expected
        && compact_state.contains("pub(super)fnremove_exact_at")
        && compact_state.contains("pub(super)fnremove_selected_at")
        && !compact_state.contains("pub(incrate::o3_runtime)fnremove_exact_at")
        && compact_owner.contains("fnassert_durable_live_issue_removal_boundary")
        && compact_owner.contains("assert!(!self.live_issue.transaction_active(),")
        && !compact_owner.contains("debug_assert!(!self.live_issue.transaction_active()")
        && compact_owner.contains("fnremove_durable_live_issue_at")
        && compact_owner.contains("fncomplete_committed_live_issue_removals_at")
        && compact_owner.contains("remove_durable_blocked_sequences_at_or_after(tick,sequences)")
        && !transaction.contains("remove_durable_blocked_sequences_at_or_after")
        && !transaction.contains("complete_committed_live_issue_removals_at")
        && issue.contains("remove_durable_live_issue_at(")
        && !issue.contains(".remove_selected_at(")
        && control.contains("remove_durable_live_issue_at(")
        && !control.contains(".remove_selected_at(")
        && pending.contains("remove_durable_live_issue_at(")
        && !pending.contains(".remove_exact_at(")
        && service.contains("complete_committed_live_issue_removals_at(")
        && !service.contains(".remove_exact_at(")
        && !service.contains(".remove_selected_at(")
}
