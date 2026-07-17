use super::*;

fn div_x20() -> RiscvInstruction {
    RiscvInstruction::Div {
        rd: reg(20),
        rs1: reg(1),
        rs2: reg(2),
    }
}

fn decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::Addi { rd, rs1, imm } => {
            i_type(imm.value(), rs1.index(), 0, rd.index(), 0x13)
        }
        RiscvInstruction::Div { rd, rs1, rs2 } => {
            r_type(1, rs2.index(), rs1.index(), 4, rd.index(), 0x33)
        }
        RiscvInstruction::Beq { rs1, rs2, offset } => {
            let imm = offset.value() as u32;
            ((imm >> 12) & 0x1) << 31
                | ((imm >> 5) & 0x3f) << 25
                | u32::from(rs2.index()) << 20
                | u32::from(rs1.index()) << 15
                | ((imm >> 1) & 0xf) << 8
                | ((imm >> 11) & 0x1) << 7
                | 0x63
        }
        _ => panic!("unsupported younger-window instruction {instruction:?}"),
    };
    RiscvInstruction::decode_with_length(raw).expect("younger-window instruction decodes")
}

fn independent_branch() -> RiscvInstruction {
    RiscvInstruction::Beq {
        rs1: reg(1),
        rs2: reg(2),
        offset: Immediate::new(8),
    }
}

fn younger_request(
    pc: u64,
    consumed_request: MemoryRequestId,
    instruction: RiscvInstruction,
) -> O3LiveIssueRequest {
    O3LiveIssueRequest::new(
        Address::new(pc),
        vec![consumed_request],
        decoded(instruction),
    )
}

fn stage_supported_result_window(
    label: &str,
    event: RiscvCpuExecutionEvent,
    integer_destination: Option<u8>,
    younger_window_policy: O3DataAccessWindowPolicy,
) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(
        runtime.stage_live_data_access_issue(&event, request(20), 31, younger_window_policy,),
        "{label} result stages"
    );

    let div = div_x20();
    let witness = integer_destination.map(|destination| dependent_instruction(21, destination));
    let mut younger = vec![(Address::new(0x8004), div)];
    if let Some(witness) = witness {
        younger.push((Address::new(0x8008), witness));
    }
    let staged = runtime
        .stage_live_data_access_younger_window(event.fetch().request_id(), younger.iter().copied());
    assert_eq!(staged, younger.len(), "{label} bounded younger rows");

    let head = runtime
        .live_data_access_head_reservation(event.fetch().request_id())
        .expect("supported result owns a live data-access reservation");
    let mut issue_requests = vec![younger_request(0x8004, request(30), div)];
    if let Some(witness) = witness {
        issue_requests.push(younger_request(0x8008, request(31), witness));
    }
    runtime
        .schedule_live_speculative_issues(&RiscvHartState::new(0x8000), head, 31, &issue_requests)
        .expect("supported result schedules its independent fixed-latency row");

    let div_sequence = sequence_for_pc(&runtime, 0x8004);
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.sequence == div_sequence));
    assert!(runtime.writeback_reservation(div_sequence).is_some());
    if witness.is_some() {
        let witness_sequence = sequence_for_pc(&runtime, 0x8008);
        assert!(runtime
            .live_speculative_executions
            .iter()
            .all(|issued| issued.sequence != witness_sequence));
        assert!(runtime.writeback_reservation(witness_sequence).is_none());
    }
    runtime
}

#[test]
fn supported_memory_results_stage_real_independent_younger_div_owner() {
    let cases = [
        ("float load", float_load_event(0x8000, 1), None),
        (
            "load reserved",
            load_reserved_event(0x8000, 1, 7, 0x9000),
            Some(7),
        ),
        ("atomic", atomic_event(0x8000, 1, 11), Some(11)),
        (
            "restricted vector e64 m1",
            vector_unit_event(
                0x8000,
                1,
                0x9000,
                Some([vec![false; 8], vec![true; 8]].concat()),
            ),
            None,
        ),
        ("scalar MMIO load", load_event(0x8000, 1, 12), Some(12)),
    ];

    for (label, event, integer_destination) in cases {
        stage_supported_result_window(
            label,
            event,
            integer_destination,
            O3DataAccessWindowPolicy::MemoryResult,
        );
    }
}

#[test]
fn integer_result_dependents_wait_for_result_admission() {
    for (label, event, destination) in [
        (
            "load reserved",
            load_reserved_event(0x8000, 1, 7, 0x9000),
            7,
        ),
        ("atomic", atomic_event(0x8000, 1, 11), 11),
        ("scalar MMIO load", load_event(0x8000, 1, 12), 12),
    ] {
        let mut runtime = stage_supported_result_window(
            label,
            event.clone(),
            Some(destination),
            O3DataAccessWindowPolicy::MemoryResult,
        );
        let witness_sequence = sequence_for_pc(&runtime, 0x8008);
        assert!(runtime
            .live_speculative_executions
            .iter()
            .all(|issued| issued.sequence != witness_sequence));
        assert!(runtime.writeback_reservation(witness_sequence).is_none());

        let mut completed = event.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime
            .complete_live_data_access_response(
                &completed,
                request(20),
                41,
                10,
                Some(&42_u64.to_le_bytes()),
            )
            .unwrap());
        let admitted = runtime.live_data_accesses[0]
            .admitted_writeback_tick
            .expect("integer result reserves an admission tick");
        assert!(runtime
            .take_ready_live_data_access_event(admitted.saturating_sub(1))
            .is_none());
        assert!(runtime
            .live_speculative_issue_candidate(
                Address::new(0x8008),
                dependent_instruction(21, destination),
            )
            .is_none());
        assert!(runtime
            .take_ready_live_data_access_event(admitted)
            .is_some());
        let head = runtime
            .live_data_access_head_reservation(event.fetch().request_id())
            .expect("admitted result retains its live head reservation");
        runtime
            .schedule_live_speculative_issues(
                &RiscvHartState::new(0x8000),
                head,
                admitted,
                &[younger_request(
                    0x8008,
                    request(31),
                    dependent_instruction(21, destination),
                )],
            )
            .expect("admitted integer result wakes its dependent witness");
        assert!(runtime
            .live_speculative_executions
            .iter()
            .any(|issued| issued.sequence == witness_sequence));
        assert!(runtime.writeback_reservation(witness_sequence).is_some());
    }
}

#[test]
fn result_window_rejects_second_data_access_and_unsupported_shapes() {
    for (label, event) in [
        ("float load", float_load_event(0x8000, 1)),
        ("load reserved", load_reserved_event(0x8000, 1, 7, 0x9000)),
        ("atomic", atomic_event(0x8000, 1, 11)),
        (
            "restricted vector e64 m1",
            vector_unit_event(0x8000, 1, 0x9000, None),
        ),
    ] {
        let mut runtime = O3RuntimeState::default();
        assert!(
            runtime.stage_live_data_access_issue_for_test(&event, request(20), 31),
            "{label}"
        );
        assert!(
            !runtime.stage_live_data_access_issue_for_test(
                &load_event(0x8004, 2, 13),
                request(21),
                32
            ),
            "{label} remains terminal for a second data access"
        );
    }

    let mut x0 = O3RuntimeState::default();
    let x0_load = load_event(0x8000, 1, 0);
    assert!(x0.stage_live_data_access_issue_for_test(&x0_load, request(20), 31));
    assert_eq!(
        x0.stage_live_data_access_younger_window(
            x0_load.fetch().request_id(),
            [(Address::new(0x8004), div_x20())],
        ),
        0
    );

    for (label, instruction, access) in unsupported_results() {
        let mut runtime = O3RuntimeState::default();
        let event = execution_event(0x8000, 1, instruction, access);
        assert!(
            !runtime.stage_live_data_access_issue_for_test(&event, request(20), 31),
            "{label}"
        );
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                event.fetch().request_id(),
                [(Address::new(0x8004), div_x20())],
            ),
            0,
            "{label}"
        );
    }
}

#[test]
fn scalar_load_result_window_rejects_terminal_control_and_second_data_access() {
    let load = load_event(0x8000, 1, 12);

    let mut control = O3RuntimeState::default();
    control.set_scalar_memory_window_limit(4);
    assert!(control.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResult,
    ));
    assert!(control.scalar_memory_window_state().is_none());
    assert!(!control.can_stage_scalar_memory(&load_event(0x8004, 2, 13)));
    assert_eq!(
        control.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), independent_branch())],
        ),
        0
    );

    let mut second_data = O3RuntimeState::default();
    second_data.set_scalar_memory_window_limit(4);
    assert!(second_data.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResult,
    ));
    assert!(!second_data.stage_live_data_access_issue_for_test(
        &load_event(0x8004, 2, 13),
        request(21),
        32,
    ));
}

#[test]
fn live_data_access_younger_window_policy_is_stored_and_mismatches_fail_closed() {
    let load = load_event(0x8000, 1, 12);
    let mut result = O3RuntimeState::default();
    assert!(result.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResult,
    ));
    assert_eq!(
        result.live_data_accesses[0].younger_window_policy,
        O3DataAccessWindowPolicy::MemoryResult
    );

    let mut scalar = O3RuntimeState::default();
    assert!(scalar.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix,
    ));
    assert_eq!(
        scalar.live_data_accesses[0].younger_window_policy,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix
    );

    for (label, event, policy) in [
        (
            "supported result cannot suppress its policy",
            load_event(0x8000, 1, 12),
            O3DataAccessWindowPolicy::None,
        ),
        (
            "float result cannot claim scalar-prefix semantics",
            float_load_event(0x8000, 1),
            O3DataAccessWindowPolicy::ScalarMemoryPrefix,
        ),
        (
            "x0 load cannot claim result semantics",
            load_event(0x8000, 1, 0),
            O3DataAccessWindowPolicy::MemoryResult,
        ),
    ] {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.defer_live_data_access_execution(&event));
        assert!(
            !runtime.stage_live_data_access_issue(&event, request(20), 31, policy),
            "{label}"
        );
        assert_eq!(
            runtime.deferred_live_data_access_execution,
            Some(event.fetch().request_id()),
            "{label}"
        );
        assert!(runtime.live_data_accesses.is_empty(), "{label}");
        assert!(runtime.snapshot().reorder_buffer().is_empty(), "{label}");
        assert!(runtime.snapshot().load_store_queue().is_empty(), "{label}");
    }
}

#[test]
fn scalar_load_window_and_non_scalar_handoff_contracts_remain_unchanged() {
    let scalar = stage_supported_result_window(
        "cacheable scalar load",
        load_event(0x8000, 1, 5),
        Some(5),
        O3DataAccessWindowPolicy::ScalarMemoryPrefix,
    );
    assert_eq!(scalar.snapshot().load_store_queue().len(), 1);

    let result = stage_supported_result_window(
        "float load",
        float_load_event(0x8000, 1),
        None,
        O3DataAccessWindowPolicy::MemoryResult,
    );
    assert!(result.live_scalar_memory_handoff().is_none());
    assert!(!result.live_data_access_lifecycle_is_quiescent());
    let core = core_with_runtime(result);
    assert_eq!(
        core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Rejected
    );
}
