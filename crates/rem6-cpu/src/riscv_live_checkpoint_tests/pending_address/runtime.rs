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

#[path = "runtime/codec.rs"]
mod codec;
#[path = "runtime/restore.rs"]
mod restore;

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

pub(in crate::riscv_live_checkpoint_tests::pending_address) fn pending_store_payload(
) -> RiscvO3LiveCheckpointPayload {
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
    value.pending_addresses = vec![RiscvO3LiveCheckpointPendingDataAddress {
        sequence: STORE_SEQUENCE,
        fetch: pending_fetch(store(6, 5, MemoryWidth::Doubleword)),
        consumed_requests: vec![request(STORE_SEQUENCE)],
        fetch_predecessor_request: request(PRODUCER_SEQUENCE),
        producer_register: reg(5),
        destination: None,
        producer_sequence: PRODUCER_SEQUENCE,
        root_sequence: PRODUCER_SEQUENCE,
        root_fetch_request: request(PRODUCER_SEQUENCE),
        root_range: AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap())
            .unwrap(),
        root_atomic: false,
        lsq_kind: O3LoadStoreQueueKind::Store,
        expected_lsq_bytes: 8,
        published_producer_ready_tick: Some(99),
        requested_wake_tick: Some(value.wake.tick),
    }];
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
    value.pending_addresses.first_mut().unwrap()
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
