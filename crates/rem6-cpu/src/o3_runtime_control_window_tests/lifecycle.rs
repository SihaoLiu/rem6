use super::*;

#[test]
fn inner_control_uses_staged_outer_ownership_before_execution_record() {
    let (mut runtime, outer, inner, _) = nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .expect("staged predicted-path ownership should not be a data wait");
    assert_eq!(inner_candidate.issue_tick(11), 11);
    assert_eq!(inner_candidate.producer_sequences(), &[outer_sequence]);

    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            outer_candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap();

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .is_some());
}

#[test]
fn outer_control_validation_preserves_inner_control_chain() {
    let (mut runtime, _, inner, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let descendant_sequence = rob[3].sequence();

    runtime.validate_live_speculative_producer(outer_sequence);

    assert!(!runtime
        .live_control_dependencies
        .contains_key(&inner_sequence));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant_sequence),
        Some(&inner_sequence)
    );
    let inner_record = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == inner)
        .unwrap();
    assert!(inner_record.producer_sequences.is_empty());
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == descendant));
}

#[test]
fn validated_outer_control_keeps_terminal_inner_timing_window_live() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = beq(5, 6);
    let inner = beq(4, 0);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [(Address::new(0x8004), outer), (Address::new(0x8008), inner)],
    );
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            outer_candidate,
            &[request(11)],
            11,
            RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
        )
        .unwrap();

    runtime.validate_live_speculative_producer(outer_sequence);

    assert!(runtime.live_control_dependencies.is_empty());
    assert!(runtime.has_live_control_window());
    assert!(runtime
        .live_control_window_sequences
        .contains(&inner_sequence));

    runtime.discard_live_staged_window_from(outer_sequence);

    assert!(!runtime.has_live_control_window());
}

#[test]
fn outer_control_discard_removes_inner_branch_and_descendant() {
    let (mut runtime, outer, inner, descendant) = issued_nested_control_runtime();
    let outer_sequence = runtime.snapshot().reorder_buffer()[1].sequence();

    runtime.discard_live_control_descendants_from_at(outer_sequence, 0);

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)]
    );
    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0]
            .execution
            .instruction(),
        outer
    );
    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| ![inner, descendant].contains(&issued.execution.instruction())));
}

#[test]
fn branch_descendant_cleanup_discards_only_future_writeback_reservations() {
    let (mut runtime, _, _, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let load_sequence = rob[0].sequence();
    let branch_sequence = rob[1].sequence();
    let descendant_sequence = rob[3].sequence();
    let mut load = runtime.live_data_accesses[0].execution.clone();
    load.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(&load, request(20), 10, 0, Some(&[0x2a, 0, 0, 0]),)
        .unwrap());
    let load_admitted_tick = runtime.live_data_accesses[0]
        .admitted_writeback_tick
        .unwrap();
    let descendant_admitted_tick = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == descendant)
        .unwrap()
        .admitted_writeback_tick;
    assert!(descendant_admitted_tick > 12);

    runtime.discard_live_control_descendants_from_at(branch_sequence, 12);

    assert_eq!(
        runtime.live_data_accesses[0].admitted_writeback_tick,
        Some(load_admitted_tick)
    );
    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| issued.sequence != descendant_sequence));
    assert_eq!(
        runtime
            .writeback_reservation(load_sequence)
            .map(O3WritebackReservation::admitted_tick),
        Some(load_admitted_tick)
    );
    assert!(runtime.writeback_reservation(descendant_sequence).is_none());
}

#[test]
fn branch_descendant_cleanup_discards_unpublished_past_writeback_reservation() {
    let (mut runtime, _, _, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let branch_sequence = rob[1].sequence();
    let descendant_sequence = rob[3].sequence();
    let descendant_admitted_tick = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == descendant)
        .expect("descendant execution is staged")
        .admitted_writeback_tick;

    runtime.discard_live_control_descendants_from_at(branch_sequence, descendant_admitted_tick);

    assert!(runtime.writeback_reservation(descendant_sequence).is_none());
}

#[test]
fn inner_control_discard_preserves_outer_branch() {
    let (mut runtime, outer, inner, _) = issued_nested_control_runtime();
    let inner_sequence = runtime.snapshot().reorder_buffer()[2].sequence();

    runtime.discard_live_control_descendants_from_at(inner_sequence, 0);

    let instructions = runtime
        .live_speculative_executions
        .iter()
        .map(|issued| issued.execution.instruction())
        .collect::<Vec<_>>();
    assert_eq!(instructions, [outer, inner]);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
}

#[test]
fn middle_control_discard_removes_only_inner_control() {
    let (mut runtime, outer, middle, inner) = issued_three_deep_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let middle_sequence = rob[2].sequence();
    let inner_sequence = rob[3].sequence();

    runtime.discard_live_control_descendants_from_at(middle_sequence, 0);

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008].map(Address::new)
    );
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .map(|issued| issued.execution.instruction())
            .collect::<Vec<_>>(),
        [outer, middle]
    );
    assert!(!runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == inner));
    assert_eq!(runtime.live_control_window_sequences.len(), 2);
    assert!(runtime
        .live_control_window_sequences
        .contains(&outer_sequence));
    assert!(runtime
        .live_control_window_sequences
        .contains(&middle_sequence));
    assert!(!runtime
        .live_control_window_sequences
        .contains(&inner_sequence));
}

#[test]
fn mixed_middle_control_discard_removes_only_indirect_jump() {
    let (mut runtime, direct_jump, conditional, indirect_jump) = issued_mixed_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let direct_jump_sequence = rob[1].sequence();
    let conditional_sequence = rob[2].sequence();
    let indirect_jump_sequence = rob[3].sequence();

    runtime.discard_live_control_descendants_from_at(conditional_sequence, 0);

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x8008].map(Address::new)
    );
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .map(|issued| issued.execution.instruction())
            .collect::<Vec<_>>(),
        [direct_jump, conditional]
    );
    assert!(!runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == indirect_jump));
    assert!(runtime
        .live_control_window_sequences
        .contains(&direct_jump_sequence));
    assert!(runtime
        .live_control_window_sequences
        .contains(&conditional_sequence));
    assert!(!runtime
        .live_control_window_sequences
        .contains(&indirect_jump_sequence));
}

#[test]
fn split_inner_branch_suffix_replacement_prunes_nested_chain() {
    let (mut runtime, outer, inner, _) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    runtime.validate_live_speculative_producer(outer_sequence);

    let inner_execution = runtime
        .live_speculative_executions
        .iter_mut()
        .find(|issued| issued.sequence == inner_sequence)
        .map(|issued| {
            issued.consumed_requests = vec![request(12), request(14)];
            issued.execution.clone()
        })
        .unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8008, 12), inner, inner_execution),
        &[request(12), request(15)],
        40,
    );

    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(
        runtime.live_speculative_executions[0]
            .execution
            .instruction(),
        outer
    );
    assert!(runtime.live_control_dependencies.is_empty());
}

#[test]
fn outer_control_redirect_discards_invalidated_descendant_fetch_identity() {
    let (mut runtime, _, inner, _) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let inner_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == inner_sequence)
        .map(|issued| issued.execution.clone())
        .unwrap();
    let inner_event = RiscvCpuExecutionEvent::new(fetch_event(0x8008, 12), inner, inner_execution);
    runtime.retire_live_staged_instruction(&inner_event, &[request(14)], 40);
    runtime.record_retired_instruction_with_trace(&inner_event, true);
    assert_eq!(runtime.invalidated_live_staged_fetch_identities.len(), 1);
    assert!(runtime.owns_pending_retirement_authority(request(13)));

    runtime.discard_live_control_descendants_from_at(outer_sequence, 40);

    assert!(runtime.invalidated_live_staged_fetch_identities.is_empty());
    assert!(!runtime.owns_pending_retirement_authority(request(13)));
}

#[test]
fn mismatched_control_redirect_preserves_current_fallback_until_runtime_recording() {
    let (mut runtime, _, inner, _) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let inner_sequence = rob[2].sequence();
    let inner_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == inner_sequence)
        .map(|issued| issued.execution.clone())
        .unwrap();
    let inner_event = RiscvCpuExecutionEvent::new(fetch_event(0x8008, 12), inner, inner_execution);
    runtime.retire_live_staged_instruction(&inner_event, &[request(14)], 40);
    assert!(runtime.owns_pending_retirement_authority(request(12)));
    assert!(runtime.owns_pending_retirement_authority(request(13)));

    runtime.discard_live_control_descendants_from_at(inner_sequence, 40);

    assert!(runtime.owns_pending_retirement_authority(request(12)));
    assert!(!runtime.owns_pending_retirement_authority(request(13)));
    let staged_rows_before = runtime.snapshot.reorder_buffer.clone();
    runtime.record_retired_instruction_with_trace(&inner_event, true);

    assert_eq!(runtime.snapshot.reorder_buffer, staged_rows_before);
    assert!(runtime
        .snapshot
        .reorder_buffer
        .iter()
        .all(|entry| entry.is_live_staged() && !entry.is_ready()));
    assert_eq!(
        runtime
            .trace_records()
            .last()
            .unwrap()
            .rob_commits_at_tick(),
        1
    );
}

#[test]
fn predicted_descendants_use_staged_branch_ownership_and_invalidate_with_it() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let branch = beq(5, 6);
    let multiply = mul(7, 1, 2);
    let dependent = addi(8, 7, 1);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), branch),
            (Address::new(0x8008), multiply),
            (Address::new(0x800c), dependent),
        ],
    );

    let branch_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
    let multiply_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), multiply)
        .expect("predicted descendant should use staged branch ownership");
    assert_eq!(multiply_candidate.issue_tick(11), 11);
    assert_eq!(multiply_candidate.producer_sequences(), &[branch_sequence]);

    let branch_execution = RiscvExecutionRecord::new(branch, 0x8004, 0x8008, Vec::new(), None);
    let branch_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), branch)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            branch_candidate,
            &[request(11)],
            11,
            branch_execution.clone(),
        )
        .unwrap();

    let multiply_execution = RiscvExecutionRecord::new(
        multiply,
        0x8008,
        0x800c,
        vec![RegisterWrite::new(reg(7), 42)],
        None,
    );
    runtime
        .record_live_speculative_execution(
            multiply_candidate,
            &[request(12)],
            12,
            multiply_execution,
        )
        .unwrap();
    let dependent_execution = RiscvExecutionRecord::new(
        dependent,
        0x800c,
        0x8010,
        vec![RegisterWrite::new(reg(8), 43)],
        None,
    );
    let dependent_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), dependent)
        .unwrap();
    runtime
        .record_live_speculative_execution(
            dependent_candidate,
            &[request(13)],
            14,
            dependent_execution,
        )
        .unwrap();
    assert_eq!(runtime.live_speculative_executions.len(), 3);

    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(0x8004, 99), branch, branch_execution),
        &[request(99)],
        40,
    );

    assert!(runtime.live_speculative_executions.is_empty());
}
