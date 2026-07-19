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

fn float_load_event_at(pc: u64, sequence: u64, address: u64) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        float_load_instruction(),
        MemoryAccessKind::FloatLoad {
            rd: freg(3),
            address,
            width: MemoryWidth::Doubleword,
        },
    )
}

fn stage_result(
    runtime: &mut O3RuntimeState,
    event: &RiscvCpuExecutionEvent,
    request_sequence: u64,
    issue_tick: u64,
) -> bool {
    runtime.stage_live_data_access_issue(
        event,
        request(request_sequence),
        issue_tick,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    )
}

#[test]
fn memory_result_window_stages_two_read_results_and_two_scalar_rows() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let first = float_load_event(0x8000, 1);
    let second = vector_unit_event(0x8004, 2, 0x9010, None);

    assert!(stage_result(&mut runtime, &first, 20, 31));
    assert!(stage_result(&mut runtime, &second, 21, 32));
    assert_eq!(runtime.live_data_accesses.len(), 2);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            second.fetch().request_id(),
            [
                (Address::new(0x8008), div_x20()),
                (Address::new(0x800c), addi(21, 20)),
            ],
        ),
        2
    );
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);
}

#[test]
fn memory_result_pair_scalar_suffix_tracks_either_integer_result() {
    for (label, first, second, dependency) in [
        (
            "older atomic result",
            atomic_event(0x8000, 1, 11),
            float_load_event_at(0x8004, 2, 0x9010),
            11,
        ),
        (
            "younger scalar-load result",
            vector_unit_event(0x8000, 1, 0x9000, None),
            load_event(0x8004, 2, 13),
            13,
        ),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        assert!(stage_result(&mut runtime, &first, 20, 31), "{label}");
        assert!(stage_result(&mut runtime, &second, 21, 32), "{label}");
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                second.fetch().request_id(),
                [
                    (Address::new(0x8008), div_x20()),
                    (Address::new(0x800c), addi(21, dependency)),
                    (Address::new(0x8010), addi(22, 21)),
                ],
            ),
            2,
            "{label}"
        );
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 4, "{label}");
    }
}

#[test]
fn memory_result_window_rejects_third_side_effecting_or_late_second_results() {
    let first = float_load_event(0x8000, 1);
    let second = load_event(0x8004, 2, 13);

    let mut full = O3RuntimeState::default();
    full.set_scalar_memory_window_limit(4);
    assert!(stage_result(&mut full, &first, 20, 31));
    assert!(stage_result(&mut full, &second, 21, 32));
    assert!(!stage_result(
        &mut full,
        &vector_unit_event(0x8008, 3, 0x9020, None),
        22,
        33,
    ));
    assert!(!stage_result(
        &mut full,
        &atomic_event(0x8008, 3, 15),
        23,
        34,
    ));

    let mut overlapping_atomic = O3RuntimeState::default();
    overlapping_atomic.set_scalar_memory_window_limit(4);
    assert!(stage_result(
        &mut overlapping_atomic,
        &atomic_event(0x8000, 1, 11),
        20,
        31,
    ));
    assert!(!stage_result(
        &mut overlapping_atomic,
        &float_load_event_at(0x8004, 2, 0x9000),
        21,
        32,
    ));

    let mut after_scalar = O3RuntimeState::default();
    after_scalar.set_scalar_memory_window_limit(4);
    assert!(stage_result(&mut after_scalar, &first, 20, 31));
    assert_eq!(
        after_scalar.stage_live_data_access_younger_window(
            first.fetch().request_id(),
            [(Address::new(0x8004), div_x20())],
        ),
        1
    );
    assert!(!stage_result(&mut after_scalar, &second, 21, 32));
}

#[test]
fn younger_result_completion_waits_for_older_result_publication() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let first = float_load_event(0x8000, 1);
    let second = load_event(0x8004, 2, 13);
    assert!(stage_result(&mut runtime, &first, 20, 31));
    assert!(stage_result(&mut runtime, &second, 21, 32));

    let mut second_completed = second.clone();
    second_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &second_completed,
            request(21),
            40,
            8,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert!(runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_none());

    let mut first_completed = first.clone();
    first_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &first_completed,
            request(20),
            45,
            14,
            Some(&3.5_f64.to_bits().to_le_bytes()),
        )
        .unwrap());
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(first_completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&first_completed, true);
    assert_eq!(
        runtime.take_ready_live_data_access_event(u64::MAX),
        Some(second_completed.clone())
    );
    runtime.record_retired_instruction_with_trace(&second_completed, true);
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
}

#[test]
fn older_result_retry_discards_the_younger_result_and_scalar_suffix() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let first = float_load_event(0x8000, 1);
    let second = load_event(0x8004, 2, 13);
    assert!(stage_result(&mut runtime, &first, 20, 31));
    assert!(stage_result(&mut runtime, &second, 21, 32));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            second.fetch().request_id(),
            [
                (Address::new(0x8008), div_x20()),
                (Address::new(0x800c), addi(21, 13)),
            ],
        ),
        2
    );
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);

    let mut retry = first.clone();
    retry.set_data_access_event_kind(RiscvDataAccessEventKind::Retry);
    assert!(runtime
        .complete_live_data_access_response(&retry, request(20), 40, 9, None)
        .unwrap());
    assert_eq!(runtime.live_data_accesses.len(), 1);
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.writeback_reservations().is_empty());
}

#[test]
fn older_result_failure_discards_an_already_admitted_younger_writeback() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let first = float_load_event(0x8000, 1);
    let second = load_event(0x8004, 2, 13);
    assert!(stage_result(&mut runtime, &first, 20, 31));
    assert!(stage_result(&mut runtime, &second, 21, 32));

    let mut second_completed = second.clone();
    second_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &second_completed,
            request(21),
            40,
            8,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    assert_eq!(runtime.writeback_reservations().len(), 1);

    let mut first_failed = first.clone();
    first_failed.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);
    assert!(runtime
        .complete_live_data_access_response(&first_failed, request(20), 50, 19, None)
        .unwrap());

    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.writeback_reservations().is_empty());
}

#[test]
fn younger_result_retry_or_failure_preserves_only_the_older_rows_until_retirement() {
    for kind in [
        RiscvDataAccessEventKind::Retry,
        RiscvDataAccessEventKind::Failed,
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let first = float_load_event(0x8000, 1);
        let second = load_event(0x8004, 2, 13);
        assert!(stage_result(&mut runtime, &first, 20, 31));
        assert!(stage_result(&mut runtime, &second, 21, 32));
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                second.fetch().request_id(),
                [
                    (Address::new(0x8008), div_x20()),
                    (Address::new(0x800c), addi(21, 13)),
                ],
            ),
            2
        );

        let mut younger_outcome = second.clone();
        younger_outcome.set_data_access_event_kind(kind);
        assert!(runtime
            .complete_live_data_access_response(&younger_outcome, request(21), 40, 8, None)
            .unwrap());
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
        assert_eq!(runtime.live_data_accesses.len(), 2);

        let mut first_completed = first.clone();
        first_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime
            .complete_live_data_access_response(
                &first_completed,
                request(20),
                45,
                14,
                Some(&3.5_f64.to_bits().to_le_bytes()),
            )
            .unwrap());
        assert_eq!(
            runtime.take_ready_live_data_access_event(u64::MAX),
            Some(first_completed.clone())
        );
        runtime.record_retired_instruction_with_trace(&first_completed, true);
        assert_eq!(
            runtime.take_ready_live_data_access_event(u64::MAX),
            Some(younger_outcome.clone())
        );
        runtime.record_retired_instruction_with_trace(&younger_outcome, true);
        assert!(runtime.live_data_access_lifecycle_is_quiescent());
    }
}
