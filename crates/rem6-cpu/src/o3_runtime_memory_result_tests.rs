use super::o3_runtime_memory::*;
use super::*;

use rem6_isa_riscv::{
    AtomicMemoryOp, FloatRegister, Immediate, MemoryWidth, Register, RegisterWrite,
    RiscvDecodedInstruction, RiscvExecutionRecord, RiscvHartState, RiscvInstruction,
    RiscvVectorMaskMode, RiscvVectorMemoryInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId, MemoryResponse,
    TranslationPageMap, TranslationPagePermissions, TranslationPageSize, TranslationQueueConfig,
    TranslationTlbConfig,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
};

use crate::riscv_execution_mode_handoff::RiscvO3LiveDataHandoffCapture;
use crate::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    CpuTranslationFrontend, RiscvCore, RiscvLoadReservation,
};

#[path = "o3_runtime_memory_result_tests/replan.rs"]
mod replan;
#[path = "o3_runtime_memory_result_tests/store_conditional.rs"]
mod store_conditional;
#[path = "o3_runtime_memory_result_tests/writeback_maxima.rs"]
mod writeback_maxima;
#[path = "o3_runtime_memory_result_tests/younger_window.rs"]
mod younger_window;

#[test]
fn memory_result_policy_accepts_exact_one_destination_matrix() {
    for (label, instruction, access, class, architectural, lsq_rows) in supported_results() {
        let mut runtime = O3RuntimeState::default();
        let event = execution_event(0x8000, 1, instruction, access);

        assert!(
            runtime.stage_live_data_access_issue_for_test(&event, request(20), 31),
            "{label}"
        );

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.reorder_buffer().len(), 1, "{label}");
        assert_eq!(snapshot.load_store_queue().len(), lsq_rows, "{label}");
        let rob = snapshot.reorder_buffer()[0];
        let rename = staged_rename_entry(rob).expect("supported result has one staged rename");
        assert_eq!(rename.register_class(), class, "{label}");
        assert_eq!(rename.architectural(), architectural, "{label}");
        assert_eq!(Some(rename.physical()), rob.destination(), "{label}");
        assert_eq!(
            snapshot
                .rename_map()
                .iter()
                .filter(|entry| {
                    entry.register_class() == class && entry.architectural() == architectural
                })
                .copied()
                .collect::<Vec<_>>(),
            vec![rename],
            "{label}"
        );
    }
}

#[test]
fn memory_result_policy_rejects_zero_destination_and_unsupported_shapes() {
    let mut scalar_x0 = O3RuntimeState::default();
    let x0_load = load_event(0x8000, 1, 0);
    assert!(scalar_x0.stage_live_data_access_issue_for_test(&x0_load, request(20), 31));
    assert_eq!(scalar_x0.snapshot().reorder_buffer()[0].destination(), None);
    let mut completed_x0 = x0_load;
    completed_x0.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(
        scalar_x0
            .complete_live_data_access_response(
                &completed_x0,
                request(20),
                41,
                10,
                Some(&[1, 0, 0, 0]),
            )
            .unwrap()
    );
    assert_eq!(
        scalar_x0.live_data_accesses[0].admitted_writeback_tick,
        None
    );

    for (label, instruction, access) in unsupported_results() {
        let mut runtime = O3RuntimeState::default();
        let event = execution_event(0x8000, 1, instruction, access);

        assert!(
            !runtime.stage_live_data_access_issue_for_test(&event, request(20), 31),
            "{label}"
        );
        assert!(runtime.snapshot().reorder_buffer().is_empty(), "{label}");
        assert!(runtime.snapshot().load_store_queue().is_empty(), "{label}");
        assert!(runtime.live_data_accesses.is_empty(), "{label}");
    }
}

#[test]
fn non_scalar_result_is_terminal_while_scalar_overlap_remains_available() {
    let mut scalar = O3RuntimeState::default();
    scalar.set_scalar_memory_window_limit(4);
    assert!(scalar.stage_live_data_access_issue_for_test(
        &load_event(0x8000, 1, 5),
        request(20),
        31
    ));
    assert!(scalar.stage_live_data_access_issue_for_test(
        &load_event(0x8004, 2, 6),
        request(21),
        32
    ));
    assert_eq!(scalar.live_data_accesses.len(), 2);

    let mut result = O3RuntimeState::default();
    let float = float_load_event(0x8000, 1);
    assert!(result.stage_live_data_access_issue_for_test(&float, request(20), 31));
    assert!(!result.stage_live_data_access_issue_for_test(
        &load_event(0x8004, 2, 6),
        request(21),
        32
    ));
    let mut completed = float;
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(is_terminal_o3_data_access_event(&completed));
}

#[test]
fn live_atomic_reserves_and_retires_two_lsq_sequences() {
    let mut runtime = O3RuntimeState::default();
    let atomic = atomic_event(0x8000, 1, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&atomic, request(20), 31));
    let sequence = runtime.live_data_accesses[0].sequence;
    assert_eq!(sequence, 0);
    assert_eq!(lsq_sequences(&runtime), vec![sequence, sequence + 1]);

    let mut completed = atomic.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            41,
            10,
            Some(&0x1122_3344_5566_7788_u64.to_le_bytes()),
        )
        .unwrap());
    assert!(runtime.writeback_reservation(sequence).is_some());
    assert!(runtime.take_ready_live_data_access_event(42).is_some());
    runtime.record_retired_instruction_with_trace(&completed, true);
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.snapshot().rename_map().iter().any(|entry| {
        entry.register_class() == O3RegisterClass::Integer && entry.architectural() == 7
    }));

    let next = load_event(0x8004, 2, 8);
    assert!(runtime.stage_live_data_access_issue_for_test(&next, request(21), 43));
    assert_eq!(runtime.live_data_accesses[0].sequence, sequence + 2);
}

#[test]
fn memory_result_retry_and_failure_discard_reservation_and_full_lsq_span() {
    for kind in [
        RiscvDataAccessEventKind::Retry,
        RiscvDataAccessEventKind::Failed,
    ] {
        let mut runtime = O3RuntimeState::default();
        let atomic = atomic_event(0x8000, 1, 7);
        assert!(runtime.stage_live_data_access_issue_for_test(&atomic, request(20), 31));
        let sequence = runtime.live_data_accesses[0].sequence;
        runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(sequence, 50)])
            .unwrap();
        let mut terminal = atomic.clone();
        terminal.set_data_access_event_kind(kind);

        assert!(runtime
            .complete_live_data_access_response(&terminal, request(20), 41, 10, None)
            .unwrap());
        assert!(runtime.writeback_reservation(sequence).is_none());
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(
            runtime.live_data_accesses[0].outcome,
            match kind {
                RiscvDataAccessEventKind::Retry => O3LiveDataAccessOutcome::Retried,
                RiscvDataAccessEventKind::Failed => O3LiveDataAccessOutcome::Failed,
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn live_atomic_squash_redirect_and_rollback_remove_both_lsq_rows() {
    let mut squashed = live_atomic_runtime();
    squashed.discard_live_staged_instructions();
    assert!(squashed.snapshot().load_store_queue().is_empty());

    let redirected = core_with_runtime(live_atomic_runtime());
    redirected.redirect_pc(Address::new(0xa000));
    assert!(redirected
        .o3_runtime_snapshot()
        .load_store_queue()
        .is_empty());

    let mut rolled_back = live_atomic_runtime();
    rolled_back
        .restore_checkpoint_payload(O3RuntimeState::default().checkpoint_payload())
        .unwrap();
    assert!(rolled_back.snapshot().load_store_queue().is_empty());
}

#[test]
fn live_non_scalar_result_is_rejected_by_handoff_status_and_remains_nonquiescent() {
    let core = core_with_runtime(O3RuntimeState::default());
    core.with_o3_runtime(|runtime| {
        assert!(runtime.stage_live_data_access_issue_for_test(
            &float_load_event(0x8000, 1),
            request(20),
            31
        ));
        assert!(runtime.live_scalar_memory_handoff().is_none());
        assert!(!runtime.live_data_access_lifecycle_is_quiescent());
    });

    assert_eq!(
        core.capture_o3_live_data_handoff_status(),
        RiscvO3LiveDataHandoffCapture::Rejected
    );
}

#[test]
fn memory_result_response_waits_for_admitted_writeback_tick() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = data_core(fetch_route, data_route);
    let float = float_load_event(0x8000, 1);
    stage_event(&core, float);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(99, 42)])
            .unwrap();
    }

    let payload = 3.5f64.to_bits();
    issue_data_completion_after(
        &core,
        &mut scheduler,
        &transport,
        36,
        payload.to_le_bytes().to_vec(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.hart.read_float(freg(3)), 0);
        assert_eq!(
            calendar_rows(&state.o3_runtime),
            vec![(0, 42, 0), (99, 43, 0)]
        );
        assert_eq!(
            state.o3_runtime.live_data_accesses[0].response_tick,
            Some(41)
        );
        assert_eq!(
            state.o3_runtime.live_data_accesses[0].raw_ready_tick,
            Some(42)
        );
        assert_eq!(
            state.o3_runtime.live_data_accesses[0].admitted_writeback_tick,
            Some(42)
        );
        assert!(!state.o3_runtime.snapshot().reorder_buffer()[0].is_ready());
    }

    assert!(core
        .record_ready_o3_data_access_event_with_trace(41, true)
        .is_none());
    assert_eq!(core.read_float_register(freg(3)), 0);
    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(!state.o3_runtime.snapshot().reorder_buffer()[0].is_ready());
    }
    let execution = core
        .record_ready_o3_data_access_event_with_trace(42, true)
        .expect("production FLD publication path admits at the reserved tick");
    assert_eq!(execution.fetch().request_id(), request(1));
    assert_eq!(core.read_float_register(freg(3)), payload);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.writeback_reservation(0).is_some());
    assert_eq!(
        state
            .o3_runtime
            .live_writeback_counted_sequences
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![99]
    );
    let trace = state
        .o3_runtime
        .trace_records()
        .last()
        .copied()
        .expect("production FLD publication records a trace");
    assert_eq!(trace.admitted_writeback_tick(), Some(42));
}

#[test]
fn load_reserved_installs_physical_reservation_only_at_admission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = translated_core(fetch_route, data_route);
    let event = load_reserved_event(0x8000, 1, 5, 0x4008);
    stage_event(&core, event);

    issue_translated_completion(
        &core,
        &mut scheduler,
        &transport,
        0x4000,
        0x9000,
        0x1122_3344_5566_7788u64.to_le_bytes().to_vec(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.hart.read(reg(5)), 0);
        assert_eq!(state.reservation, None);
    }

    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some());
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.hart.read(reg(5)), 0x1122_3344_5566_7788);
    assert_eq!(
        state.reservation,
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );
}

#[test]
fn masked_vector_result_applies_at_preserved_nonzero_request_offset() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = translated_core(fetch_route, data_route);
    let event = vector_unit_event(
        0x8000,
        1,
        0x4000,
        Some([vec![false; 8], vec![true; 8]].concat()),
    );
    stage_event(&core, event);
    let payload = 0xaabb_ccdd_eeff_0011u64.to_le_bytes().to_vec();

    issue_translated_completion(
        &core,
        &mut scheduler,
        &transport,
        0x4000,
        0x9000,
        payload.clone(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.hart.read_vector(vreg(2)),
            [0; RISCV_VECTOR_REGISTER_BYTES]
        );
    }

    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some());
    let state = core.state.lock().expect("riscv core lock");
    let vector = state.hart.read_vector(vreg(2));
    assert_eq!(&vector[0..8], &[0; 8]);
    assert_eq!(&vector[8..16], payload.as_slice());
}

#[test]
fn memory_result_collision_uses_older_sequence_and_width_two_exact_fit() {
    let fixed_first_single = fixed_fu_owner_reserved_before_older_memory_result(1);
    assert_eq!(
        calendar_rows(&fixed_first_single.runtime),
        vec![(0, 42, 0), (1, 43, 0)]
    );
    assert_memory_owner(&fixed_first_single.runtime, 0, Some(42), Some(0));
    assert_fixed_owner(&fixed_first_single, 43, Some(0));
    assert!(fixed_first_single
        .runtime
        .live_data_access_publication_is_admitted(42));
    assert_writeback_stats(&fixed_first_single.runtime, 2, 2, 1, 1, 2, 1);

    let fixed_first_double = fixed_fu_owner_reserved_before_older_memory_result(2);
    assert_eq!(
        calendar_rows(&fixed_first_double.runtime),
        vec![(0, 42, 0), (1, 42, 1)]
    );
    assert_memory_owner(&fixed_first_double.runtime, 0, Some(42), Some(0));
    assert_fixed_owner(&fixed_first_double, 42, Some(1));
    assert!(fixed_first_double
        .runtime
        .live_data_access_publication_is_admitted(42));
    assert_writeback_stats(&fixed_first_double.runtime, 1, 2, 0, 0, 2, 0);

    let memory_first_single = memory_result_owner_reserved_before_older_fixed_fu(1);
    assert_eq!(
        calendar_rows(&memory_first_single.runtime),
        vec![(0, 42, 0), (1, 43, 0)]
    );
    assert_fixed_owner(&memory_first_single, 42, Some(0));
    assert_memory_owner(&memory_first_single.runtime, 1, Some(43), Some(0));
    assert!(!memory_first_single
        .runtime
        .live_data_access_publication_is_admitted(42));
    assert!(memory_first_single
        .runtime
        .live_data_access_publication_is_admitted(43));
    assert_writeback_stats(&memory_first_single.runtime, 2, 2, 1, 1, 2, 1);

    let memory_first_double = memory_result_owner_reserved_before_older_fixed_fu(2);
    assert_eq!(
        calendar_rows(&memory_first_double.runtime),
        vec![(0, 42, 0), (1, 42, 1)]
    );
    assert_fixed_owner(&memory_first_double, 42, Some(0));
    assert_memory_owner(&memory_first_double.runtime, 1, Some(42), Some(1));
    assert!(memory_first_double
        .runtime
        .live_data_access_publication_is_admitted(42));
    assert_writeback_stats(&memory_first_double.runtime, 1, 2, 0, 0, 2, 0);
}

#[test]
fn memory_result_replanning_invalidates_dependent_chain_until_authoritative_reissue() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(1));
    runtime.set_writeback_width(1);
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let producer = fixed_instruction(6);
    let child = dependent_instruction(7, 6);
    let grandchild = dependent_instruction(8, 7);
    let producer_sequence = runtime
        .stage_live_retire_window(
            Address::new(0x8004),
            producer,
            0,
            [
                (Address::new(0x8008), child),
                (Address::new(0x800c), grandchild),
            ],
        )
        .expect("fixed-FU producer stages behind older memory result");
    assert_eq!(producer_sequence, 1);
    let child_sequence = sequence_for_pc(&runtime, 0x8008);
    let grandchild_sequence = sequence_for_pc(&runtime, 0x800c);
    let producer_execution = record_fixed_fu_owner(
        &mut runtime,
        producer_sequence,
        producer,
        0x8004,
        request(30),
        42,
    );
    let child_execution =
        record_speculative_owner(&mut runtime, 0x8008, child, request(31), 10, 7, 8);
    let grandchild_execution =
        record_speculative_owner(&mut runtime, 0x800c, grandchild, request(32), 10, 8, 9);
    assert_eq!(
        calendar_rows_with_raw(&runtime),
        vec![(1, 42, 42, 0), (2, 42, 43, 0), (3, 43, 44, 0)]
    );

    let mut completed_older = older.clone();
    completed_older.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_older,
            request(20),
            41,
            10,
            Some(&0x1111_1111u32.to_le_bytes()),
        )
        .unwrap());

    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| issued.sequence != child_sequence && issued.sequence != grandchild_sequence));
    assert!(runtime.writeback_reservation(child_sequence).is_none());
    assert!(runtime.writeback_reservation(grandchild_sequence).is_none());
    let head = runtime
        .live_data_access_head_reservation(older.fetch().request_id())
        .expect("memory head remains available for descendant reissue");
    runtime
        .schedule_live_speculative_issues(
            &RiscvHartState::new(0x8000),
            head,
            43,
            &[
                issue_request(0x8008, request(31), child),
                issue_request(0x800c, request(32), grandchild),
            ],
        )
        .unwrap();

    assert_eq!(
        calendar_rows_with_raw(&runtime),
        vec![
            (0, 42, 42, 0),
            (1, 42, 43, 0),
            (2, 43, 44, 0),
            (3, 44, 45, 0),
        ]
    );
    assert_memory_owner(&runtime, 0, Some(42), Some(0));
    assert_speculative_owner(
        &runtime,
        producer_sequence,
        &[request(30)],
        &producer_execution,
        42,
        42,
        43,
        Some(0),
        &[],
    );
    assert_speculative_owner(
        &runtime,
        child_sequence,
        &[request(31)],
        &child_execution,
        43,
        43,
        44,
        Some(0),
        &[producer_sequence],
    );
    assert_speculative_owner(
        &runtime,
        grandchild_sequence,
        &[request(32)],
        &grandchild_execution,
        44,
        44,
        45,
        Some(0),
        &[child_sequence],
    );
    assert_eq!(
        ready_rows_by_tick(&runtime),
        vec![(42, vec![0, 1]), (43, vec![2]), (44, vec![3])]
    );
    assert!(runtime.live_data_access_publication_is_admitted(42));
    assert_eq!(
        runtime.live_speculative_execution_ready_tick(&[request(30)], &producer_execution),
        Some(43)
    );
    assert_writeback_stats(&runtime, 4, 4, 3, 3, 2, 1);
}

#[test]
fn memory_result_replanning_reverses_provisional_deferred_stats() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_writeback_width(2));
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let other = multiply_instruction(6, 0);
    let producer = fixed_instruction(7);
    let dependent = dependent_instruction(8, 7);
    let other_sequence = runtime
        .stage_live_retire_window(
            Address::new(0x8004),
            other,
            0,
            [
                (Address::new(0x8008), producer),
                (Address::new(0x800c), dependent),
            ],
        )
        .expect("width-two statistics fixture stages");
    let producer_sequence = sequence_for_pc(&runtime, 0x8008);
    let dependent_sequence = sequence_for_pc(&runtime, 0x800c);
    record_fixed_fu_owner(&mut runtime, other_sequence, other, 0x8004, request(30), 40);
    let producer_execution =
        record_speculative_owner(&mut runtime, 0x8008, producer, request(31), 42, 7, 7);
    let dependent_execution =
        record_speculative_owner(&mut runtime, 0x800c, dependent, request(32), 42, 8, 8);
    assert_writeback_stats(&runtime, 2, 3, 1, 1, 3, 1);

    let mut completed_older = older.clone();
    completed_older.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_older,
            request(20),
            41,
            10,
            Some(&0x2222_2222u32.to_le_bytes()),
        )
        .unwrap());
    assert!(runtime.writeback_reservation(dependent_sequence).is_none());

    let head = runtime
        .live_data_access_head_reservation(older.fetch().request_id())
        .expect("memory head remains available for dependent reissue");
    runtime
        .schedule_live_speculative_issues(
            &RiscvHartState::new(0x8000),
            head,
            43,
            &[issue_request(0x800c, request(32), dependent)],
        )
        .unwrap();

    assert_speculative_owner(
        &runtime,
        producer_sequence,
        &[request(31)],
        &producer_execution,
        42,
        42,
        43,
        Some(0),
        &[],
    );
    assert_speculative_owner(
        &runtime,
        dependent_sequence,
        &[request(32)],
        &dependent_execution,
        43,
        43,
        43,
        Some(1),
        &[producer_sequence],
    );
    assert_eq!(
        calendar_rows_with_raw(&runtime),
        vec![
            (0, 42, 42, 0),
            (other_sequence, 42, 42, 1),
            (producer_sequence, 42, 43, 0),
            (dependent_sequence, 43, 43, 1),
        ]
    );
    assert_eq!(runtime.stats().issue_cycles(), 1);
    assert_eq!(runtime.stats().issued_rows(), 1);
    assert_eq!(runtime.stats().resource_blocked_row_cycles(), 0);
    assert_eq!(runtime.stats().dependency_blocked_row_cycles(), 0);
    assert_writeback_stats(&runtime, 2, 4, 1, 1, 3, 1);
}

fn supported_results() -> Vec<(
    &'static str,
    RiscvInstruction,
    MemoryAccessKind,
    O3RegisterClass,
    u32,
    usize,
)> {
    vec![
        (
            "load",
            load_instruction(5),
            load_access(5, 0x9000),
            O3RegisterClass::Integer,
            5,
            1,
        ),
        (
            "load reserved",
            load_reserved_instruction(6),
            load_reserved_access(6, 0x9000),
            O3RegisterClass::Integer,
            6,
            1,
        ),
        (
            "atomic",
            atomic_instruction(7),
            atomic_access(7, 0x9000),
            O3RegisterClass::Integer,
            7,
            2,
        ),
        (
            "store conditional",
            store_conditional_instruction(),
            MemoryAccessKind::StoreConditional {
                rd: reg(7),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                value: 1,
                acquire: false,
                release: false,
            },
            O3RegisterClass::Integer,
            7,
            1,
        ),
        (
            "float load",
            float_load_instruction(),
            MemoryAccessKind::FloatLoad {
                rd: freg(3),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
            },
            O3RegisterClass::FloatingPoint,
            3,
            1,
        ),
        (
            "vector e64 m1 unmasked",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(MemoryWidth::Doubleword, 16, None, 1, false),
            O3RegisterClass::Vector,
            2,
            1,
        ),
        (
            "vector e64 m1 partly active",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(
                MemoryWidth::Doubleword,
                16,
                Some([vec![false; 8], vec![true; 8]].concat()),
                1,
                false,
            ),
            O3RegisterClass::Vector,
            2,
            1,
        ),
    ]
}

fn unsupported_results() -> Vec<(&'static str, RiscvInstruction, MemoryAccessKind)> {
    vec![
        (
            "x0 load reserved",
            load_reserved_instruction(0),
            load_reserved_access(0, 0x9000),
        ),
        ("x0 atomic", atomic_instruction(0), atomic_access(0, 0x9000)),
        (
            "vector word width",
            vector_unit_instruction(MemoryWidth::Word, false),
            vector_unit_access(MemoryWidth::Word, 16, None, 1, false),
        ),
        (
            "vector zero length",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(MemoryWidth::Doubleword, 0, None, 1, false),
        ),
        (
            "vector oversized",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(
                MemoryWidth::Doubleword,
                RISCV_VECTOR_REGISTER_BYTES + 1,
                None,
                1,
                false,
            ),
        ),
        (
            "vector group",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(MemoryWidth::Doubleword, 32, None, 2, false),
        ),
        (
            "vector segment",
            RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadSegmentUnitStride {
                vd: vreg(2),
                rs1: reg(10),
                width: MemoryWidth::Doubleword,
                fields: 2,
                mask: RiscvVectorMaskMode::Unmasked,
            }),
            MemoryAccessKind::VectorLoadSegmentUnitStride {
                vd: vreg(2),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                fields: 2,
                element_count: 1,
                byte_len: 16,
                byte_mask: None,
                group_registers: 1,
            },
        ),
        (
            "vector strided",
            RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadStrided {
                vd: vreg(2),
                rs1: reg(10),
                rs2: reg(11),
                width: MemoryWidth::Doubleword,
                mask: RiscvVectorMaskMode::Unmasked,
            }),
            MemoryAccessKind::VectorLoadStrided {
                vd: vreg(2),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                stride: 16,
                element_count: 2,
                span_len: 24,
                byte_mask: None,
                group_registers: 1,
            },
        ),
        (
            "vector indexed",
            RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadIndexedUnordered {
                vd: vreg(2),
                rs1: reg(10),
                vs2: vreg(4),
                index_width: MemoryWidth::Doubleword,
                mask: RiscvVectorMaskMode::Unmasked,
            }),
            MemoryAccessKind::VectorLoadIndexed {
                vd: vreg(2),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                index_width: MemoryWidth::Doubleword,
                offsets: vec![0, 16],
                span_len: 24,
                byte_mask: None,
                group_registers: 1,
            },
        ),
        (
            "fault only first",
            vector_unit_instruction(MemoryWidth::Doubleword, true),
            vector_unit_access(MemoryWidth::Doubleword, 16, None, 1, true),
        ),
        (
            "all inactive",
            vector_unit_instruction(MemoryWidth::Doubleword, false),
            vector_unit_access(MemoryWidth::Doubleword, 16, Some(vec![false; 16]), 1, false),
        ),
    ]
}

fn live_atomic_runtime() -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.stage_live_data_access_issue_for_test(
        &atomic_event(0x8000, 1, 7),
        request(20),
        31
    ));
    assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
    runtime
}

#[derive(Clone, Debug)]
struct RealOwnerCollision {
    runtime: O3RuntimeState,
    fixed_sequence: u64,
    fixed_request: MemoryRequestId,
    fixed_execution: RiscvExecutionRecord,
}

fn fixed_fu_owner_reserved_before_older_memory_result(width: usize) -> RealOwnerCollision {
    let mut runtime = O3RuntimeState::default();
    runtime.set_writeback_width(width);
    let older = load_event(0x8000, 1, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&older, request(20), 31));
    let fixed = fixed_instruction(6);
    let fixed_sequence = runtime
        .stage_live_retire_window(Address::new(0x8004), fixed, 0, [])
        .expect("younger fixed-FU row stages after live memory result");
    assert_eq!(fixed_sequence, 1);
    let fixed_request = request(30);
    let fixed_execution = record_fixed_fu_owner(
        &mut runtime,
        fixed_sequence,
        fixed,
        0x8004,
        fixed_request,
        42,
    );
    let mut completed_older = older;
    completed_older.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_older,
            request(20),
            41,
            10,
            Some(&0x1111_1111u32.to_le_bytes()),
        )
        .unwrap());

    RealOwnerCollision {
        runtime,
        fixed_sequence,
        fixed_request,
        fixed_execution,
    }
}

fn memory_result_owner_reserved_before_older_fixed_fu(width: usize) -> RealOwnerCollision {
    let mut runtime = O3RuntimeState::default();
    runtime.set_writeback_width(width);
    let fixed = fixed_instruction(6);
    let fixed_sequence = runtime
        .stage_live_retire_window(Address::new(0x8000), fixed, 0, [])
        .expect("older fixed-FU row stages first");
    assert_eq!(fixed_sequence, 0);
    let younger = load_event(0x8004, 2, 5);
    assert!(runtime.stage_live_data_access_issue_for_test(&younger, request(20), 31));
    assert_eq!(runtime.live_data_accesses[0].sequence, 1);
    let mut completed_younger = younger;
    completed_younger.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed_younger,
            request(20),
            41,
            10,
            Some(&0x2222_2222u32.to_le_bytes()),
        )
        .unwrap());
    let fixed_request = request(30);
    let fixed_execution = record_fixed_fu_owner(
        &mut runtime,
        fixed_sequence,
        fixed,
        0x8000,
        fixed_request,
        42,
    );

    RealOwnerCollision {
        runtime,
        fixed_sequence,
        fixed_request,
        fixed_execution,
    }
}

fn fixed_instruction(rd: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(0),
        imm: Immediate::new(7),
    }
}

fn dependent_instruction(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(1),
    }
}

fn multiply_instruction(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Mul {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(0),
    }
}

fn issue_request(
    pc: u64,
    consumed_request: MemoryRequestId,
    instruction: RiscvInstruction,
) -> O3LiveIssueRequest {
    O3LiveIssueRequest::new(
        Address::new(pc),
        vec![consumed_request],
        decoded_instruction(instruction),
    )
}

fn decoded_instruction(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::Addi { rd, rs1, imm } => {
            i_type(imm.value(), rs1.index(), 0x0, rd.index(), 0x13)
        }
        RiscvInstruction::Mul { rd, rs1, rs2 } => {
            r_type(1, rs2.index(), rs1.index(), 0x0, rd.index(), 0x33)
        }
        _ => panic!("unsupported issue-request instruction {instruction:?}"),
    };
    RiscvInstruction::decode_with_length(raw).expect("issue-request instruction decodes")
}

fn i_type(imm: i64, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32 & 0xfff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn record_fixed_fu_owner(
    runtime: &mut O3RuntimeState,
    sequence: u64,
    instruction: RiscvInstruction,
    pc: u64,
    fetch_request: MemoryRequestId,
    issue_tick: u64,
) -> RiscvExecutionRecord {
    let execution = RiscvExecutionRecord::new(
        instruction,
        pc,
        pc + 4,
        vec![RegisterWrite::new(reg(6), 7)],
        None,
    );
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, issue_tick, instruction);
    assert!(runtime
        .record_live_issue_head_execution(head, &[fetch_request], execution.clone())
        .unwrap());
    execution
}

fn record_speculative_owner(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    fetch_request: MemoryRequestId,
    earliest_issue_tick: u64,
    rd: u8,
    value: u64,
) -> RiscvExecutionRecord {
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(pc), instruction)
        .expect("dependent speculative issue candidate is available");
    let execution = RiscvExecutionRecord::new(
        instruction,
        pc,
        pc + 4,
        vec![RegisterWrite::new(reg(rd), value)],
        None,
    );
    assert!(runtime
        .record_live_speculative_execution(
            candidate,
            &[fetch_request],
            earliest_issue_tick,
            execution.clone(),
        )
        .unwrap());
    execution
}

fn sequence_for_pc(runtime: &O3RuntimeState, pc: u64) -> u64 {
    runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(pc))
        .map(|entry| entry.sequence())
        .expect("live ROB row exists for pc")
}

fn assert_fixed_owner(collision: &RealOwnerCollision, admitted_tick: u64, slot: Option<usize>) {
    let reservation = collision
        .runtime
        .writeback_reservation(collision.fixed_sequence)
        .expect("fixed-FU writeback reservation exists");
    assert_eq!(reservation.admitted_tick(), admitted_tick);
    assert_eq!(Some(reservation.slot()), slot);
    let rob = collision
        .runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.sequence() == collision.fixed_sequence)
        .copied()
        .expect("fixed-FU ROB owner exists");
    assert!(rob.is_ready());
    assert_eq!(rob.ready_tick(), admitted_tick);
    let issued = collision
        .runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == collision.fixed_sequence)
        .expect("fixed-FU speculative owner exists");
    assert_eq!(issued.raw_ready_tick, 42);
    assert_eq!(issued.admitted_writeback_tick, admitted_tick);
    assert_eq!(issued.writeback_slot, slot);
    assert_eq!(
        collision.runtime.live_speculative_execution_ready_tick(
            &[collision.fixed_request],
            &collision.fixed_execution,
        ),
        Some(admitted_tick)
    );
}

fn assert_speculative_owner(
    runtime: &O3RuntimeState,
    sequence: u64,
    consumed_requests: &[MemoryRequestId],
    execution: &RiscvExecutionRecord,
    issue_tick: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: Option<usize>,
    producer_sequences: &[u64],
) {
    let reservation = runtime
        .writeback_reservation(sequence)
        .expect("speculative writeback reservation exists");
    assert_eq!(reservation.raw_ready_tick(), raw_ready_tick);
    assert_eq!(reservation.admitted_tick(), admitted_tick);
    assert_eq!(Some(reservation.slot()), slot);
    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence)
        .expect("speculative owner exists");
    assert_eq!(issued.consumed_requests, consumed_requests);
    assert_eq!(issued.issue_tick, issue_tick);
    assert_eq!(issued.raw_ready_tick, raw_ready_tick);
    assert_eq!(issued.admitted_writeback_tick, admitted_tick);
    assert_eq!(issued.writeback_slot, slot);
    assert_eq!(issued.producer_sequences, producer_sequences);
    assert_eq!(issued.execution, *execution);
}

fn assert_memory_owner(
    runtime: &O3RuntimeState,
    sequence: u64,
    admitted_tick: Option<u64>,
    slot: Option<usize>,
) {
    let live = runtime
        .live_data_accesses
        .iter()
        .find(|live| live.sequence == sequence)
        .expect("live memory-result owner exists");
    assert_eq!(live.raw_ready_tick, Some(42));
    assert_eq!(live.admitted_writeback_tick, admitted_tick);
    assert_eq!(live.writeback_slot, slot);
    assert_eq!(
        runtime.earliest_unpublished_memory_result_writeback_tick(),
        admitted_tick
    );
    assert_eq!(
        runtime.ready_live_data_access_completion_timing(),
        Some((live.fetch_request, live.issue_tick, admitted_tick.unwrap()))
    );
}

fn assert_writeback_stats(
    runtime: &O3RuntimeState,
    cycles: u64,
    admitted_rows: u64,
    deferred_rows: u64,
    deferred_row_cycles: u64,
    max_ready_rows: u64,
    max_deferred_rows: u64,
) {
    let stats = runtime.stats();
    assert_eq!(stats.writeback_port_cycles(), cycles);
    assert_eq!(stats.writeback_port_admitted_rows(), admitted_rows);
    assert_eq!(stats.writeback_port_deferred_rows(), deferred_rows);
    assert_eq!(
        stats.writeback_port_deferred_row_cycles(),
        deferred_row_cycles
    );
    assert_eq!(
        stats.writeback_port_max_ready_rows_per_cycle(),
        max_ready_rows
    );
    assert_eq!(stats.writeback_port_max_deferred_rows(), max_deferred_rows);
}

fn calendar_rows(runtime: &O3RuntimeState) -> Vec<(u64, u64, usize)> {
    runtime
        .writeback_reservations()
        .iter()
        .map(|reservation| {
            (
                reservation.sequence(),
                reservation.admitted_tick(),
                reservation.slot(),
            )
        })
        .collect()
}

fn calendar_rows_with_raw(runtime: &O3RuntimeState) -> Vec<(u64, u64, u64, usize)> {
    runtime
        .writeback_reservations()
        .iter()
        .map(|reservation| {
            (
                reservation.sequence(),
                reservation.raw_ready_tick(),
                reservation.admitted_tick(),
                reservation.slot(),
            )
        })
        .collect()
}

fn ready_rows_by_tick(runtime: &O3RuntimeState) -> Vec<(u64, Vec<u64>)> {
    runtime
        .live_writeback_ready_rows_by_tick
        .iter()
        .map(|(tick, rows)| (*tick, rows.iter().copied().collect()))
        .collect()
}

fn issue_rows_by_tick(runtime: &O3RuntimeState) -> Vec<(u64, Vec<u64>)> {
    let mut rows = BTreeMap::<u64, Vec<u64>>::new();
    for issued in &runtime.live_speculative_executions {
        rows.entry(issued.issue_tick)
            .or_default()
            .push(issued.sequence);
    }
    rows.into_iter()
        .map(|(tick, mut sequences)| {
            sequences.sort_unstable();
            (tick, sequences)
        })
        .collect()
}

fn assert_issue_capacity(runtime: &O3RuntimeState, width: usize) {
    for (tick, sequences) in issue_rows_by_tick(runtime) {
        assert!(
            sequences.len() <= width,
            "issue tick {tick} exceeds width {width}: {sequences:?}"
        );
        let multiply_rows = sequences
            .iter()
            .filter(|sequence| {
                runtime
                    .live_speculative_executions
                    .iter()
                    .find(|issued| issued.sequence == **sequence)
                    .is_some_and(|issued| {
                        matches!(issued.execution.instruction(), RiscvInstruction::Mul { .. })
                    })
            })
            .count();
        assert!(
            multiply_rows <= 1,
            "issue tick {tick} exceeds multiply capacity: {sequences:?}"
        );
    }
}

fn load_event(pc: u64, sequence: u64, rd: u8) -> RiscvCpuExecutionEvent {
    execution_event(pc, sequence, load_instruction(rd), load_access(rd, 0x9000))
}

fn float_load_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        float_load_instruction(),
        MemoryAccessKind::FloatLoad {
            rd: freg(3),
            address: 0x9000,
            width: MemoryWidth::Doubleword,
        },
    )
}

fn atomic_event(pc: u64, sequence: u64, rd: u8) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        atomic_instruction(rd),
        atomic_access(rd, 0x9000),
    )
}

fn load_instruction(rd: u8) -> RiscvInstruction {
    RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn load_access(rd: u8, address: u64) -> MemoryAccessKind {
    MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn float_load_instruction() -> RiscvInstruction {
    RiscvInstruction::FloatLoad {
        rd: freg(3),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
    }
}

fn load_reserved_instruction(rd: u8) -> RiscvInstruction {
    RiscvInstruction::LoadReserved {
        rd: reg(rd),
        rs1: reg(10),
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    }
}

fn load_reserved_access(rd: u8, address: u64) -> MemoryAccessKind {
    MemoryAccessKind::LoadReserved {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    }
}

fn store_conditional_instruction() -> RiscvInstruction {
    RiscvInstruction::StoreConditional {
        rd: reg(7),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    }
}

fn atomic_instruction(rd: u8) -> RiscvInstruction {
    RiscvInstruction::AtomicMemory {
        rd: reg(rd),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Add,
        acquire: false,
        release: false,
    }
}

fn atomic_access(rd: u8, address: u64) -> MemoryAccessKind {
    MemoryAccessKind::AtomicMemory {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Add,
        value: 3,
        acquire: false,
        release: false,
    }
}

fn vector_unit_instruction(width: MemoryWidth, fault_only_first: bool) -> RiscvInstruction {
    let instruction = if fault_only_first {
        RiscvVectorMemoryInstruction::LoadUnitStrideFaultOnly {
            vd: vreg(2),
            rs1: reg(10),
            width,
            mask: RiscvVectorMaskMode::Masked,
        }
    } else {
        RiscvVectorMemoryInstruction::LoadUnitStride {
            vd: vreg(2),
            rs1: reg(10),
            width,
            mask: RiscvVectorMaskMode::Masked,
        }
    };
    RiscvInstruction::VectorMemory(instruction)
}

fn vector_unit_access(
    width: MemoryWidth,
    byte_len: usize,
    byte_mask: Option<Vec<bool>>,
    group_registers: usize,
    fault_only_first: bool,
) -> MemoryAccessKind {
    MemoryAccessKind::VectorLoadUnitStride {
        vd: vreg(2),
        address: 0x4000,
        width,
        byte_len,
        byte_mask,
        group_registers,
        fault_only_first,
    }
}

fn load_reserved_event(pc: u64, sequence: u64, rd: u8, address: u64) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        RiscvInstruction::LoadReserved {
            rd: reg(rd),
            rs1: reg(10),
            width: MemoryWidth::Doubleword,
            acquire: false,
            release: false,
        },
        MemoryAccessKind::LoadReserved {
            rd: reg(rd),
            address,
            width: MemoryWidth::Doubleword,
            acquire: false,
            release: false,
        },
    )
}

fn vector_unit_event(
    pc: u64,
    sequence: u64,
    address: u64,
    byte_mask: Option<Vec<bool>>,
) -> RiscvCpuExecutionEvent {
    execution_event(
        pc,
        sequence,
        vector_unit_instruction(MemoryWidth::Doubleword, false),
        MemoryAccessKind::VectorLoadUnitStride {
            vd: vreg(2),
            address,
            width: MemoryWidth::Doubleword,
            byte_len: 16,
            byte_mask,
            group_registers: 1,
            fault_only_first: false,
        },
    )
}

fn execution_event(
    pc: u64,
    sequence: u64,
    instruction: RiscvInstruction,
    access: MemoryAccessKind,
) -> RiscvCpuExecutionEvent {
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn core_with_runtime(runtime: O3RuntimeState) -> RiscvCore {
    let core = RiscvCore::new(cpu_core(MemoryRouteId::new(0)));
    core.state.lock().expect("riscv core lock").o3_runtime = runtime;
    core
}

fn translated_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let core = RiscvCore::with_data_translation(
        cpu_core(fetch_route),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core
}

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core
}

fn cpu_core(route: MemoryRouteId) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(0x8000),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            route,
            line_layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn memory_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    MemoryRouteId,
    MemoryRouteId,
) {
    let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();

    (scheduler, transport, fetch_route, data_route)
}

fn page_map(virtual_base: u64, physical_base: u64) -> TranslationPageMap {
    let mut page_map = TranslationPageMap::new(TranslationPageSize::new(4096).unwrap());
    page_map
        .map(
            Address::new(virtual_base),
            Address::new(physical_base),
            1,
            TranslationPagePermissions::read_write_execute(),
        )
        .unwrap();
    page_map
}

fn stage_event(core: &RiscvCore, event: RiscvCpuExecutionEvent) {
    let mut state = core.state.lock().expect("riscv core lock");
    state.hart.write(reg(10), access_address(&event));
    state.events.push(event);
}

fn access_address(event: &RiscvCpuExecutionEvent) -> u64 {
    match event
        .execution()
        .memory_access()
        .expect("test event has a memory access")
    {
        MemoryAccessKind::FloatLoad { address, .. }
        | MemoryAccessKind::LoadReserved { address, .. }
        | MemoryAccessKind::VectorLoadUnitStride { address, .. } => *address,
        access => panic!("unexpected translated test access {access:?}"),
    }
}

fn issue_data_completion_after(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    delay: u64,
    data: Vec<u8>,
) {
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| TargetOutcome::RespondAfter {
            delay,
            response: MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
        },
    )
    .unwrap()
    .expect("memory-result access should issue");
    scheduler.run_until_idle_conservative();
}

fn issue_translated_completion(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    virtual_base: u64,
    physical_base: u64,
    data: Vec<u8>,
) {
    let page_map = page_map(virtual_base, physical_base);
    core.issue_next_translated_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        &page_map,
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(data)).unwrap(),
            )
        },
    )
    .unwrap()
    .expect("translated memory-result access should issue");
    scheduler.run_until_idle_conservative();
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn lsq_sequences(runtime: &O3RuntimeState) -> Vec<u64> {
    runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .map(|entry| entry.sequence())
        .collect()
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn freg(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}
