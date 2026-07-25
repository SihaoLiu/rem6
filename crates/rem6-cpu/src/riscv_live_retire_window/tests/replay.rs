use super::*;

#[test]
fn live_younger_selection_matches_oldest_retirement_request_order() {
    let current = MemoryRequestId::new(AgentId::new(7), 10);
    let pc = Address::new(0x8004);
    let events = vec![
        completed_fetch(7, 12, pc),
        completed_fetch(8, 11, pc),
        completed_fetch(7, 9, pc),
        completed_fetch(7, 11, pc),
    ];
    let mut executed = BTreeSet::new();

    let selected = oldest_completed_fetch_at(&executed, &events, current, pc).unwrap();
    assert_eq!(selected.request_id().sequence(), 11);
    assert_eq!(selected.request_id().agent(), current.agent());

    executed.insert(selected.request_id());
    let next = oldest_completed_fetch_at(&executed, &events, current, pc).unwrap();
    assert_eq!(next.request_id().sequence(), 12);
}

#[test]
fn live_younger_selection_assembles_split_word_fetch() {
    let current = MemoryRequestId::new(AgentId::new(7), 10);
    let pc = Address::new(0x800e);
    let raw = 0x0090_0213_u32;
    let bytes = raw.to_le_bytes();
    let events = vec![
        completed_fetch_with_data(7, 11, pc, bytes[..2].to_vec()),
        completed_fetch_with_data(7, 12, Address::new(pc.get() + 2), bytes[2..].to_vec()),
    ];

    let selected =
        completed_fetch_instruction_from_events(&BTreeSet::new(), &events, current, pc).unwrap();

    assert_eq!(selected.consumed_requests[0].sequence(), 11);
    assert_eq!(
        selected.consumed_requests,
        vec![
            MemoryRequestId::new(AgentId::new(7), 11),
            MemoryRequestId::new(AgentId::new(7), 12),
        ]
    );
    assert_eq!(
        selected.decoded.instruction(),
        RiscvInstruction::decode(raw).unwrap()
    );
    assert_eq!(selected.decoded.bytes(), 4);
}

#[test]
fn live_younger_window_collects_two_contiguous_instructions() {
    let current = MemoryRequestId::new(AgentId::new(7), 10);
    let first_pc = Address::new(0x8004);
    let events = vec![
        completed_fetch_with_data(7, 11, first_pc, 0x0050_0213_u32.to_le_bytes().to_vec()),
        completed_fetch_with_data(
            7,
            12,
            Address::new(0x8008),
            0x00b2_0293_u32.to_le_bytes().to_vec(),
        ),
    ];
    let state = RiscvCoreState::new(0x8000, 0);

    let window = completed_fetch_instruction_window(&state, &events, current, first_pc, 2);

    assert_eq!(window.len(), 2);
    assert_eq!(window[0].pc, Address::new(0x8004));
    assert_eq!(window[1].pc, Address::new(0x8008));
    assert_eq!(window[0].consumed_requests, vec![request(7, 11)]);
    assert_eq!(window[1].consumed_requests, vec![request(7, 12)]);
}

#[test]
fn live_retire_replay_stops_before_return_without_recorded_ras_lineage() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let descendant = i_type(1, 0, 0x0, 7, 0x13);
    let core = test_core();
    {
        let mut core_state = core.core.state.lock().expect("cpu core lock");
        for (sequence, pc, raw) in [
            (0, 0x8000, load),
            (1, 0x8004, call),
            (2, 0x800c, return_jump),
            (3, 0x8008, descendant),
        ] {
            core_state.events.push(completed_fetch_with_data(
                7,
                sequence,
                Address::new(pc),
                raw.to_le_bytes().to_vec(),
            ));
        }
    }
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    let return_sequence = 2;
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .squash_return_address_stack_speculation(return_sequence)
            .unwrap();
    }

    let fetch_events = core.core.fetch_events();
    let state = core.state.lock().expect("riscv core lock");
    let window =
        RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix([Register::new(6).unwrap()], 1, 4)
            .unwrap();
    let (replayed, force_normal_execute) = completed_scalar_integer_younger_window(
        &state,
        &fetch_events,
        request(7, 0),
        Address::new(0x8004),
        window,
        3,
    );

    assert_eq!(force_normal_execute, None);

    assert_eq!(
        replayed
            .iter()
            .map(RiscvCompletedFetchInstruction::pc)
            .collect::<Vec<_>>(),
        [Address::new(0x8004)]
    );
}

#[test]
fn live_retire_replay_marks_rejected_fp_dependency_for_normal_execution() {
    let current = request(7, 10);
    let first_pc = Address::new(0x8004);
    let events = vec![
        completed_fetch_with_data(
            7,
            11,
            first_pc,
            0x0020_8253_u32.to_le_bytes().to_vec(), // fadd.s f4, f1, f2
        ),
        completed_fetch_with_data(
            7,
            12,
            Address::new(0x8008),
            0x1032_02d3_u32.to_le_bytes().to_vec(), // fmul.s f5, f4, f3
        ),
    ];
    let state = RiscvCoreState::new(0x8000, 0);
    let window =
        RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix([Register::new(15).unwrap()], 1, 5)
            .unwrap();

    let (replayed, force_normal_execute) =
        completed_scalar_integer_younger_window(&state, &events, current, first_pc, window, 4);

    assert_eq!(
        replayed
            .iter()
            .map(RiscvCompletedFetchInstruction::pc)
            .collect::<Vec<_>>(),
        [first_pc],
    );
    assert_eq!(force_normal_execute, Some((request(7, 12), 1)));
}

#[test]
fn fu_head_replay_marks_rejected_fp_dependency_for_normal_execution() {
    let current = request(7, 10);
    let events = vec![
        completed_fetch_with_data(
            7,
            10,
            Address::new(0x8000),
            0x0220_c1b3_u32.to_le_bytes().to_vec(), // div x3, x1, x2
        ),
        completed_fetch_with_data(
            7,
            11,
            Address::new(0x8004),
            0x0020_8253_u32.to_le_bytes().to_vec(), // fadd.s f4, f1, f2
        ),
        completed_fetch_with_data(
            7,
            12,
            Address::new(0x8008),
            0x1032_02d3_u32.to_le_bytes().to_vec(), // fmul.s f5, f4, f3
        ),
    ];
    let mut state = RiscvCoreState::new(0x8000, 0);
    state
        .live_retire_gate
        .set_policy(RiscvLiveRetireGatePolicy::detailed());

    stage_o3_live_retire_window(
        &mut state,
        current,
        Address::new(0x8000),
        0x0220_c1b3, // div x3, x1, x2
        10,
        29,
        &events,
    )
    .unwrap();

    assert_eq!(
        state.o3_force_normal_execute_fetches,
        [request(7, 12)].into_iter().collect(),
    );
    assert_eq!(
        state
            .o3_runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)],
    );
}
