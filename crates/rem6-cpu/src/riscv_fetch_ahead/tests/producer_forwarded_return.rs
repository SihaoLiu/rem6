use super::*;

pub(super) fn live_x1_return_core(branch_lookahead: usize) -> RiscvCore {
    let load_raw = i_type(0, 18, 0x6, 12, 0x03);
    let producer_raw = i_type(0, 11, 0x0, 1, 0x13);
    let call_raw = i_type(0, 1, 0x0, 1, 0x67);
    let return_raw = i_type(0, 1, 0x0, 0, 0x67);
    let producer = RiscvInstruction::decode(producer_raw).unwrap();
    let call = RiscvInstruction::decode(call_raw).unwrap();
    let core = core_with_completed_fetches([
        (0, 0x8000, load_raw.to_le_bytes().to_vec()),
        (1, 0x8004, producer_raw.to_le_bytes().to_vec()),
        (2, 0x8008, call_raw.to_le_bytes().to_vec()),
        (3, 0x9000, return_raw.to_le_bytes().to_vec()),
    ]);
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
                        Register::new(1).unwrap(),
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
                        Register::new(1).unwrap(),
                        0x800c,
                    )],
                    None,
                ),
            )
            .unwrap());
    }
    core
}

#[test]
fn pending_data_gate_admits_producer_forwarded_call_target_return() {
    let core = live_x1_return_core(2);
    let call_decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link call decision");
    assert_eq!(call_decision.pc(), Address::new(0x9000));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let return_decision = core
        .next_pending_data_fetch_ahead(true)
        .expect("producer-forwarded call target return decision");
    assert_eq!(return_decision.pc(), Address::new(0x800c));
    let speculation = return_decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x9000));
    assert_eq!(speculation.target(), Some(Address::new(0x800c)));
    assert!(speculation.producer_forwarded_control_target.is_none());
    assert!(speculation.producer_forwarded_return_descendant.is_some());
}

#[test]
fn producer_forwarded_return_apply_fails_closed_after_descendant_invalidation() {
    let core = live_x1_return_core(2);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_pending_data_fetch_ahead(true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap();

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let parent_sequence = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .unwrap()
            .parent()
            .consumer_sequence();
        state
            .o3_runtime
            .discard_live_control_descendants_from_at(parent_sequence, 30);
    }
    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert_eq!(state.branch_speculation_kinds.len(), 1);
    assert_eq!(state.return_address_stack.pending_operation_count(), 1);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}

#[test]
fn producer_forwarded_return_apply_fails_closed_after_ras_lineage_changes() {
    let core = live_x1_return_core(2);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_pending_data_fetch_ahead(true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0xa000));
    }

    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}

#[test]
fn branch_lookahead_one_does_not_stage_producer_forwarded_return() {
    let core = live_x1_return_core(1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    assert_eq!(core.next_pending_data_fetch_ahead(true), None);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    assert_eq!(
        state.o3_runtime.producer_forwarded_return_descendant(),
        None
    );
}
