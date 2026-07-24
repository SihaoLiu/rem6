use super::*;

#[test]
fn o3_live_issue_durable_cleanup_is_post_commit_and_tick_aware() {
    let sources = DurableCleanupSources::read();
    assert!(durable_cleanup_policy_holds(&sources));
}

#[test]
fn o3_live_issue_durable_cleanup_policy_rejects_mutations() {
    let canonical = DurableCleanupSources::read();
    assert!(durable_cleanup_policy_holds(&canonical));

    let mut mutation = canonical.clone();
    mutation.service = mutation.service.replacen(
        "self.complete_committed_live_issue_removals_at(now, &issued_sequences);",
        "let _ = &issued_sequences;",
        1,
    );
    assert_rejected("service post-commit cleanup removed", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.service = mutation
        .service
        .replacen(
            "Ok(O3LiveIssueBatchOutcome::Recorded) => recorded_rows,",
            "Ok(O3LiveIssueBatchOutcome::Recorded) => {\n                        self.complete_committed_live_issue_removals_at(now, &[]);\n                        recorded_rows\n                    },",
            1,
        )
        .replacen(
            "self.complete_committed_live_issue_removals_at(now, &issued_sequences);",
            "let _ = &issued_sequences;",
            1,
        );
    assert_rejected(
        "cleanup moved inside transaction result",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.transaction = mutation.transaction.replacen(
        "debug_assert!(reservations.is_empty());",
        "debug_assert!(reservations.is_empty());\n    runtime.complete_committed_live_issue_removals_at(0, &[]);",
        1,
    );
    assert_rejected(
        "transaction mutates retained decisions",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.owner = mutation.owner.replacen(
        "self.live_issue\n            .remove_durable_blocked_sequences_at_or_after(tick, sequences);",
        "let _ = (tick, sequences);",
        1,
    );
    assert_rejected("committed cleanup delegate removed", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.owner = mutation.owner.replacen(
        "if !removed {\n            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });\n        }",
        "let _ = removed;",
        1,
    );
    assert_rejected(
        "failed raw removal no longer fails closed",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.state = mutation.state.replacen(
        "self.remove_active_blocked_sequence_at_or_after(tick, sequence);",
        "self.remove_durable_blocked_sequences_at_or_after(tick, &[sequence]);",
        1,
    );
    assert_rejected(
        "rollbackable removal mutates retained window",
        &canonical,
        mutation,
    );

    let mut mutation = canonical.clone();
    mutation.decision_state = mutation.decision_state.replacen(
        ".filter(|active| active.tick() >= tick)",
        ".filter(|active| active.tick() > tick)",
        1,
    );
    assert_rejected("same-tick active cleanup skipped", &canonical, mutation);

    let mut mutation = canonical.clone();
    mutation.decision_window = mutation.decision_window.replacen(
        "self.ticks.range_mut(tick..)",
        "self.ticks.range_mut(tick.saturating_add(1)..)",
        1,
    );
    assert_rejected("same-tick retained cleanup skipped", &canonical, mutation);
}

#[derive(Clone)]
struct DurableCleanupSources {
    service: String,
    transaction: String,
    state: String,
    decision_state: String,
    decision_window: String,
    owner: String,
}

impl DurableCleanupSources {
    fn read() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |relative: &str| {
            production_rust_source(&fs::read_to_string(root.join(relative)).unwrap())
        };
        Self {
            service: read("src/o3_runtime_issue/service.rs"),
            transaction: read("src/o3_runtime_issue/transaction.rs"),
            state: read("src/o3_runtime_issue/state.rs"),
            decision_state: read("src/o3_runtime_issue/state/decision_state.rs"),
            decision_window: read("src/o3_runtime_issue/state/decision_window.rs"),
            owner: read("src/o3_runtime_issue/durable_cleanup.rs"),
        }
    }
}

fn assert_rejected(
    description: &str,
    canonical: &DurableCleanupSources,
    mutation: DurableCleanupSources,
) {
    assert!(
        sources_differ(canonical, &mutation),
        "durable-cleanup mutation did not apply: {description}",
    );
    assert!(
        !durable_cleanup_policy_holds(&mutation),
        "accepted weakened durable-cleanup mutation: {description}",
    );
}

fn sources_differ(left: &DurableCleanupSources, right: &DurableCleanupSources) -> bool {
    left.service != right.service
        || left.transaction != right.transaction
        || left.state != right.state
        || left.decision_state != right.decision_state
        || left.decision_window != right.decision_window
        || left.owner != right.owner
}

fn durable_cleanup_policy_holds(sources: &DurableCleanupSources) -> bool {
    let Some(service) = rust_function_definition(&sources.service, "service_live_issue_queue_at")
    else {
        return false;
    };
    let Some(remove_exact) = rust_function_definition(&sources.state, "remove_exact_at") else {
        return false;
    };
    let Some(remove_active) = rust_function_definition(
        &sources.decision_state,
        "remove_active_blocked_sequence_at_or_after",
    ) else {
        return false;
    };
    let Some(remove_durable) = rust_function_definition(
        &sources.decision_state,
        "remove_durable_blocked_sequences_at_or_after",
    ) else {
        return false;
    };
    let Some(remove_window) =
        rust_function_definition(&sources.decision_window, "remove_blocked_at_or_after")
    else {
        return false;
    };
    let Some(finish) =
        rust_function_definition(&sources.owner, "finish_durable_live_issue_removal_at")
    else {
        return false;
    };
    let Some(committed) =
        rust_function_definition(&sources.owner, "complete_committed_live_issue_removals_at")
    else {
        return false;
    };

    let service = compact_rust_code(&service);
    let remove_exact = compact_rust_code(&remove_exact);
    let remove_active = compact_rust_code(&remove_active);
    let remove_durable = compact_rust_code(&remove_durable);
    let remove_window = compact_rust_code(&remove_window);
    let finish = compact_rust_code(&finish);
    let committed = compact_rust_code(&committed);
    let transaction = compact_rust_code(&sources.transaction);
    let cleanup = "self.complete_committed_live_issue_removals_at(now,&issued_sequences);";

    service.matches(cleanup).count() == 1
        && rust_anchor_occurs_at_brace_depth(&service, cleanup, 1)
        && ordered_once(
            &service,
            "O3LiveIssueTransaction::record(self,rows)",
            cleanup,
        )
        && !transaction.contains("complete_committed_live_issue_removals_at")
        && !transaction.contains("remove_durable_blocked_sequences_at_or_after")
        && committed.contains("self.assert_durable_live_issue_removal_boundary();")
        && committed.contains(
            "self.live_issue.remove_durable_blocked_sequences_at_or_after(tick,sequences);",
        )
        && finish.contains(
            "if!removed{returnErr(O3RuntimeError::InvalidLiveIssueQueueEntry{sequence});}",
        )
        && finish.contains(
            "self.live_issue.remove_durable_blocked_sequences_at_or_after(tick,&[sequence]);",
        )
        && remove_exact.contains("self.remove_active_blocked_sequence_at_or_after(tick,sequence);")
        && !remove_exact.contains("remove_durable_blocked_sequences_at_or_after")
        && remove_active.contains(".filter(|active|active.tick()>=tick)")
        && remove_durable.contains("forsequenceinsequences")
        && remove_durable
            .contains("self.remove_active_blocked_sequence_at_or_after(tick,*sequence);")
        && remove_window.contains("self.ticks.range_mut(tick..)")
        && remove_window.contains("decision.remove_blocked(*sequence);")
}

fn ordered_once(source: &str, first: &str, second: &str) -> bool {
    source.matches(first).count() == 1
        && source.matches(second).count() == 1
        && source.find(first) < source.find(second)
}
