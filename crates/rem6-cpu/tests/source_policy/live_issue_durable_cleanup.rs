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

    let mut mutated = canonical.clone();
    mutated.issue = mutated.issue.replacen(
        "self.complete_durable_live_issue_removal_at(head.issue_tick, &[head.sequence()]);",
        "let _ = head.issue_tick;",
        1,
    );
    assert_rejected("head cleanup removed", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.control = mutated.control.replacen(
        "self.complete_durable_live_issue_removal_at(issue_tick, &[sequence]);",
        "let _ = issue_tick;",
        1,
    );
    assert_rejected("control cleanup removed", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.pending = mutated.pending.replacen(
        "self.complete_durable_live_issue_removal_at(issue_tick, &[sequence]);",
        "let _ = issue_tick;",
        1,
    );
    assert_rejected("pending cleanup removed", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.service = mutated.service.replacen(
        "self.complete_durable_live_issue_removal_at(now, &issued_sequences);",
        "let _ = &issued_sequences;",
        1,
    );
    assert_rejected("service post-commit cleanup removed", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.service = mutated
        .service
        .replacen(
            "Ok(O3LiveIssueBatchOutcome::Recorded) => recorded_rows,",
            "Ok(O3LiveIssueBatchOutcome::Recorded) => {\n                        self.complete_durable_live_issue_removal_at(now, &[]);\n                        recorded_rows\n                    },",
            1,
        )
        .replacen(
            "self.complete_durable_live_issue_removal_at(now, &issued_sequences);",
            "let _ = &issued_sequences;",
            1,
        );
    assert_rejected(
        "service cleanup moved before commit exit",
        &canonical,
        mutated,
    );

    let mut mutated = canonical.clone();
    mutated.transaction = mutated.transaction.replacen(
        "debug_assert!(reservations.is_empty());",
        "debug_assert!(reservations.is_empty());\n    runtime.complete_durable_live_issue_removal_at(0, &[]);",
        1,
    );
    assert_rejected(
        "transaction mutates retained decisions",
        &canonical,
        mutated,
    );

    let mut mutated = canonical.clone();
    mutated.owner = mutated.owner.replacen(
        "debug_assert!(!self.live_issue.transaction_active());",
        "let _ = self.live_issue.transaction_active();",
        1,
    );
    assert_rejected("transaction guard removed", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.state = mutated.state.replacen(
        "self.remove_active_blocked_sequence_at_or_after(tick, sequence);",
        "self.remove_durable_blocked_sequences_at_or_after(tick, &[sequence]);",
        1,
    );
    assert_rejected(
        "rollbackable removal mutates retained window",
        &canonical,
        mutated,
    );

    let mut mutated = canonical.clone();
    mutated.decision_state = mutated.decision_state.replacen(
        ".filter(|active| active.tick() >= tick)",
        ".filter(|active| active.tick() > tick)",
        1,
    );
    assert_rejected("same-tick active cleanup skipped", &canonical, mutated);

    let mut mutated = canonical.clone();
    mutated.decision_window = mutated.decision_window.replacen(
        "self.ticks.range_mut(tick..)",
        "self.ticks.range_mut(tick.saturating_add(1)..)",
        1,
    );
    assert_rejected("same-tick retained cleanup skipped", &canonical, mutated);
}

#[derive(Clone)]
struct DurableCleanupSources {
    issue: String,
    control: String,
    pending: String,
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
            issue: read("src/o3_runtime_issue.rs"),
            control: read("src/o3_runtime_control_window.rs"),
            pending: read("src/o3_runtime_pending_address.rs"),
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
    mutated: DurableCleanupSources,
) {
    assert!(
        sources_differ(canonical, &mutated),
        "durable-cleanup mutation did not apply: {description}",
    );
    assert!(
        !durable_cleanup_policy_holds(&mutated),
        "accepted weakened durable-cleanup mutation: {description}",
    );
}

fn sources_differ(left: &DurableCleanupSources, right: &DurableCleanupSources) -> bool {
    left.issue != right.issue
        || left.control != right.control
        || left.pending != right.pending
        || left.service != right.service
        || left.transaction != right.transaction
        || left.state != right.state
        || left.decision_state != right.decision_state
        || left.decision_window != right.decision_window
        || left.owner != right.owner
}

fn durable_cleanup_policy_holds(sources: &DurableCleanupSources) -> bool {
    let Some(head) = rust_function_definition(&sources.issue, "record_live_issue_head_execution")
    else {
        return false;
    };
    let Some(control) =
        rust_function_definition(&sources.control, "record_live_speculative_execution")
    else {
        return false;
    };
    let Some(pending) = rust_function_definition(
        &sources.pending,
        "record_pending_data_address_materialization",
    ) else {
        return false;
    };
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
    let Some(owner) =
        rust_function_definition(&sources.owner, "complete_durable_live_issue_removal_at")
    else {
        return false;
    };

    let compact = |source: &str| compact_rust_code(source);
    let head = compact(&head);
    let control = compact(&control);
    let pending = compact(&pending);
    let service = compact(&service);
    let remove_exact = compact(&remove_exact);
    let remove_active = compact(&remove_active);
    let remove_durable = compact(&remove_durable);
    let remove_window = compact(&remove_window);
    let owner = compact(&owner);
    let transaction = compact(&sources.transaction);

    ordered_once(
        &head,
        "self.live_issue.remove_selected_at(",
        "self.complete_durable_live_issue_removal_at(head.issue_tick,&[head.sequence()]);",
    ) && ordered_once(
        &control,
        "self.live_issue.remove_selected_at(",
        "self.complete_durable_live_issue_removal_at(issue_tick,&[sequence]);",
    ) && ordered_once(
        &pending,
        "self.live_issue.remove_exact_at(",
        "self.complete_durable_live_issue_removal_at(issue_tick,&[sequence]);",
    ) && service.contains("self.complete_durable_live_issue_removal_at(now,&issued_sequences);")
        && rust_anchor_occurs_at_brace_depth(
            &service,
            "self.complete_durable_live_issue_removal_at(now,&issued_sequences);",
            1,
        )
        && ordered_once(
            &service,
            "O3LiveIssueTransaction::record(self,rows)",
            "self.complete_durable_live_issue_removal_at(now,&issued_sequences);",
        )
        && !transaction.contains("complete_durable_live_issue_removal_at")
        && !transaction.contains("remove_durable_blocked_sequences_at_or_after")
        && owner.contains("debug_assert!(!self.live_issue.transaction_active());")
        && owner.contains(
            "self.live_issue.remove_durable_blocked_sequences_at_or_after(tick,sequences);",
        )
        && remove_exact.contains("self.remove_active_blocked_sequence_at_or_after(tick,sequence);")
        && !remove_exact.contains("remove_durable_blocked_sequences_at_or_after")
        && remove_active.contains(".filter(|active|active.tick()>=tick)")
        && remove_active.contains("active.remove_blocked(sequence);")
        && remove_durable.contains("forsequenceinsequences")
        && remove_durable
            .contains("self.remove_active_blocked_sequence_at_or_after(tick,*sequence);")
        && remove_durable
            .contains("self.decision_window.remove_blocked_at_or_after(tick,sequences);")
        && remove_window.contains("self.ticks.range_mut(tick..)")
        && remove_window.contains("decision.remove_blocked(*sequence);")
}

fn ordered_once(source: &str, first: &str, second: &str) -> bool {
    source.matches(first).count() == 1
        && source.matches(second).count() == 1
        && source.find(first) < source.find(second)
}
