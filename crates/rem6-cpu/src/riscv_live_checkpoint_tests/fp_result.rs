use rem6_isa_riscv::{MemoryAccessKind, RiscvFloatRoundingMode, RiscvInstruction};
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{CacheLineLayout, MemoryRequestId};

use super::*;
use crate::o3_runtime::{O3ArchitecturalRegister, O3DataAccessWindowPolicy, O3RuntimeState};
use crate::riscv_data_completion::RiscvDataCompletion;
use crate::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCoreCheckpointRestoreInput,
    RiscvO3CheckpointProjection,
};

const CAPTURED_TICK: u64 = 41;
const ISSUE_TICK: u64 = 31;
const RESPONSE_TICK: u64 = 41;
const LATENCY_TICKS: u64 = 10;
const LOAD_FETCH_SEQUENCE: u64 = 10;
const CONSUMER_FETCH_SEQUENCE: u64 = 11;
const COLLISION_PEER_FETCH_SEQUENCE: u64 = 11;
const COLLISION_CONSUMER_FETCH_SEQUENCE: u64 = 12;
const COLLISION_PEER_ISSUE_TICK: u64 = 19;
const LOAD_PC: u64 = 0x8000;
const CONSUMER_PC: u64 = 0x8004;
const PHYSICAL_ADDRESS: u64 = 0x9000;
const REQUEST_BYTE_OFFSET: usize = 4;

#[test]
fn response_admitted_flw_and_fld_capture_exact_result_and_prepared_restore() {
    for width in [MemoryWidth::Word, MemoryWidth::Doubleword] {
        let fixture = FpResultFixture::new(width);
        let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
        let live = captured_live(&projection);

        assert_eq!(live.profile, RiscvO3LiveCheckpointProfile::CompletedFpLoad);
        assert!(live.pending_address.is_none());
        assert_eq!(live.captured_tick, CAPTURED_TICK);
        assert_eq!(live.events.len(), 2);
        assert_eq!(
            live.events
                .iter()
                .map(|event| event.fetch.request_id())
                .collect::<Vec<_>>(),
            [fixture.load_fetch, fixture.consumer_fetch],
        );
        assert_eq!(
            live.issue_rows,
            [RiscvO3LiveCheckpointIssueRow {
                sequence: fixture.consumer_sequence,
                fetch_request: fixture.consumer_fetch,
            }],
        );
        assert_eq!(live.resident_sequences, [fixture.consumer_sequence]);
        assert_eq!(live.executed_fetch_requests, [fixture.load_fetch]);
        assert_eq!(live.issued_fetch_requests, [fixture.load_fetch]);
        assert_eq!(live.writeback_counted_sequences, [fixture.load_sequence]);
        assert!(live.writeback_published_sequences.is_empty());

        let result = live.completed_result.as_ref().expect("completed FP result");
        assert!(result.data_request.sequence() > result.fetch_request.sequence());
        assert!(result.data_request.sequence() < live.next_fetch_request_sequence);
        assert_eq!(
            result,
            &RiscvO3LiveCheckpointCompletedFpLoad {
                fetch_request: fixture.load_fetch,
                data_request: fixture.data_request,
                sequence: fixture.load_sequence,
                lsq_sequence: fixture.load_sequence,
                rob_first_sequence: fixture.load_sequence,
                rob_last_sequence: fixture.consumer_sequence,
                issue_tick: ISSUE_TICK,
                response_tick: RESPONSE_TICK,
                raw_ready_tick: fixture.admitted_tick,
                admitted_tick: fixture.admitted_tick,
                latency_ticks: LATENCY_TICKS,
                physical_address: Address::new(PHYSICAL_ADDRESS),
                access_size: fixture.access_size,
                request_byte_offset: REQUEST_BYTE_OFFSET as u32,
                response_bytes: fixture.response_bytes.clone(),
                destination: freg(3),
                width,
            }
        );
        assert_eq!(
            live.reservation,
            Some(RiscvO3LiveCheckpointReservation {
                sequence: fixture.load_sequence,
                raw_ready_tick: fixture.admitted_tick,
                admitted_tick: fixture.admitted_tick,
                slot: 0,
                source: RiscvO3LiveCheckpointWritebackSource::MemoryResult,
                decision_counted: true,
            })
        );
        assert_eq!(live.wake.tick, fixture.admitted_tick);
        assert_eq!(live.service.requested_tick, fixture.admitted_tick);
        assert_eq!(
            live.events[0].memory_access,
            Some(MemoryAccessKind::FloatLoad {
                rd: freg(3),
                address: PHYSICAL_ADDRESS,
                width,
            })
        );
        assert_eq!(
            live.events[0].data_access_event_kind,
            Some(RiscvDataAccessEventKind::Completed)
        );
        assert_eq!(
            live.events[1].float_register_writes,
            [FloatRegisterWrite::new(freg(5), fixture.consumer_value)]
        );
        assert_eq!(
            projection.stable().snapshot().reorder_buffer()[0].sequence(),
            fixture.load_sequence,
        );
        assert_eq!(
            projection.stable().snapshot().load_store_queue()[0].sequence(),
            fixture.load_sequence,
        );
        assert!(projection.stable().snapshot().load_store_queue()[0].is_completed());
        assert_eq!(live.finalized_writeback.cycles, 0);
        assert_eq!(
            live.finalized_writeback.closed_before_tick,
            CAPTURED_TICK + 1,
            "completed result closes the captured writeback cycle"
        );
        assert!(live.finalized_writeback.partial_cycle_ticks.is_empty());
        assert!(live
            .finalized_writeback
            .partial_ready_rows_by_tick
            .is_empty());
        assert!(live
            .finalized_writeback
            .partial_deferred_rows_by_tick
            .is_empty());
        assert_eq!(projection.stable().stats().writeback_port_cycles(), 1);

        let destination = test_core();
        install_projection(&destination, &fixture.core, &projection);
        assert_restored_result(&destination, &fixture, live);
    }
}

#[test]
fn pending_terminal_completed_fp_projection_replays_at_immediate_canonical_pc() {
    for width in [MemoryWidth::Word, MemoryWidth::Doubleword] {
        let fixture = FpResultFixture::new(width);
        fixture.make_load_pending_terminal();
        let source_hart = fixture.core.checkpoint_hart_state();

        let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
        let live = captured_live(&projection);

        assert_eq!(live.profile, RiscvO3LiveCheckpointProfile::CompletedFpLoad);
        assert_eq!(source_hart.pc(), LOAD_PC);
        assert_eq!(live.events[0].next_pc, CONSUMER_PC);
        assert_eq!(live.next_fetch_pc, Address::new(CONSUMER_PC + 4));
        assert_eq!(projection.replay().hart.pc(), live.events[0].next_pc);
        assert_ne!(projection.replay().hart.pc(), live.next_fetch_pc.get());
        assert_eq!(fixture.core.checkpoint_hart_state(), source_hart);
    }
}

#[test]
fn completed_fp_capture_rejects_an_empty_source_lsq() {
    let fixture = FpResultFixture::new(MemoryWidth::Word);
    let source_hart = fixture.core.checkpoint_hart_state();
    fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .clear_live_checkpoint_lsq_for_test();

    assert!(matches!(
        fixture
            .core
            .capture_checkpoint_projection(CAPTURED_TICK)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
    assert_eq!(fixture.core.checkpoint_hart_state(), source_hart);
}

#[test]
fn already_canonical_completed_fp_projection_retains_source_hart() {
    let fixture = FpResultFixture::new(MemoryWidth::Word);
    let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
    let live = captured_live(&projection);

    assert_eq!(
        projection.replay().hart,
        fixture.core.checkpoint_hart_state()
    );
    assert_ne!(projection.replay().hart.pc(), live.next_fetch_pc.get());
}

#[test]
fn restored_fp_result_cleanup_cannot_leak_redirect_retry_failure_or_mode_disable_authority() {
    for width in [MemoryWidth::Word, MemoryWidth::Doubleword] {
        let redirect_fixture = FpResultFixture::new(width);
        let redirect_projection = redirect_fixture
            .core
            .capture_checkpoint_projection(CAPTURED_TICK);
        let redirect = test_core();
        install_projection(&redirect, &redirect_fixture.core, &redirect_projection);
        redirect.redirect_pc(Address::new(0xa000));
        assert_no_result_authority(&redirect, redirect_fixture.load_sequence, "redirect");

        for terminal in [
            RiscvDataAccessEventKind::Retry,
            RiscvDataAccessEventKind::Failed,
        ] {
            let fixture = FpResultFixture::new(width);
            let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
            let destination = test_core();
            install_projection(&destination, &fixture.core, &projection);
            {
                let mut state = destination.state.lock().expect("riscv core lock");
                assert!(state
                    .o3_runtime
                    .rearm_restored_completed_fp_result_for_terminal_injection(
                        fixture.load_fetch,
                        fixture.data_request,
                    ));
            }
            let mut injected = fixture.load_event.clone();
            injected.set_data_access_event_kind(terminal);
            {
                let mut state = destination.state.lock().expect("riscv core lock");
                assert!(state
                    .o3_runtime
                    .complete_live_data_access_response(
                        &injected,
                        fixture.data_request,
                        fixture.admitted_tick,
                        LATENCY_TICKS,
                        None,
                    )
                    .unwrap());
            }
            let terminal_event = destination
                .record_ready_o3_data_access_event_with_trace(fixture.admitted_tick, false)
                .expect("injected terminal FP result drains through production retirement");
            assert_eq!(terminal_event.data_access_event_kind(), Some(terminal));
            assert_no_result_authority(&destination, fixture.load_sequence, "terminal");
        }

        let mode_fixture = FpResultFixture::new(width);
        let mode_projection = mode_fixture
            .core
            .capture_checkpoint_projection(CAPTURED_TICK);
        let mode = test_core();
        install_projection(&mode, &mode_fixture.core, &mode_projection);
        mode.set_detailed_live_retire_gate_enabled(false);
        let published = mode
            .record_ready_o3_data_access_event_with_trace(mode_fixture.admitted_tick, false)
            .expect("restored FP result remains publishable while detailed mode drains");
        assert_eq!(published.fetch().request_id(), mode_fixture.load_fetch);
        assert_eq!(
            mode.read_float_register(freg(3)),
            mode_fixture.load_register_value
        );
        assert_eq!(
            mode.requested_o3_writeback_wake_tick(mode_fixture.admitted_tick + 1),
            None,
        );
        assert_no_result_authority(&mode, mode_fixture.load_sequence, "mode disable");
    }
}

#[test]
fn completed_fld_finalized_peer_tick_or_slot_mutation_is_rejected() {
    assert!(O3RuntimeState::completed_fld_peer_collision_for_test(
        42, 42, 0, 42, 42, 1
    ));
    assert!(!O3RuntimeState::completed_fld_peer_collision_for_test(
        42, 42, 0, 43, 43, 1
    ));
    assert!(!O3RuntimeState::completed_fld_peer_collision_for_test(
        42, 42, 0, 42, 42, 0
    ));
}

#[test]
fn completed_fp_data_request_identity_is_validated_at_capture_and_restore() {
    let invalid_requests = [
        MemoryRequestId::new(AgentId::new(8), CONSUMER_FETCH_SEQUENCE + 1),
        request(CONSUMER_FETCH_SEQUENCE + 2),
        request(CONSUMER_FETCH_SEQUENCE + 3),
        request(LOAD_FETCH_SEQUENCE),
        request(LOAD_FETCH_SEQUENCE - 1),
    ];
    for invalid_request in invalid_requests {
        let fixture = FpResultFixture::new(MemoryWidth::Word);
        assert!(fixture
            .core
            .state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .checkpoint_set_completed_data_request_for_test(invalid_request));
        assert!(matches!(
            fixture
                .core
                .capture_checkpoint_projection(CAPTURED_TICK)
                .live_capture(),
            RiscvO3LiveCheckpointCapture::Rejected
        ));
    }

    let fixture = FpResultFixture::new(MemoryWidth::Word);
    let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
    for invalid_request in invalid_requests {
        let mut live = captured_live(&projection).clone();
        live.completed_result
            .as_mut()
            .expect("completed FP result")
            .data_request = invalid_request;
        assert_restore_rejected_without_mutation(&fixture.core, projection.stable().clone(), live);
    }
}

#[test]
fn completed_fp_restore_rejects_empty_or_unrelated_resident_dependent() {
    let fixture = FpResultFixture::new(MemoryWidth::Word);
    let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);

    let mut empty = captured_live(&projection).clone();
    empty.issue_rows.clear();
    empty.resident_sequences.clear();
    empty.service.telemetry.current_occupancy = 0;
    assert_restore_rejected_without_mutation(&fixture.core, projection.stable().clone(), empty);

    let mut unrelated = captured_live(&projection).clone();
    unrelated.events[1] = event(
        CONSUMER_PC,
        CONSUMER_FETCH_SEQUENCE,
        float_mul(5, 4, 4, MemoryWidth::Word),
        Vec::new(),
        vec![FloatRegisterWrite::new(freg(5), fixture.consumer_value)],
        None,
        None,
    );
    assert_restore_rejected_without_mutation(&fixture.core, projection.stable().clone(), unrelated);

    let mut unrelated_instruction = captured_live(&projection).clone();
    unrelated_instruction.events[1] = event(
        CONSUMER_PC,
        CONSUMER_FETCH_SEQUENCE,
        r_type(0, 4, 3, 0, 5, 0x53),
        Vec::new(),
        vec![FloatRegisterWrite::new(
            freg(5),
            0xffff_ffff_0000_0000 | u64::from(5.0_f32.to_bits()),
        )],
        None,
        None,
    );
    assert_restore_rejected_without_mutation(
        &fixture.core,
        projection.stable().clone(),
        unrelated_instruction,
    );
}

#[test]
fn restored_fld_collision_recaptures_the_same_completed_profile_without_stats_duplication() {
    let fixture = FpResultFixture::new_fld_collision();
    let collision_capture_tick = fixture.admitted_tick;
    fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .compute_checkpoint_projection(collision_capture_tick)
        .expect("source FLD collision must match the bounded runtime profile");
    let first = fixture
        .core
        .capture_checkpoint_projection(collision_capture_tick);
    let first_live = captured_live(&first).clone();
    assert_eq!(first_live.events.len(), 3);
    assert_eq!(first_live.resident_sequences, [fixture.consumer_sequence]);
    assert_eq!(
        first_live.completed_result.as_ref().unwrap().destination,
        freg(1)
    );
    assert_eq!(first_live.reservation.unwrap().slot, 0);

    let destination = test_core();
    destination.set_o3_issue_width(2);
    destination.set_o3_writeback_width(2);
    install_projection(&destination, &fixture.core, &first);
    let mut scheduler = PartitionedScheduler::new(3).unwrap();
    let wake = scheduler
        .schedule_at(PartitionId::new(2), fixture.admitted_tick, |_| {})
        .unwrap();
    destination.mark_o3_writeback_wake_scheduled(
        scheduler.instance_id(),
        scheduler.pending_event_snapshot(wake).unwrap(),
    );

    destination
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .compute_checkpoint_projection(collision_capture_tick)
        .expect("restored FLD collision must reproject");
    let second = destination.capture_checkpoint_projection(collision_capture_tick);
    let second_live = captured_live(&second).clone();
    let mut equivalent_first = first_live;
    equivalent_first.wake = second_live.wake;
    assert_eq!(second_live, equivalent_first);
    assert_eq!(second.stable(), first.stable());
    assert_eq!(
        destination.o3_runtime_stats(),
        fixture.core.o3_runtime_stats(),
        "recapture must not duplicate normalized finalized-peer statistics"
    );
}

#[test]
fn live_fp_restore_atomically_reinstalls_detailed_policy_after_mode_disable() {
    let fixture = FpResultFixture::new(MemoryWidth::Word);
    let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
    let destination = test_core();
    destination.set_detailed_live_retire_gate_enabled(true);
    destination.set_detailed_live_retire_gate_enabled(false);
    assert!(!destination
        .state
        .lock()
        .expect("riscv core lock")
        .live_retire_gate
        .detailed_policy_enabled());

    install_projection(&destination, &fixture.core, &projection);
    let state = destination.state.lock().expect("riscv core lock");
    assert!(state.live_retire_gate.detailed_policy_enabled());
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        [fixture.consumer_sequence],
    );
}

fn assert_restored_result(
    destination: &RiscvCore,
    fixture: &FpResultFixture,
    live: &RiscvO3LiveCheckpointPayload,
) {
    assert_eq!(
        destination.o3_runtime_stats(),
        fixture.core.o3_runtime_stats()
    );
    assert_eq!(
        destination.o3_runtime_snapshot().rename_map(),
        live.rename_rows
    );
    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&fixture.load_fetch)
            .map(|authorization| (
                authorization.role(),
                authorization.integer_destination(),
                authorization.resolved_range(),
            )),
        Some((
            crate::riscv_fetch_ahead::O3MemoryResultWindowRole::Head,
            None,
            rem6_memory::AddressRange::new(Address::new(PHYSICAL_ADDRESS), fixture.access_size)
                .ok(),
        )),
    );
    assert_eq!(
        state.o3_runtime.writeback_reservations(),
        [state
            .o3_runtime
            .writeback_reservation(fixture.load_sequence)
            .expect("restored memory-result reservation")]
    );
    let reservation = state
        .o3_runtime
        .writeback_reservation(fixture.load_sequence)
        .unwrap();
    assert_eq!(reservation.raw_ready_tick(), fixture.admitted_tick);
    assert_eq!(reservation.admitted_tick(), fixture.admitted_tick);
    assert_eq!(reservation.slot(), 0);
    assert!(reservation.decision_counted());
    assert_eq!(
        state.o3_runtime.live_issue_forwarding_artifacts_for_test(
            fixture.load_sequence,
            O3ArchitecturalRegister::floating_point(freg(3)),
        ),
        (true, Some(fixture.admitted_tick), true),
    );
    assert_eq!(
        state
            .o3_runtime
            .checkpoint_issue_plan_at_for_test(RESPONSE_TICK)
            .unwrap(),
        (Vec::new(), vec![fixture.consumer_sequence]),
    );
    assert_eq!(
        state
            .o3_runtime
            .checkpoint_issue_plan_at_for_test(fixture.admitted_tick)
            .unwrap(),
        (vec![fixture.consumer_sequence], Vec::new()),
    );
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        [fixture.consumer_sequence]
    );
    assert!(state.o3_writeback_wake.has_desired_tick());
    assert!(!state.o3_writeback_wake.has_scheduled_wake_authority());
}

fn assert_no_result_authority(core: &RiscvCore, sequence: u64, context: &str) {
    let state = core.state.lock().expect("riscv core lock");
    assert!(
        state.memory_result_window_authorizations.is_empty(),
        "{context}: authorization"
    );
    assert!(
        !state.o3_runtime.has_unpublished_writeback_reservation(),
        "{context}: reservation authority"
    );
    assert!(state
        .o3_runtime
        .live_issue_resident_sequences_for_checkpoint()
        .is_empty());
    assert!(!state.o3_writeback_wake.has_pending_checkpoint_authority());
    assert_eq!(
        state.o3_runtime.live_issue_forwarding_artifacts_for_test(
            sequence,
            O3ArchitecturalRegister::floating_point(freg(3)),
        ),
        (false, None, false),
    );
}

struct FpResultFixture {
    core: RiscvCore,
    load_event: RiscvCpuExecutionEvent,
    load_fetch: MemoryRequestId,
    consumer_fetch: MemoryRequestId,
    data_request: MemoryRequestId,
    load_sequence: u64,
    consumer_sequence: u64,
    admitted_tick: u64,
    access_size: AccessSize,
    response_bytes: Vec<u8>,
    load_register_value: u64,
    consumer_value: u64,
}

impl FpResultFixture {
    fn new(width: MemoryWidth) -> Self {
        Self::new_with_fld_collision(width, false)
    }

    fn new_fld_collision() -> Self {
        Self::new_with_fld_collision(MemoryWidth::Doubleword, true)
    }

    fn make_load_pending_terminal(&self) {
        let raw = self
            .load_event
            .fetch()
            .data()
            .expect("completed load fetch bytes");
        let mut bytes = [0_u8; 4];
        bytes[..raw.len()].copy_from_slice(raw);
        let decoded = RiscvInstruction::decode_with_length(u32::from_le_bytes(bytes)).unwrap();
        let mut state = self.core.state.lock().expect("riscv core lock");
        state
            .events
            .retain(|event| event.fetch().request_id() != self.load_fetch);
        state.executed_fetches.remove(&self.load_fetch);
        state.pending_terminal_memory_result = Some(
            crate::riscv_live_retire_window::RiscvPendingTerminalMemoryResult::ready_for_checkpoint_test(
                self.load_event.clone(),
                vec![self.load_fetch],
                decoded,
            ),
        );
    }

    fn new_with_fld_collision(width: MemoryWidth, fld_collision: bool) -> Self {
        assert!(!fld_collision || width == MemoryWidth::Doubleword);
        let core = test_core();
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_window_depths(1, 5);
        core.set_o3_issue_width(if fld_collision { 2 } else { 1 });
        core.set_o3_writeback_width(if fld_collision { 2 } else { 1 });
        core.write_register(reg(10), PHYSICAL_ADDRESS);
        let (response_bytes, load_register_value, consumer_value) = match width {
            MemoryWidth::Word => (
                2.0_f32.to_bits().to_le_bytes().to_vec(),
                0xffff_ffff_0000_0000 | u64::from(2.0_f32.to_bits()),
                0xffff_ffff_0000_0000 | u64::from(6.0_f32.to_bits()),
            ),
            MemoryWidth::Doubleword => (
                2.0_f64.to_bits().to_le_bytes().to_vec(),
                2.0_f64.to_bits(),
                6.0_f64.to_bits(),
            ),
            _ => unreachable!(),
        };
        core.write_float_register(
            freg(4),
            match width {
                MemoryWidth::Word => 0xffff_ffff_0000_0000 | u64::from(3.0_f32.to_bits()),
                MemoryWidth::Doubleword => 3.0_f64.to_bits(),
                _ => unreachable!(),
            },
        );
        core.write_float_register(freg(3), 4.0_f64.to_bits());
        let load_destination = if fld_collision { 1 } else { 3 };
        let consumer_source = load_destination;
        let load_fetch = request(LOAD_FETCH_SEQUENCE);
        let consumer_fetch = request(if fld_collision {
            COLLISION_CONSUMER_FETCH_SEQUENCE
        } else {
            CONSUMER_FETCH_SEQUENCE
        });
        let data_request = request(consumer_fetch.sequence() + 1);
        let load_projected = event(
            LOAD_PC,
            LOAD_FETCH_SEQUENCE,
            float_load(load_destination, 10, width),
            Vec::new(),
            Vec::new(),
            Some(MemoryAccessKind::FloatLoad {
                rd: freg(load_destination),
                address: PHYSICAL_ADDRESS,
                width,
            }),
            Some(RiscvDataAccessEventKind::Completed),
        );
        let collision_peer = fld_collision.then(|| {
            event(
                CONSUMER_PC,
                COLLISION_PEER_FETCH_SEQUENCE,
                float_sqrt_d(6, 3),
                Vec::new(),
                vec![FloatRegisterWrite::new(freg(6), 2.0_f64.to_bits())],
                None,
                None,
            )
        });
        let consumer_instruction = float_mul_instruction(width, consumer_source);
        let consumer_pc = if fld_collision {
            CONSUMER_PC + 4
        } else {
            CONSUMER_PC
        };
        let consumer_projected = event(
            consumer_pc,
            consumer_fetch.sequence(),
            float_mul(5, consumer_source, 4, width),
            Vec::new(),
            vec![FloatRegisterWrite::new(freg(5), consumer_value)],
            None,
            None,
        );
        let mut fetch_events = vec![load_projected.fetch.clone()];
        fetch_events.extend(collision_peer.iter().map(|event| event.fetch.clone()));
        fetch_events.push(consumer_projected.fetch.clone());
        core.core.state.lock().expect("cpu core lock").events = fetch_events;
        core.inner().set_pc(Address::new(consumer_pc + 4));
        core.inner().advance_sequence_past(data_request);
        assert_eq!(
            core.next_fetch_ahead_before_retire(),
            None,
            "matching FP consumer closes the result window"
        );

        let load_event = load_projected.rebuild().unwrap();
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.stage_live_data_access_issue(
            &load_event,
            data_request,
            ISSUE_TICK,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        let load_sequence = state.o3_runtime.snapshot().reorder_buffer()[0].sequence();
        let younger = if fld_collision {
            vec![
                (
                    Address::new(CONSUMER_PC),
                    RiscvInstruction::FloatSqrtD {
                        rd: freg(6),
                        rs1: freg(3),
                        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
                    },
                ),
                (Address::new(consumer_pc), consumer_instruction),
            ]
        } else {
            vec![(Address::new(CONSUMER_PC), consumer_instruction)]
        };
        assert_eq!(
            state
                .o3_runtime
                .stage_live_data_access_younger_window(load_fetch, younger),
            if fld_collision { 2 } else { 1 },
        );
        let consumer_sequence = state
            .o3_runtime
            .snapshot()
            .reorder_buffer()
            .last()
            .unwrap()
            .sequence();
        if fld_collision {
            assert!(state.o3_runtime.bind_live_staged_issue_packet(
                Address::new(CONSUMER_PC),
                RiscvInstruction::decode_with_length(float_sqrt_d(6, 3)).unwrap(),
                &[request(COLLISION_PEER_FETCH_SEQUENCE)],
                COLLISION_PEER_ISSUE_TICK,
            ));
        }
        assert!(state.o3_runtime.bind_live_staged_issue_packet(
            Address::new(consumer_pc),
            RiscvInstruction::decode_with_length(float_mul(5, consumer_source, 4, width)).unwrap(),
            &[consumer_fetch],
            if fld_collision {
                COLLISION_PEER_ISSUE_TICK
            } else {
                RESPONSE_TICK
            },
        ));
        if fld_collision {
            let hart = state.hart.clone();
            state
                .o3_runtime
                .service_live_issue_queue_at(&hart, COLLISION_PEER_ISSUE_TICK)
                .unwrap();
        }
        let access_size = AccessSize::new(width.bytes() as u64).unwrap();
        let completion = RiscvDataCompletion::from_issued_response(
            load_fetch,
            load_event.execution().memory_access().unwrap().clone(),
            Address::new(PHYSICAL_ADDRESS),
            access_size,
            REQUEST_BYTE_OFFSET,
            Some(response_bytes.clone()),
        );
        assert!(state
            .o3_runtime
            .complete_live_data_access_completion(
                &load_event,
                data_request,
                RESPONSE_TICK,
                LATENCY_TICKS,
                (
                    Address::new(PHYSICAL_ADDRESS),
                    access_size,
                    REQUEST_BYTE_OFFSET,
                ),
                Some(completion),
            )
            .unwrap());
        let admitted_tick = state
            .o3_runtime
            .writeback_reservation(load_sequence)
            .expect("completed FP load reservation")
            .admitted_tick();
        let hart = state.hart.clone();
        state
            .o3_runtime
            .service_live_issue_queue_at(&hart, RESPONSE_TICK)
            .unwrap();
        assert_eq!(
            state
                .o3_runtime
                .live_issue_resident_sequences_for_checkpoint(),
            [consumer_sequence]
        );
        assert_eq!(
            state.o3_runtime.live_issue_service_tick(),
            Some(admitted_tick)
        );
        state.events = vec![load_event.clone()];
        state.executed_fetches.insert(load_fetch);
        state.issued_data_for_fetches.insert(load_fetch);
        state.refresh_o3_writeback_wake(RESPONSE_TICK);
        assert!(state
            .memory_result_window_authorizations
            .remove(&load_fetch)
            .is_some());
        assert!(state.memory_result_window_authorizations.is_empty());
        drop(state);
        let mut scheduler = PartitionedScheduler::new(3).unwrap();
        let wake = scheduler
            .schedule_at(PartitionId::new(2), admitted_tick, |_| {})
            .unwrap();
        core.mark_o3_writeback_wake_scheduled(
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(wake).unwrap(),
        );

        Self {
            core,
            load_event,
            load_fetch,
            consumer_fetch,
            data_request,
            load_sequence,
            consumer_sequence,
            admitted_tick,
            access_size,
            response_bytes,
            load_register_value,
            consumer_value,
        }
    }
}

fn float_mul_instruction(width: MemoryWidth, source: u8) -> RiscvInstruction {
    match width {
        MemoryWidth::Word => RiscvInstruction::FloatMulS {
            rd: freg(5),
            rs1: freg(source),
            rs2: freg(4),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        },
        MemoryWidth::Doubleword => RiscvInstruction::FloatMulD {
            rd: freg(5),
            rs1: freg(source),
            rs2: freg(4),
            rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
        },
        _ => unreachable!(),
    }
}

fn float_sqrt_d(rd: u8, rs1: u8) -> u32 {
    r_type(0x2d, 0, rs1, 0, rd, 0x53)
}

fn test_core() -> RiscvCore {
    let reset = CpuResetState::new(
        CpuId::new(0),
        PartitionId::new(2),
        AgentId::new(7),
        Address::new(LOAD_PC),
    );
    let fetch = CpuFetchConfig::new(
        TransportEndpointId::new("cpu0.ifetch").unwrap(),
        MemoryRouteId::new(9),
        CacheLineLayout::new(16).unwrap(),
        AccessSize::new(4).unwrap(),
    );
    RiscvCore::new(CpuCore::new(reset, fetch).unwrap())
}

fn install_projection(
    destination: &RiscvCore,
    source: &RiscvCore,
    projection: &RiscvO3CheckpointProjection,
) {
    let input = checkpoint_input_with_live(
        source,
        projection.stable().clone(),
        captured_live(projection).clone(),
    );
    let prepared = destination.prepare_checkpoint_restore(input).unwrap();
    destination.install_prepared_checkpoint_restore(prepared);
}

fn checkpoint_input_with_live(
    source: &RiscvCore,
    stable: O3RuntimeCheckpointPayload,
    live: RiscvO3LiveCheckpointPayload,
) -> RiscvCoreCheckpointRestoreInput {
    RiscvCoreCheckpointRestoreInput::new(
        source.checkpoint_hart_state(),
        source.pmp_snapshot(),
        source.hart_run_state(),
        source.in_order_pipeline_snapshot(),
        source.branch_predictor_checkpoint_payload(),
        source.gshare_branch_predictor_checkpoint_payload(),
        source.bimode_branch_predictor_checkpoint_payload(),
        source.tournament_branch_predictor_checkpoint_payload(),
        source.tage_sc_l_branch_predictor_checkpoint_payload(),
        source.multiperspective_perceptron_checkpoint_payload(),
        stable,
        Some(live),
    )
}

pub(super) fn assert_completed_fp_projection_mutation_rejected(
    width: MemoryWidth,
    mutation: impl FnOnce(&mut O3RuntimeCheckpointPayload, &mut RiscvO3LiveCheckpointPayload),
) {
    let fixture = FpResultFixture::new(width);
    let projection = fixture.core.capture_checkpoint_projection(CAPTURED_TICK);
    let mut stable = projection.stable().clone();
    let mut live = captured_live(&projection).clone();
    mutation(&mut stable, &mut live);
    assert_restore_rejected_without_mutation(&fixture.core, stable, live);
}

pub(super) fn assert_completed_fld_occupied_slot_collision_rejected() {
    let fixture = FpResultFixture::new_fld_collision();
    let source_reservations = fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .writeback_reservations();
    assert_eq!(source_reservations.len(), 2);
    let peer = source_reservations
        .iter()
        .find(|reservation| reservation.sequence() != fixture.load_sequence)
        .expect("finalized FLD collision peer reservation");
    assert_eq!(peer.raw_ready_tick(), fixture.admitted_tick);
    assert_eq!(peer.admitted_tick(), fixture.admitted_tick);
    assert_eq!(peer.slot(), 1);
    let projection = fixture
        .core
        .capture_checkpoint_projection(fixture.admitted_tick);
    let mut live = captured_live(&projection).clone();
    let reservation = live
        .reservation
        .as_mut()
        .expect("completed FLD reservation");
    assert_eq!(reservation.slot, 0);
    assert_eq!(live.events.len(), 3);
    assert_eq!(
        live.finalized_writeback.partial_ready_rows_by_tick,
        BTreeMap::from([(fixture.admitted_tick, 1)]),
    );
    assert!(O3RuntimeState::completed_fld_peer_collision_for_test(
        reservation.raw_ready_tick,
        reservation.admitted_tick,
        reservation.slot as usize,
        fixture.admitted_tick,
        fixture.admitted_tick,
        1,
    ));
    reservation.slot = 1;
    assert!((reservation.slot as usize) < 2);
    assert_restore_rejected_without_mutation(&fixture.core, projection.stable().clone(), live);
}

pub(super) fn assert_completed_fp_terminal_capture_rejected(terminal: RiscvDataAccessEventKind) {
    let fixture = FpResultFixture::new(MemoryWidth::Word);
    {
        let mut state = fixture.core.state.lock().expect("riscv core lock");
        assert!(state
            .o3_runtime
            .rearm_restored_completed_fp_result_for_terminal_injection(
                fixture.load_fetch,
                fixture.data_request,
            ));
        let mut injected = fixture.load_event.clone();
        injected.set_data_access_event_kind(terminal);
        assert!(state
            .o3_runtime
            .complete_live_data_access_response(
                &injected,
                fixture.data_request,
                fixture.admitted_tick,
                LATENCY_TICKS,
                None,
            )
            .unwrap());
    }
    assert!(matches!(
        fixture
            .core
            .capture_checkpoint_projection(CAPTURED_TICK)
            .live_capture(),
        RiscvO3LiveCheckpointCapture::Rejected
    ));
}

#[derive(Debug, Eq, PartialEq)]
struct CoreSentinel {
    cpu: crate::cpu_core::CpuCoreCheckpointState,
    riscv: RiscvCoreState,
}

fn core_sentinel(core: &RiscvCore) -> CoreSentinel {
    CoreSentinel {
        cpu: core.core.checkpoint_state(),
        riscv: core.state.lock().expect("riscv core lock").clone(),
    }
}

fn assert_restore_rejected_without_mutation(
    source: &RiscvCore,
    stable: O3RuntimeCheckpointPayload,
    live: RiscvO3LiveCheckpointPayload,
) {
    let destination = test_core();
    destination.write_register(reg(9), 0xfeed_face);
    destination.inner().set_pc(Address::new(0xa000));
    let before = core_sentinel(&destination);
    let runtime_before = destination.o3_runtime_snapshot();
    let stats_before = destination.o3_runtime_stats();
    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_live(source, stable, live))
        .is_err());
    assert_eq!(core_sentinel(&destination), before);
    assert_eq!(destination.o3_runtime_snapshot(), runtime_before);
    assert_eq!(destination.o3_runtime_stats(), stats_before);
}

fn captured_live(projection: &RiscvO3CheckpointProjection) -> &RiscvO3LiveCheckpointPayload {
    match projection.live_capture() {
        RiscvO3LiveCheckpointCapture::Captured(live) => live,
        other => panic!("expected completed FP result capture, got {other:?}"),
    }
}
