use super::*;

#[test]
fn live_issue_raw_removal_is_owned_by_transaction_durable_and_lifecycle_boundaries() {
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
        "src/o3_runtime_issue/state.rs",
        "pub(super) fn remove_suffix_at",
        "pub(in crate::o3_runtime) fn remove_suffix_at",
    );
    assert_rejected(
        "raw suffix removal visibility widened",
        &canonical,
        mutation,
    );

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

    let mut mutation = canonical.clone();
    mutation.files.push((
        PathBuf::from("src/o3_runtime_issue/cleanup_tests.rs"),
        "use super::*;\nfn hidden_raw_alias() { let remove = O3LiveIssueState::remove_exact_at; let _ = remove; }\n"
            .to_owned(),
    ));
    mutation.files.sort_by(|left, right| left.0.cmp(&right.0));
    assert_rejected(
        "tests-suffixed production file adds raw removal",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/state.rs",
        "#[cfg(test)]\n#[path = \"state/test_support_tests.rs\"]",
        "#[path = \"state/test_support_tests.rs\"]",
    );
    assert_rejected("test support loses cfg gate", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.replace_in(
        "src/o3_runtime_issue/state/rollback.rs",
        "#[cfg(test)]\n#[path = \"rollback_tests.rs\"]",
        "#[path = \"rollback_tests.rs\"]",
    );
    assert_rejected("rollback tests lose cfg gate", &canonical, mutation);
}

#[derive(Clone)]
struct RawRemovalSources {
    files: Vec<(PathBuf, String)>,
}

impl RawRemovalSources {
    fn read() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = rust_source_files(&root.join("src"))
            .into_iter()
            .map(|path| {
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                let source = fs::read_to_string(path).unwrap();
                (relative, source)
            })
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.0.cmp(&right.0));
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
    let Some(rollback) = sources.source("src/o3_runtime_issue/state/rollback.rs") else {
        return false;
    };
    let Some(owner) = sources.source("src/o3_runtime_issue/durable_cleanup.rs") else {
        return false;
    };
    let Some(lifecycle) = sources.source("src/o3_runtime_issue/lifecycle_cleanup.rs") else {
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
            let code = rust_code_without_comments_and_literals(source);
            let chars = code.chars().collect::<Vec<_>>();
            let exact = rust_identifier_count(&chars, "remove_exact_at");
            let selected = rust_identifier_count(&chars, "remove_selected_at");
            let suffix = rust_identifier_count(&chars, "remove_suffix_at");
            ((exact != 0) || (selected != 0) || (suffix != 0))
                .then(|| (path.clone(), exact, selected, suffix))
        })
        .collect::<Vec<_>>();
    let expected = vec![
        (
            PathBuf::from("src/o3_runtime_issue/durable_cleanup.rs"),
            1,
            1,
            0,
        ),
        (
            PathBuf::from("src/o3_runtime_issue/lifecycle_cleanup.rs"),
            1,
            0,
            1,
        ),
        (
            PathBuf::from("src/o3_runtime_issue/state/rollback_tests.rs"),
            1,
            1,
            0,
        ),
        (
            PathBuf::from("src/o3_runtime_issue/state/test_support_tests.rs"),
            1,
            0,
            1,
        ),
        (PathBuf::from("src/o3_runtime_issue/state.rs"), 2, 1, 1),
        (
            PathBuf::from("src/o3_runtime_issue/transaction.rs"),
            1,
            1,
            0,
        ),
    ];
    let compact_state = compact_rust_code(&production_rust_source(state));
    let compact_owner = compact_rust_code(&production_rust_source(owner));
    let compact_lifecycle = compact_rust_code(&production_rust_source(lifecycle));
    let compact_state_module = compact_rust_code(state);
    let compact_rollback_module = compact_rust_code(rollback);
    let compact_control_module = compact_rust_code(control);
    let compact_pending_module = compact_rust_code(pending);
    let transaction = production_rust_source(transaction);
    let issue = production_rust_source(issue);
    let control = production_rust_source(control);
    let pending = production_rust_source(pending);
    let service = production_rust_source(service);
    inventory == expected
        && compact_state_module
            .contains("#[cfg(test)]#[path=\"state/test_support_tests.rs\"]modtest_support;")
        && compact_rollback_module.contains("#[cfg(test)]#[path=\"rollback_tests.rs\"]modtests;")
        && compact_state.contains("pub(super)fnremove_exact_at")
        && compact_state.contains("pub(super)fnremove_selected_at")
        && compact_state.contains("pub(super)fnremove_suffix_at")
        && !compact_state.contains("pub(incrate::o3_runtime)fnremove_exact_at")
        && !compact_state.contains("pub(incrate::o3_runtime)fnremove_suffix_at")
        && compact_owner.contains("fnassert_durable_live_issue_removal_boundary")
        && compact_owner.contains("assert!(!self.live_issue.transaction_active(),")
        && !compact_owner.contains("debug_assert!(!self.live_issue.transaction_active()")
        && compact_owner.contains("fnremove_durable_live_issue_at")
        && compact_owner.contains("fncomplete_committed_live_issue_removals_at")
        && compact_owner.contains("remove_durable_blocked_sequences_at_or_after(tick,sequences)")
        && compact_lifecycle.contains("fndiscard_live_issue_exact_at")
        && compact_lifecycle.contains(".remove_exact_at(sequence,action,pc,issue_class,now)")
        && compact_lifecycle.contains("fndiscard_live_issue_suffix_at")
        && compact_lifecycle.contains(".remove_suffix_at(boundary,action,&rows,now)")
        && compact_lifecycle.contains("fndiscard_all_live_issue_transient_state")
        && compact_lifecycle.contains("self.live_issue.discard_all();")
        && !transaction.contains("remove_durable_blocked_sequences_at_or_after")
        && !transaction.contains("complete_committed_live_issue_removals_at")
        && issue.contains("remove_durable_live_issue_at(")
        && !issue.contains(".remove_selected_at(")
        && compact_control_module
            .contains("#[cfg(test)]pub(crate)fnrecord_live_speculative_execution")
        && compact_control_module.contains("self.remove_durable_live_issue_at(")
        && !control.contains("remove_durable_live_issue_at(")
        && !control.contains(".remove_selected_at(")
        && compact_pending_module
            .contains("#[cfg(test)]pub(super)fnrecord_pending_data_address_materialization")
        && compact_pending_module.contains("self.remove_durable_live_issue_at(")
        && !pending.contains("remove_durable_live_issue_at(")
        && !pending.contains(".remove_exact_at(")
        && service.contains("complete_committed_live_issue_removals_at(")
        && !service.contains(".remove_exact_at(")
        && !service.contains(".remove_selected_at(")
}
