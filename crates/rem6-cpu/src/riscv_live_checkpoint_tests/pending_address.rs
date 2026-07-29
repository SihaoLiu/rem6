use super::*;

use rem6_isa_riscv::{Immediate, RiscvExecutionRecord};
use rem6_kernel::{PartitionedScheduler, PendingEventSnapshot, SchedulerInstanceId};
use rem6_memory::CacheLineLayout;

use crate::o3_runtime::{O3DataAccessWindowPolicy, O3PendingDataAddressRequest};
use crate::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreCheckpointRestoreInput,
};

const STORE_SEQUENCE: u64 = 11;
const PRODUCER_SEQUENCE: u64 = 10;
const LIVE_STORE_SEQUENCE: u64 = 1;
const LIVE_PRODUCER_SEQUENCE: u64 = 0;
const STORE_PC: u64 = 0x8000;
const ROOT_ADDRESS: u64 = 0x9000;
const ROOT_PC: u64 = STORE_PC - 4;
const STORE_ADDRESS: u64 = 0xa000;
const STORE_VALUE: u64 = 0x66;
const PRODUCER_RESPONSE_TICK: u64 = 40;
const CAPTURED_TICK: u64 = 41;

#[test]
fn o3_live_checkpoint_v2_round_trips_pending_store_and_decodes_v1() {
    let expected = pending_store_payload();
    let encoded = expected.encode().unwrap();
    assert_eq!(encoded[4], 2);
    assert_eq!(encoded[5], 2);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(&encoded),
        Ok((2, expected.clone())),
    );
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected));

    let legacy = include_bytes!("fixtures/compute-v1.bin");
    assert_eq!(legacy[4], 1);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(legacy),
        Ok((1, compute_payload())),
    );
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(legacy),
        Ok(compute_payload()),
    );
}

#[test]
fn o3_live_checkpoint_v1_rejects_pending_profile_tag() {
    let mut legacy = include_bytes!("fixtures/compute-v1.bin").to_vec();
    assert_eq!(legacy[4], 1);
    legacy[5] = 2;
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&legacy),
        Err(RiscvO3LiveCheckpointError::UnsupportedProfile { profile: 2 }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_missing_pending_row() {
    let mut value = pending_store_payload();
    value.pending_address = None;
    assert_bad_pending_value(value);
}

#[test]
fn o3_live_checkpoint_compute_and_fp_profiles_reject_pending_row() {
    let pending = pending_store_payload().pending_address;
    for mut value in [
        compute_payload(),
        completed_fp_payload(MemoryWidth::Doubleword),
    ] {
        value.pending_address = pending.clone();
        assert_bad_pending_value(value);
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_destinationful_instruction() {
    assert_bad_pending(|value| {
        replace_pending_instruction(value, i_type(0, 5, 3, 7, 0x03));
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_non_store_lsq_kind() {
    assert_bad_pending(|value| {
        pending_mut(value).lsq_kind = O3LoadStoreQueueKind::Load;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_wrong_width_store() {
    assert_bad_pending(|value| {
        replace_pending_instruction(value, store(6, 5, MemoryWidth::Word));
        pending_mut(value).expected_lsq_bytes = 4;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_duplicate_consumed_requests() {
    assert_bad_pending(|value| {
        pending_mut(value).consumed_requests = vec![request(STORE_SEQUENCE); 2];
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_invalid_root_range() {
    let (mut encoded, root_address_offset) = pending_field_offset(|pending| {
        pending.root_range =
            AddressRange::new(Address::new(ROOT_ADDRESS + 1), AccessSize::new(8).unwrap()).unwrap();
    });
    encoded[root_address_offset..root_address_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::InvalidField {
            field: "pending root range",
            value: u64::MAX,
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_sequence_lineage_mismatches() {
    let mutations: &[fn(&mut RiscvO3LiveCheckpointPayload)] = &[
        |value| pending_mut(value).root_sequence = PRODUCER_SEQUENCE - 1,
        |value| pending_mut(value).producer_sequence = STORE_SEQUENCE,
        |value| pending_mut(value).sequence = STORE_SEQUENCE + 1,
    ];
    for mutation in mutations {
        assert_bad_pending(*mutation);
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_publication_after_capture() {
    assert_bad_pending(|value| {
        pending_mut(value).published_producer_ready_tick = value.captured_tick + 1;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_wake_before_publication() {
    assert_bad_pending(|value| {
        let pending = pending_mut(value);
        pending.requested_wake_tick = pending.published_producer_ready_tick - 1;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_trailing_bytes() {
    let mut encoded = pending_store_payload()
        .encode_without_validation_for_test()
        .unwrap();
    encoded.push(0xaa);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::TrailingBytes { remaining: 1 }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_truncated_consumed_request_list() {
    let (mut encoded, count_offset) = pending_field_offset(|pending| {
        pending.consumed_requests.clear();
    });
    encoded.truncate(count_offset + 4 + 11);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::Truncated {
            field: "pending consumed request",
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_excessive_consumed_request_list() {
    let (mut encoded, count_offset) = pending_field_offset(|pending| {
        pending.consumed_requests.clear();
    });
    encoded[count_offset..count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::ExcessiveCount {
            field: "pending consumed requests",
            count: u64::from(u32::MAX),
            maximum: 65_536,
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_invalid_boolean_tag_and_register() {
    let cases = [
        pending_wire_corruption(
            |pending| pending.root_atomic = true,
            2,
            RiscvO3LiveCheckpointError::InvalidBoolean {
                field: "pending root atomic",
                value: 2,
            },
        ),
        pending_wire_corruption(
            |pending| pending.lsq_kind = O3LoadStoreQueueKind::Load,
            9,
            RiscvO3LiveCheckpointError::InvalidTag {
                field: "pending LSQ kind",
                value: 9,
            },
        ),
        pending_wire_corruption(
            |pending| pending.producer_register = reg(6),
            32,
            RiscvO3LiveCheckpointError::InvalidRegister {
                field: "pending producer register",
                index: 32,
            },
        ),
    ];
    for (encoded, expected) in cases {
        assert_eq!(
            RiscvO3LiveCheckpointPayload::decode(&encoded),
            Err(expected)
        );
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_generic_and_writeback_ownership() {
    let mutations: &[fn(&mut RiscvO3LiveCheckpointPayload)] = &[
        |value| value.events = compute_payload().events,
        |value| value.rename_rows = compute_payload().rename_rows,
        |value| value.executed_fetch_requests = vec![request(STORE_SEQUENCE)],
        |value| value.issued_fetch_requests = vec![request(STORE_SEQUENCE)],
        |value| value.writeback_counted_sequences = vec![PRODUCER_SEQUENCE],
        |value| value.writeback_published_sequences = vec![PRODUCER_SEQUENCE],
        |value| value.issue_rows[0].sequence += 1,
        |value| value.resident_sequences[0] += 1,
    ];
    for mutation in mutations {
        assert_bad_pending(*mutation);
    }
}

#[test]
fn pending_store_capture_uses_pending_profile_after_producer_commit() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection);
    let pending = live.pending_address.as_ref().expect("pending store row");

    assert_eq!(
        live.profile,
        RiscvO3LiveCheckpointProfile::PendingDataAddress
    );
    assert_eq!(live.captured_tick, CAPTURED_TICK);
    assert_eq!(live.resident_sequences, [LIVE_STORE_SEQUENCE]);
    assert_eq!(
        live.issue_rows,
        [RiscvO3LiveCheckpointIssueRow {
            sequence: LIVE_STORE_SEQUENCE,
            fetch_request: request(STORE_SEQUENCE),
        }]
    );
    assert_eq!(live.service.requested_tick, CAPTURED_TICK);
    assert_eq!(live.wake.tick, CAPTURED_TICK);
    assert_eq!(
        live.wake.scheduler_instance_raw,
        fixture.scheduler.checkpoint_raw()
    );
    assert_eq!(pending.sequence, LIVE_STORE_SEQUENCE);
    assert_eq!(pending.producer_sequence, LIVE_PRODUCER_SEQUENCE);
    assert_eq!(pending.root_sequence, LIVE_PRODUCER_SEQUENCE);
    assert_eq!(pending.root_range.start(), Address::new(ROOT_ADDRESS));
    assert_eq!(pending.root_range.size(), AccessSize::new(8).unwrap());
    assert_eq!(pending.published_producer_ready_tick, CAPTURED_TICK);
    assert_eq!(pending.requested_wake_tick, CAPTURED_TICK);
    assert_eq!(fixture.core.read_register(reg(5)), STORE_ADDRESS);
}

#[test]
fn pending_store_capture_keeps_generic_execution_events_empty() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection);

    assert!(live.events.is_empty());
    assert!(live.executed_fetch_requests.is_empty());
    assert!(live.issued_fetch_requests.is_empty());
    assert!(live.rename_rows.is_empty());
    assert!(live.completed_result.is_none());
    assert!(live.reservation.is_none());
    assert!(live.writeback_counted_sequences.is_empty());
    assert!(live.writeback_published_sequences.is_empty());
    assert_eq!(fixture.core.execution_events().len(), 1);
}

#[test]
fn pending_store_prepare_rebuilds_exact_destinationless_owner_set() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let live = captured_pending(&projection).clone();
    let destination = pending_store_core();

    install_pending_projection(&destination, &fixture.core, &projection);

    let snapshot = destination.o3_runtime_snapshot();
    let [rob] = snapshot.reorder_buffer() else {
        panic!("expected one restored ROB owner: {snapshot:?}");
    };
    let [lsq] = snapshot.load_store_queue() else {
        panic!("expected one restored LSQ owner: {snapshot:?}");
    };
    assert_eq!(rob.sequence(), LIVE_STORE_SEQUENCE);
    assert_eq!(rob.pc(), Address::new(STORE_PC));
    assert!(rob.destination().is_none());
    assert!(rob.is_live_staged());
    assert!(!rob.is_ready());
    assert_eq!(lsq.sequence(), LIVE_STORE_SEQUENCE);
    assert_eq!(lsq.kind(), O3LoadStoreQueueKind::Store);
    assert_eq!(lsq.address(), None);
    assert_eq!(lsq.bytes(), 8);
    assert!(!lsq.is_completed());

    let restored_telemetry = destination.o3_runtime_live_issue_telemetry();
    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        [LIVE_STORE_SEQUENCE]
    );
    assert_eq!(
        state.o3_runtime.live_issue_service_tick(),
        Some(CAPTURED_TICK)
    );
    assert_eq!(
        restored_telemetry,
        O3LiveIssueTelemetry::from_checkpoint_for_test([
            live.service.telemetry.enqueued_rows,
            live.service.telemetry.service_turns,
            live.service.telemetry.wake_requests,
            live.service.telemetry.current_occupancy,
            live.service.telemetry.peak_occupancy,
            live.service.telemetry.scalar_integer_issued_rows,
            live.service.telemetry.integer_mul_div_issued_rows,
            live.service.telemetry.memory_agu_issued_rows,
            live.service.telemetry.control_issued_rows,
            live.service.telemetry.scalar_float_issued_rows,
            live.service.telemetry.vector_to_scalar_issued_rows,
        ])
    );
    assert!(state
        .o3_runtime
        .live_issue_queue_materializes_for_checkpoint());
    assert_eq!(
        state.o3_runtime.pending_data_address_wake_tick(),
        Some(CAPTURED_TICK)
    );
    drop(state);
    assert_eq!(
        destination.pending_o3_live_data_access_retirement_count(),
        1
    );
}

#[test]
fn pending_store_prepare_restores_fetch_without_execution_or_data_issue() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let pending = captured_pending(&projection)
        .pending_address
        .as_ref()
        .expect("pending store row")
        .clone();
    let destination = pending_store_core();

    install_pending_projection(&destination, &fixture.core, &projection);

    assert_eq!(destination.inner().fetch_events(), [pending.fetch]);
    assert!(destination.execution_events().is_empty());
    let state = destination.state.lock().expect("riscv core lock");
    assert!(!state.executed_fetches.contains(&request(STORE_SEQUENCE)));
    assert!(!state
        .issued_data_for_fetches
        .contains(&request(STORE_SEQUENCE)));
    assert_eq!(state.hart.pc(), STORE_PC);
}

#[test]
fn pending_store_recapture_before_wake_is_identical() {
    let fixture = PendingStoreCheckpointFixture::new();
    let first = fixture.capture();
    let destination = pending_store_core();
    install_pending_projection(&destination, &fixture.core, &first);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    let second = destination.capture_checkpoint_projection(CAPTURED_TICK);

    assert_eq!(captured_pending(&second), captured_pending(&first));
    assert_eq!(second.stable(), first.stable());
}

#[test]
fn pending_store_rebound_wake_materializes_restored_request() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let destination = pending_store_core();
    install_pending_projection(&destination, &fixture.core, &projection);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    destination.mark_o3_writeback_wake_fired(CAPTURED_TICK);

    let state = destination.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_tick_for_test(),
        Some(CAPTURED_TICK)
    );
    assert!(state
        .o3_runtime
        .pending_data_address_materialized_execution_for_test()
        .is_some());
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        []
    );
}

#[test]
fn pending_store_prepare_rejects_stable_cross_reference_without_mutation() {
    let fixture = PendingStoreCheckpointFixture::new();
    let projection = fixture.capture();
    let stable = projection.stable().clone();
    let live = captured_pending(&projection).clone();
    let invalid_stable = super::compute::rebuilt_stable(
        &stable,
        None,
        Some(vec![O3LoadStoreQueueEntry::store(
            LIVE_STORE_SEQUENCE + 1,
            None,
            8,
        )]),
        None,
    )
    .unwrap();
    let destination = pending_store_core();
    destination.write_register(reg(9), 0xfeed_face);
    destination.inner().set_pc(Address::new(0xb000));
    let destination_cpu = destination.core.checkpoint_state();
    let destination_riscv = destination.state.lock().expect("riscv core lock").clone();

    assert!(destination
        .prepare_checkpoint_restore(checkpoint_input_with_live(
            &fixture.core,
            invalid_stable,
            live.clone(),
        ))
        .is_err());

    assert_eq!(projection.stable(), &stable);
    assert_eq!(captured_pending(&projection), &live);
    assert_eq!(destination.core.checkpoint_state(), destination_cpu);
    assert_eq!(
        *destination.state.lock().expect("riscv core lock"),
        destination_riscv
    );
}

struct PendingStoreCheckpointFixture {
    core: RiscvCore,
    scheduler: SchedulerInstanceId,
    wake: PendingEventSnapshot,
}

impl PendingStoreCheckpointFixture {
    fn new() -> Self {
        let core = pending_store_core();
        core.set_o3_window_depths(2, 2);
        core.set_o3_issue_width(1);
        core.write_register(reg(2), ROOT_ADDRESS);
        core.write_register(reg(6), STORE_VALUE);

        let producer = producer_event();
        let store_fetch = pending_fetch(store(6, 5, MemoryWidth::Doubleword));
        {
            let mut cpu = core.core.state.lock().expect("cpu core lock");
            cpu.events = vec![producer.fetch().clone(), store_fetch.clone()];
        }
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.events.push(producer.clone());
            state.executed_fetches.insert(request(PRODUCER_SEQUENCE));
            state
                .issued_data_for_fetches
                .insert(request(PRODUCER_SEQUENCE));
            assert!(state.o3_runtime.stage_live_data_access_issue(
                &producer,
                request(20),
                31,
                O3DataAccessWindowPolicy::MemoryResultWindow,
            ));
            let pending = O3PendingDataAddressRequest::new(
                request(PRODUCER_SEQUENCE),
                store_fetch,
                vec![request(STORE_SEQUENCE)],
                RiscvInstruction::decode_with_length(store(6, 5, MemoryWidth::Doubleword)).unwrap(),
                reg(5),
            );
            assert_eq!(
                state.o3_runtime.stage_pending_data_address_window(
                    request(PRODUCER_SEQUENCE),
                    [pending],
                    [],
                    0,
                ),
                1
            );
            let hart = state.hart.clone();
            state
                .o3_runtime
                .service_live_issue_scheduler_at(&hart, 0)
                .unwrap();
            assert_eq!(state.o3_runtime.live_issue_service_tick(), None);

            let mut completed = producer.clone();
            completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
            assert!(state
                .o3_runtime
                .complete_live_data_access_response(
                    &completed,
                    request(20),
                    PRODUCER_RESPONSE_TICK,
                    PRODUCER_RESPONSE_TICK - 31,
                    Some(&STORE_ADDRESS.to_le_bytes()),
                )
                .unwrap());
        }

        let published = core
            .record_ready_o3_data_access_event_with_trace(CAPTURED_TICK, false)
            .expect("producer publishes through the production retirement path");
        assert_eq!(published.fetch().request_id(), request(PRODUCER_SEQUENCE));
        assert_eq!(core.read_register(reg(5)), STORE_ADDRESS);
        core.inner().set_pc(Address::new(STORE_PC));
        core.inner().advance_sequence_past(request(STORE_SEQUENCE));
        core.state
            .lock()
            .expect("riscv core lock")
            .hart
            .set_pc(STORE_PC);
        assert_eq!(
            core.requested_o3_writeback_wake_tick(CAPTURED_TICK),
            Some(CAPTURED_TICK)
        );

        let mut scheduler = PartitionedScheduler::new(3).unwrap();
        let event = scheduler
            .schedule_at(PartitionId::new(2), CAPTURED_TICK, |_| {})
            .unwrap();
        let scheduler_id = scheduler.instance_id();
        let wake = scheduler.pending_event_snapshot(event).unwrap();
        core.mark_o3_writeback_wake_scheduled(scheduler_id, wake);

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
        assert_eq!(
            state
                .o3_runtime
                .pending_data_address_selected_issue_tick_for_test(),
            None
        );
        assert!(state
            .o3_runtime
            .pending_data_address_materialized_execution_for_test()
            .is_none());
        assert_eq!(
            state
                .o3_runtime
                .live_issue_resident_sequences_for_checkpoint(),
            [LIVE_STORE_SEQUENCE]
        );
        assert_eq!(
            state.o3_runtime.live_issue_service_tick(),
            Some(CAPTURED_TICK)
        );
        assert_eq!(
            state.o3_runtime.pending_data_address_wake_tick(),
            Some(CAPTURED_TICK)
        );
        assert!(state.outstanding_data.is_empty());
        assert!(state.buffered_o3_effects.is_empty());
        drop(state);
        assert_eq!(core.owned_o3_writeback_wakes(), [(scheduler_id, wake)]);

        Self {
            core,
            scheduler: scheduler_id,
            wake,
        }
    }

    fn capture(&self) -> RiscvO3CheckpointProjection {
        self.core.capture_checkpoint_projection(CAPTURED_TICK)
    }
}

fn pending_store_core() -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(2),
                AgentId::new(7),
                Address::new(ROOT_PC),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRouteId::new(9),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

fn producer_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(5),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        completed_fetch(ROOT_PC, PRODUCER_SEQUENCE, i_type(0, 2, 3, 5, 0x03)),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            ROOT_PC,
            STORE_PC,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(5),
                address: ROOT_ADDRESS,
                width: MemoryWidth::Doubleword,
                signed: false,
            }),
        ),
    )
}

fn captured_pending(projection: &RiscvO3CheckpointProjection) -> &RiscvO3LiveCheckpointPayload {
    match projection.live_capture() {
        RiscvO3LiveCheckpointCapture::Captured(live) => live,
        other => panic!("expected pending-address capture, got {other:?}"),
    }
}

fn install_pending_projection(
    destination: &RiscvCore,
    source: &RiscvCore,
    projection: &RiscvO3CheckpointProjection,
) {
    let prepared = destination
        .prepare_checkpoint_restore(checkpoint_input_with_live(
            source,
            projection.stable().clone(),
            captured_pending(projection).clone(),
        ))
        .unwrap();
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

fn pending_store_payload() -> RiscvO3LiveCheckpointPayload {
    let mut value = compute_payload();
    value.profile = RiscvO3LiveCheckpointProfile::PendingDataAddress;
    value.next_fetch_pc = Address::new(STORE_PC + 4);
    value.events.clear();
    value.issue_rows = vec![RiscvO3LiveCheckpointIssueRow {
        sequence: STORE_SEQUENCE,
        fetch_request: request(STORE_SEQUENCE),
    }];
    value.rename_rows.clear();
    value.resident_sequences = vec![STORE_SEQUENCE];
    value.executed_fetch_requests.clear();
    value.issued_fetch_requests.clear();
    value.service.telemetry.current_occupancy = 1;
    value.pending_address = Some(RiscvO3LiveCheckpointPendingDataAddress {
        sequence: STORE_SEQUENCE,
        fetch: pending_fetch(store(6, 5, MemoryWidth::Doubleword)),
        consumed_requests: vec![request(STORE_SEQUENCE)],
        fetch_predecessor_request: request(PRODUCER_SEQUENCE),
        producer_register: reg(5),
        producer_sequence: PRODUCER_SEQUENCE,
        root_sequence: PRODUCER_SEQUENCE,
        root_fetch_request: request(PRODUCER_SEQUENCE),
        root_range: AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap())
            .unwrap(),
        root_atomic: false,
        lsq_kind: O3LoadStoreQueueKind::Store,
        expected_lsq_bytes: 8,
        published_producer_ready_tick: 99,
        requested_wake_tick: value.wake.tick,
    });
    value
}

fn pending_fetch(raw: u32) -> CpuFetchEvent {
    completed_fetch(STORE_PC, STORE_SEQUENCE, raw)
}

fn completed_fetch(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    let bytes = raw.to_le_bytes().to_vec();
    let record = CpuFetchRecord::new(
        31,
        PartitionId::new(2),
        MemoryRouteId::new(9),
        TransportEndpointId::new("cpu0.ifetch").unwrap(),
        request(sequence),
        Address::new(pc),
        AccessSize::new(bytes.len() as u64).unwrap(),
    );
    CpuFetchEvent::completed(record, bytes)
}

fn replace_pending_instruction(value: &mut RiscvO3LiveCheckpointPayload, raw: u32) {
    pending_mut(value).fetch = pending_fetch(raw);
}

fn pending_mut(
    value: &mut RiscvO3LiveCheckpointPayload,
) -> &mut RiscvO3LiveCheckpointPendingDataAddress {
    value.pending_address.as_mut().unwrap()
}

fn assert_bad_pending(change: impl FnOnce(&mut RiscvO3LiveCheckpointPayload)) {
    let mut value = pending_store_payload();
    change(&mut value);
    assert_bad_pending_value(value);
}

fn assert_bad_pending_value(value: RiscvO3LiveCheckpointPayload) {
    assert!(matches!(
        value.encode(),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
    let encoded = value.encode_without_validation_for_test().unwrap();
    assert!(matches!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
}

fn pending_wire_corruption(
    change: impl FnOnce(&mut RiscvO3LiveCheckpointPendingDataAddress),
    corrupt: u8,
    expected: RiscvO3LiveCheckpointError,
) -> (Vec<u8>, RiscvO3LiveCheckpointError) {
    let (mut encoded, offset) = pending_field_offset(change);
    encoded[offset] = corrupt;
    (encoded, expected)
}

fn pending_field_offset(
    change: impl FnOnce(&mut RiscvO3LiveCheckpointPendingDataAddress),
) -> (Vec<u8>, usize) {
    let value = pending_store_payload();
    let encoded = value.encode_without_validation_for_test().unwrap();
    let mut changed = value;
    change(pending_mut(&mut changed));
    let changed = changed.encode_without_validation_for_test().unwrap();
    let offset = encoded
        .iter()
        .zip(&changed)
        .position(|(left, right)| left != right)
        .expect("changed pending field must alter its wire encoding");
    (encoded, offset)
}

fn store(rs2: u8, rs1: u8, width: MemoryWidth) -> u32 {
    let funct3 = match width {
        MemoryWidth::Word => 2,
        MemoryWidth::Doubleword => 3,
        _ => unreachable!(),
    };
    (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | 0x23
}
