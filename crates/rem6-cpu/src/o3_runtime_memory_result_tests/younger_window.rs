use super::*;

fn div_x20() -> RiscvInstruction {
    RiscvInstruction::Div {
        rd: reg(20),
        rs1: reg(1),
        rs2: reg(2),
    }
}

fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(1),
    }
}

fn assert_terminal_result(label: &str, event: RiscvCpuExecutionEvent) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(
        runtime.stage_live_data_access_issue(
            &event,
            request(20),
            31,
            O3DataAccessWindowPolicy::None
        ),
        "{label} result stages"
    );
    assert_eq!(
        runtime.live_data_accesses[0].younger_window_policy,
        O3DataAccessWindowPolicy::None,
        "{label} is terminal"
    );
    assert!(runtime
        .data_access_integer_window(event.fetch().request_id())
        .is_none());
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            event.fetch().request_id(),
            [(Address::new(0x8004), div_x20())],
        ),
        0,
        "{label} cannot stage an independent fixed-FU suffix"
    );
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.writeback_reservations().is_empty());
}

#[test]
fn supported_memory_results_are_terminal_without_younger_speculation() {
    for (label, event) in [
        ("float load", float_load_event(0x8000, 1)),
        ("load reserved", load_reserved_event(0x8000, 1, 7, 0x9000)),
        ("atomic", atomic_event(0x8000, 1, 11)),
        (
            "restricted vector e64 m1",
            vector_unit_event(
                0x8000,
                1,
                0x9000,
                Some([vec![false; 8], vec![true; 8]].concat()),
            ),
        ),
        ("scalar MMIO load", load_event(0x8000, 1, 12)),
    ] {
        assert_terminal_result(label, event);
    }
}

#[test]
fn result_scalar_suffix_admits_independent_rows_without_an_integer_result() {
    for (label, event) in [
        ("float load", float_load_event(0x8000, 1)),
        (
            "restricted vector e64 m1",
            vector_unit_event(
                0x8000,
                1,
                0x9000,
                Some([vec![false; 8], vec![true; 8]].concat()),
            ),
        ),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        assert!(runtime.stage_live_data_access_issue(
            &event,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                event.fetch().request_id(),
                [
                    (Address::new(0x8004), div_x20()),
                    (Address::new(0x8008), addi(21, 20)),
                    (Address::new(0x800c), addi(22, 21)),
                ],
            ),
            3,
            "{label}"
        );
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 4, "{label}");
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1, "{label}");
    }
}

#[test]
fn integer_result_scalar_suffix_stops_at_the_exact_result_consumer() {
    for (label, event, result_register, lsq_rows) in [
        (
            "load reserved",
            load_reserved_event(0x8000, 1, 7, 0x9000),
            7,
            1,
        ),
        ("atomic", atomic_event(0x8000, 1, 11), 11, 2),
        ("scalar MMIO load", load_event(0x8000, 1, 12), 12, 1),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        assert!(runtime.stage_live_data_access_issue(
            &event,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                event.fetch().request_id(),
                [
                    (Address::new(0x8004), div_x20()),
                    (Address::new(0x8008), addi(21, 20)),
                    (Address::new(0x800c), addi(22, result_register)),
                    (Address::new(0x8010), addi(23, 22)),
                ],
            ),
            3,
            "{label}"
        );
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 4, "{label}");
        assert_eq!(
            runtime.snapshot().load_store_queue().len(),
            lsq_rows,
            "{label}"
        );
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

    for (label, event) in [
        ("float load", float_load_event(0x8000, 1)),
        ("load reserved", load_reserved_event(0x8000, 1, 7, 0x9000)),
        ("atomic", atomic_event(0x8000, 1, 11)),
    ] {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.stage_live_data_access_issue(
            &event,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        assert!(
            !runtime.stage_live_data_access_issue_for_test(
                &load_event(0x8004, 2, 13),
                request(21),
                32
            ),
            "{label} suffix still rejects a second data access"
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
fn scalar_load_prefix_remains_the_bounded_younger_lane() {
    let load = load_event(0x8000, 1, 12);
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix,
    ));
    assert!(runtime
        .data_access_integer_window(load.fetch().request_id())
        .is_some());
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), div_x20())],
        ),
        1
    );
}

#[test]
fn live_data_access_younger_window_policy_is_stored_and_mismatches_fail_closed() {
    let load = load_event(0x8000, 1, 12);
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
            "float result cannot claim scalar-prefix semantics",
            float_load_event(0x8000, 1),
            O3DataAccessWindowPolicy::ScalarMemoryPrefix,
        ),
        (
            "x0 load cannot claim scalar-prefix semantics",
            load_event(0x8000, 1, 0),
            O3DataAccessWindowPolicy::ScalarMemoryPrefix,
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
    let mut scalar = O3RuntimeState::default();
    let scalar_load = load_event(0x8000, 1, 5);
    assert!(scalar.stage_live_data_access_issue(
        &scalar_load,
        request(20),
        31,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix,
    ));
    assert_eq!(
        scalar.stage_live_data_access_younger_window(
            scalar_load.fetch().request_id(),
            [(Address::new(0x8004), div_x20())],
        ),
        1
    );
    assert_eq!(scalar.snapshot().load_store_queue().len(), 1);

    let mut result = O3RuntimeState::default();
    let float = float_load_event(0x8000, 1);
    assert!(result.stage_live_data_access_issue(
        &float,
        request(20),
        31,
        O3DataAccessWindowPolicy::None,
    ));
    assert!(result.live_scalar_memory_handoff().is_none());
    assert!(!result.live_data_access_lifecycle_is_quiescent());
    let core = core_with_runtime(result);
    assert_eq!(
        core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Rejected
    );
}
