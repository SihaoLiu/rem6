use super::*;

pub(super) fn scalar_return_core(
    branch_lookahead: usize,
    with_return_fetch: bool,
    target_source: u8,
    link_destination: u8,
) -> RiscvCore {
    let load_raw = i_type(0, 18, 0x6, 12, 0x03);
    let producer_raw = i_type(0, 11, 0x0, target_source, 0x13);
    let call_raw = i_type(0, target_source, 0x0, link_destination, 0x67);
    let scalar_raw = i_type(0, link_destination, 0x0, 13, 0x13);
    let return_raw = i_type(0, link_destination, 0x0, 0, 0x67);
    let producer = RiscvInstruction::decode(producer_raw).unwrap();
    let call = RiscvInstruction::decode(call_raw).unwrap();
    let mut fetches = vec![
        (0, 0x8000, load_raw.to_le_bytes().to_vec()),
        (1, 0x8004, producer_raw.to_le_bytes().to_vec()),
        (2, 0x8008, call_raw.to_le_bytes().to_vec()),
        (3, 0x9000, scalar_raw.to_le_bytes().to_vec()),
    ];
    if with_return_fetch {
        fetches.push((4, 0x9004, return_raw.to_le_bytes().to_vec()));
    }
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(branch_lookahead);
    core.set_o3_scalar_memory_depth(4);
    let load = scalar_load_execution_event(0x8000, 0, 12, 18, 0x100);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&load, request(20), 31));
        assert_eq!(
            state.o3_runtime.stage_live_data_access_younger_window(
                request(0),
                [
                    (Address::new(0x8004), producer),
                    (Address::new(0x8008), call),
                ],
            ),
            2
        );
        let producer_candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .unwrap();
        assert!(state
            .o3_runtime
            .record_live_speculative_execution(
                producer_candidate,
                &[request(1)],
                20,
                RiscvExecutionRecord::new(
                    producer,
                    0x8004,
                    0x8008,
                    vec![rem6_isa_riscv::RegisterWrite::new(
                        Register::new(target_source).unwrap(),
                        0x9000,
                    )],
                    None,
                ),
            )
            .unwrap());
        let call_candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8008), call)
            .unwrap();
        assert!(state
            .o3_runtime
            .record_live_speculative_execution(
                call_candidate,
                &[request(2)],
                21,
                RiscvExecutionRecord::new(
                    call,
                    0x8008,
                    0x9000,
                    vec![rem6_isa_riscv::RegisterWrite::new(
                        Register::new(link_destination).unwrap(),
                        0x800c,
                    )],
                    None,
                ),
            )
            .unwrap());
    }
    core
}

pub(super) fn record_call_and_scalar(core: &RiscvCore) {
    let call_decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link call decision");
    assert_eq!(call_decision.pc(), Address::new(0x9000));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    if let Some(decision) = core.next_pending_data_fetch_ahead(true) {
        assert_eq!(decision.pc(), Address::new(0x9004));
        assert!(decision.branch_speculation().is_none());
    }
    let state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .producer_forwarded_scalar_descendant()
        .is_some());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
}

pub(super) fn retire_data_head(core: &RiscvCore, retire_tick: u64) {
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .retire_producer_forwarded_data_head_for_test(retire_tick));
}

#[test]
fn live_data_head_allows_scalar_sequential_fetch_but_not_return_row() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("live-head scalar sequential fetch decision");
    assert_eq!(decision.pc(), Address::new(0x9004));
    assert!(decision.branch_speculation().is_none());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(
        state.o3_runtime.producer_forwarded_return_descendant(),
        None
    );
}

#[test]
fn pending_data_gate_allows_typed_scalar_sequential_fetch() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let decision = core
        .next_pending_data_fetch_ahead(true)
        .expect("pending-data typed scalar continuation");
    assert_eq!(decision.pc(), Address::new(0x9004));
    assert!(decision.branch_speculation().is_none());
}

#[test]
fn scalar_continuation_preparation_holds_lineage_while_fetch_is_pending() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let continuation = core.next_pending_data_fetch_ahead(true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&continuation)
        .unwrap()
        .expect("typed scalar continuation preparation");
    core.record_prepared_fetch_ahead_speculation(Some(prepared));
    let pending = CpuFetchRecord::new(
        24,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(4),
        Address::new(0x9004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::issued(pending));
    core.state
        .lock()
        .expect("riscv core lock")
        .hart
        .set_pc(0x8008);

    assert!(!core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
}

#[test]
fn committed_scalar_continuation_retains_exact_return_authority() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let continuation = core.next_pending_data_fetch_ahead(true).unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&continuation).unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
    }
    let return_raw = i_type(0, 1, 0x0, 0, 0x67);
    let completed = CpuFetchRecord::new(
        92,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(4),
        Address::new(0x9004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            completed,
            return_raw.to_le_bytes().to_vec(),
        ));

    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("retained scalar-return prediction");
    assert_eq!(decision.pc(), Address::new(0x800c));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x9004));
    assert_eq!(speculation.target(), Some(Address::new(0x800c)));
    let PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) =
        speculation.target_authority()
    else {
        panic!("expected producer-forwarded scalar-return authority");
    };
    assert!(descendant.scalar_descendant().is_some());
}

#[test]
fn committed_call_seed_reconstructs_unstaged_scalar_return_authority() {
    let core = scalar_return_core(2, true, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
    }

    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("retained call-seed scalar-return prediction");
    assert_eq!(decision.pc(), Address::new(0x800c));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x9004));
    assert_eq!(speculation.target(), Some(Address::new(0x800c)));
    let PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) =
        speculation.target_authority()
    else {
        panic!("expected retained producer-forwarded scalar-return authority");
    };
    assert!(descendant.scalar_descendant().is_some());
}

#[test]
fn committed_call_seed_reconstructs_already_executed_scalar_return_authority() {
    let core = scalar_return_core(2, true, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
        state.executed_fetches.insert(request(3));
    }

    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("retained executed-scalar return prediction");
    assert_eq!(decision.pc(), Address::new(0x800c));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x9004));
    assert_eq!(speculation.target(), Some(Address::new(0x800c)));
}

#[test]
fn full_lookahead_at_call_recording_retains_later_scalar_return_authority() {
    let core = scalar_return_core(2, true, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&call_decision)
        .unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let older = state
            .branch_predictor
            .predict_speculative(Address::new(0x7000));
        state.branch_speculations.insert(1, older.id());
        state
            .branch_speculation_kinds
            .insert(1, BranchTargetKind::DirectConditional);
    }
    core.record_prepared_fetch_ahead_speculation(prepared);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 2);
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
        state.executed_fetches.insert(request(3));
    }

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

#[test]
fn prepared_scalar_continuation_survives_parent_commit_before_apply() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let continuation = core.next_pending_data_fetch_ahead(true).unwrap();
    let prepared = core.prepare_fetch_ahead_speculation(&continuation).unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
    }
    core.record_prepared_fetch_ahead_speculation(prepared);
    let return_raw = i_type(0, 1, 0x0, 0, 0x67);
    let completed = CpuFetchRecord::new(
        92,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(4),
        Address::new(0x9004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            completed,
            return_raw.to_le_bytes().to_vec(),
        ));

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

#[test]
fn scalar_stage_retains_authority_before_continuation_decision_is_prepared() {
    let core = scalar_return_core(2, false, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let continuation = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(continuation.pc(), Address::new(0x9004));
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .commit_return_address_stack_speculation(2, true)
            .unwrap();
        state.branch_speculations.clear();
        state.branch_speculation_kinds.clear();
        state.o3_runtime.discard_live_staged_instructions_at(30);
    }
    let return_raw = i_type(0, 1, 0x0, 0, 0x67);
    let completed = CpuFetchRecord::new(
        92,
        PartitionId::new(0),
        MemoryRouteId::new(0),
        endpoint("cpu0.ifetch"),
        request(4),
        Address::new(0x9004),
        AccessSize::new(4).unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(crate::CpuFetchEvent::completed(
            completed,
            return_raw.to_le_bytes().to_vec(),
        ));

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

#[test]
fn retired_data_head_opens_scalar_sequential_fetch() {
    let core = scalar_return_core(2, false, 1, 1);
    record_call_and_scalar(&core);
    retire_data_head(&core, 30);

    let decision = core
        .next_pending_data_fetch_ahead(false)
        .expect("scalar sequential fetch decision");
    assert_eq!(decision.pc(), Address::new(0x9004));
    assert!(decision.branch_speculation().is_none());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
}

#[test]
fn scalar_return_issue_waits_for_data_head_retirement_tick() {
    let core = scalar_return_core(2, true, 1, 1);
    record_call_and_scalar(&core);
    retire_data_head(&core, 90);
    assert!(core.next_pending_data_fetch_ahead(false).is_some());
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .producer_forwarded_scalar_return_issue_tick_for_test()
        .is_some_and(|tick| tick >= 90));
}

#[test]
fn branch_lookahead_one_does_not_stage_scalar_sequential_return() {
    let core = scalar_return_core(1, true, 1, 1);
    record_call_and_scalar(&core);
    retire_data_head(&core, 30);

    assert_eq!(core.next_pending_data_fetch_ahead(false), None);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(
        state.o3_runtime.producer_forwarded_return_descendant(),
        None
    );
}

#[test]
fn stale_ras_does_not_stage_scalar_sequential_return() {
    let core = scalar_return_core(2, true, 1, 1);
    record_call_and_scalar(&core);
    retire_data_head(&core, 30);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0xa000));
    }

    assert_eq!(core.next_pending_data_fetch_ahead(false), None);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(state.branch_speculations.len(), 1);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}

#[test]
fn incorrect_parent_resolution_discards_retained_scalar_authority() {
    let core = scalar_return_core(2, false, 1, 1);
    record_call_and_scalar(&core);
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state.producer_forwarded_scalar_continuation.is_some());

    state
        .commit_return_address_stack_speculation(2, false)
        .unwrap();

    assert!(state.producer_forwarded_scalar_continuation.is_none());
}

#[test]
fn branch_checkpoint_restore_discards_retained_scalar_authority() {
    let core = scalar_return_core(2, false, 1, 1);
    record_call_and_scalar(&core);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .producer_forwarded_scalar_continuation
        .is_some());

    core.restore_branch_predictor_checkpoint_payload(
        RiscvCore::default_branch_predictor_checkpoint_payload(),
    )
    .unwrap();

    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .producer_forwarded_scalar_continuation
        .is_none());
}

#[test]
fn scalar_return_apply_fails_closed_after_scalar_lineage_changes() {
    let core = scalar_return_core(2, true, 1, 1);
    record_call_and_scalar(&core);
    retire_data_head(&core, 30);
    let return_decision = core.next_pending_data_fetch_ahead(false).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let scalar_sequence = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .unwrap()
            .scalar_descendant()
            .unwrap()
            .sequence();
        state
            .o3_runtime
            .discard_live_control_descendants_from_at(scalar_sequence, 30);
    }

    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert_eq!(state.return_address_stack.pending_operation_count(), 1);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}
