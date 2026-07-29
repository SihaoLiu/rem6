use std::collections::BTreeSet;

use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    O3LoadStoreQueueEntry, O3LoadStoreQueueKind, O3PhysicalRegisterId, O3RegisterClass,
    O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeCheckpointPayload, O3RuntimeSnapshot,
    RiscvCore, RiscvCoreCheckpointRestoreInput, RiscvO3LiveCheckpointFinalizedWriteback,
    RiscvO3LiveCheckpointIssueRow, RiscvO3LiveCheckpointPayload,
    RiscvO3LiveCheckpointPendingDataAddress, RiscvO3LiveCheckpointProfile,
    RiscvO3LiveCheckpointService, RiscvO3LiveCheckpointTelemetry, RiscvO3LiveCheckpointWake,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler, PendingEventSnapshot, ScheduledEventKind};
use rem6_memory::{AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

pub const LIVE_TICK: u64 = 20;
pub const PRODUCER_READY_TICK: u64 = 19;
pub const PRODUCER_SEQUENCE: u64 = 1;
pub const STORE_SEQUENCE: u64 = 2;
pub const STORE_PC: u64 = 0x8004;
pub const COMMITTED_POINTER: u64 = 0xa000;
pub const STORE_VALUE: u64 = 0x1122_3344_5566_7788;
pub const FETCH_ROUTE: MemoryRouteId = MemoryRouteId::new(0);
pub const DATA_ROUTE: MemoryRouteId = MemoryRouteId::new(1);

pub struct SeededPendingAddressCore {
    pub core: RiscvCore,
    pub stable: O3RuntimeCheckpointPayload,
    pub live: RiscvO3LiveCheckpointPayload,
    pub wake: PendingEventSnapshot,
}

pub fn seed_pending_address_core(
    cpu: u32,
    scheduler: &mut PartitionedScheduler,
    kind: ScheduledEventKind,
) -> SeededPendingAddressCore {
    seed_pending_address_core_with_offset(cpu, scheduler, kind, 0)
}

pub fn seed_pending_address_core_with_offset(
    cpu: u32,
    scheduler: &mut PartitionedScheduler,
    kind: ScheduledEventKind,
    store_offset: i32,
) -> SeededPendingAddressCore {
    let core = pending_address_core(cpu);
    let wake_id = match kind {
        ScheduledEventKind::Serial => scheduler.schedule_at(PartitionId::new(0), LIVE_TICK, |_| {}),
        ScheduledEventKind::Parallel => {
            scheduler.schedule_parallel_at(PartitionId::new(0), LIVE_TICK, |_| {})
        }
    }
    .unwrap();
    let wake = scheduler.pending_event_snapshot(wake_id).unwrap();
    let stable = pending_address_stable();
    let live = pending_address_live(cpu, scheduler, wake, store_offset);

    install_pending_address_state(&core, stable.clone(), live.clone()).unwrap();
    core.mark_o3_writeback_wake_scheduled(scheduler.instance_id(), wake);
    assert_pending_address_shape(&core, &stable, &live, scheduler.instance_id(), wake);

    SeededPendingAddressCore {
        core,
        stable,
        live,
        wake,
    }
}

pub fn pending_address_core(cpu: u32) -> RiscvCore {
    let core = RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(0),
                agent(cpu),
                Address::new(STORE_PC),
            ),
            CpuFetchConfig::new(
                fetch_endpoint(cpu),
                FETCH_ROUTE,
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            data_endpoint(cpu),
            DATA_ROUTE,
            CacheLineLayout::new(16).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core
}

pub fn pending_address_live(
    cpu: u32,
    scheduler: &PartitionedScheduler,
    wake: PendingEventSnapshot,
    store_offset: i32,
) -> RiscvO3LiveCheckpointPayload {
    let fetch_request = request(cpu, STORE_SEQUENCE);
    let fetch = completed_store_fetch(cpu, store_offset);
    RiscvO3LiveCheckpointPayload {
        profile: RiscvO3LiveCheckpointProfile::PendingDataAddress,
        captured_tick: LIVE_TICK,
        next_fetch_pc: Address::new(STORE_PC + 4),
        next_fetch_request_sequence: 3,
        events: Vec::new(),
        issue_rows: vec![RiscvO3LiveCheckpointIssueRow {
            sequence: STORE_SEQUENCE,
            fetch_request,
        }],
        rename_rows: Vec::new(),
        resident_sequences: vec![STORE_SEQUENCE],
        executed_fetch_requests: Vec::new(),
        issued_fetch_requests: Vec::new(),
        service: RiscvO3LiveCheckpointService {
            requested_tick: LIVE_TICK,
            mutation_generation: 1,
            last_service_generation: Some((PRODUCER_READY_TICK, 0)),
            telemetry: RiscvO3LiveCheckpointTelemetry {
                enqueued_rows: 1,
                service_turns: 0,
                wake_requests: 1,
                current_occupancy: 1,
                peak_occupancy: 1,
                scalar_integer_issued_rows: 0,
                integer_mul_div_issued_rows: 0,
                memory_agu_issued_rows: 0,
                control_issued_rows: 0,
                scalar_float_issued_rows: 0,
                vector_to_scalar_issued_rows: 0,
            },
        },
        finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback {
            cycles: 0,
            admitted_rows: 0,
            deferred_rows: 0,
            deferred_row_cycles: 0,
            max_ready_rows_per_cycle: 0,
            max_deferred_rows: 0,
            partial_cycle_ticks: BTreeSet::new(),
            partial_ready_rows_by_tick: Default::default(),
            partial_deferred_rows_by_tick: Default::default(),
            closed_before_tick: LIVE_TICK,
        },
        writeback_counted_sequences: Vec::new(),
        writeback_published_sequences: Vec::new(),
        reservation: None,
        completed_result: None,
        pending_addresses: vec![RiscvO3LiveCheckpointPendingDataAddress {
            sequence: STORE_SEQUENCE,
            fetch,
            consumed_requests: vec![fetch_request],
            fetch_predecessor_request: request(cpu, PRODUCER_SEQUENCE),
            producer_register: reg(5),
            destination: None,
            producer_sequence: PRODUCER_SEQUENCE,
            root_sequence: PRODUCER_SEQUENCE,
            root_fetch_request: request(cpu, PRODUCER_SEQUENCE),
            root_range: AddressRange::new(Address::new(0x9000), AccessSize::new(8).unwrap())
                .unwrap(),
            root_atomic: false,
            lsq_kind: O3LoadStoreQueueKind::Store,
            expected_lsq_bytes: 8,
            published_producer_ready_tick: Some(PRODUCER_READY_TICK),
            requested_wake_tick: Some(LIVE_TICK),
        }],
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: scheduler.instance_id().checkpoint_raw(),
            partition: PartitionId::new(0),
            tick: LIVE_TICK,
            scheduler_order: wake.order(),
            kind: wake.kind(),
        },
    }
}

pub fn install_pending_address_state(
    core: &RiscvCore,
    stable: O3RuntimeCheckpointPayload,
    live: RiscvO3LiveCheckpointPayload,
) -> Result<(), rem6_cpu::RiscvCoreCheckpointRestoreError> {
    let mut hart = core.checkpoint_hart_state();
    hart.set_pc(STORE_PC);
    hart.write(reg(5), COMMITTED_POINTER);
    hart.write(reg(6), STORE_VALUE);
    let prepared = core.prepare_checkpoint_restore(RiscvCoreCheckpointRestoreInput::new(
        hart,
        core.pmp_snapshot(),
        core.hart_run_state(),
        core.in_order_pipeline_snapshot(),
        core.branch_predictor_checkpoint_payload(),
        core.gshare_branch_predictor_checkpoint_payload(),
        core.bimode_branch_predictor_checkpoint_payload(),
        core.tournament_branch_predictor_checkpoint_payload(),
        core.tage_sc_l_branch_predictor_checkpoint_payload(),
        core.multiperspective_perceptron_checkpoint_payload(),
        stable,
        Some(live),
    ))?;
    core.install_prepared_checkpoint_restore(prepared);
    Ok(())
}

pub fn pending_address_stable() -> O3RuntimeCheckpointPayload {
    O3RuntimeCheckpointPayload::from_snapshot(
        O3RuntimeSnapshot::new(
            [
                O3ReorderBufferEntry::new(STORE_SEQUENCE, Address::new(STORE_PC), None)
                    .with_live_staged_for_checkpoint(),
            ],
            [O3LoadStoreQueueEntry::store(STORE_SEQUENCE, None, 8)],
            [O3RenameMapEntry::new(
                O3RegisterClass::Integer,
                5,
                O3PhysicalRegisterId::new(5),
            )],
            RiscvCore::default_o3_runtime_checkpoint_payload()
                .snapshot()
                .pending_state()
                .clone(),
        )
        .unwrap(),
    )
    .unwrap()
}

pub fn completed_store_fetch(cpu: u32, offset: i32) -> CpuFetchEvent {
    let raw = s_type(offset, 6, 5, 0b011, 0x23);
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            18,
            PartitionId::new(0),
            FETCH_ROUTE,
            fetch_endpoint(cpu),
            request(cpu, STORE_SEQUENCE),
            Address::new(STORE_PC),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

pub fn request(cpu: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(cpu), sequence)
}

pub fn fetch_endpoint(cpu: u32) -> TransportEndpointId {
    TransportEndpointId::new(format!("cpu{cpu}.ifetch")).unwrap()
}

pub fn data_endpoint(cpu: u32) -> TransportEndpointId {
    TransportEndpointId::new(format!("cpu{cpu}.dmem")).unwrap()
}

fn agent(cpu: u32) -> AgentId {
    AgentId::new(7 + cpu)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    ((imm & 0x0fe0) << 20)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x001f) << 7)
        | opcode
}

fn assert_pending_address_shape(
    core: &RiscvCore,
    stable: &O3RuntimeCheckpointPayload,
    live: &RiscvO3LiveCheckpointPayload,
    scheduler: rem6_kernel::SchedulerInstanceId,
    wake: PendingEventSnapshot,
) {
    live.encode().unwrap();
    let [rob] = stable.snapshot().reorder_buffer() else {
        panic!("pending-address fixture must own one ROB row")
    };
    let [lsq] = stable.snapshot().load_store_queue() else {
        panic!("pending-address fixture must own one LSQ row")
    };
    assert_eq!(rob.sequence(), STORE_SEQUENCE);
    assert_eq!(rob.pc(), Address::new(STORE_PC));
    assert!(rob.destination().is_none() && rob.is_live_staged() && !rob.is_ready());
    assert_eq!(lsq.sequence(), STORE_SEQUENCE);
    assert_eq!(lsq.kind(), O3LoadStoreQueueKind::Store);
    assert!(lsq.address().is_none() && !lsq.is_completed());
    assert_eq!(lsq.bytes(), 8);
    assert!(stable
        .snapshot()
        .reorder_buffer()
        .iter()
        .all(|row| row.sequence() != PRODUCER_SEQUENCE));
    assert!(stable
        .snapshot()
        .load_store_queue()
        .iter()
        .all(|row| row.sequence() != PRODUCER_SEQUENCE));

    let pending = live.pending_addresses.first().unwrap();
    assert_eq!(pending.sequence, STORE_SEQUENCE);
    assert_eq!(pending.root_sequence, PRODUCER_SEQUENCE);
    assert_eq!(pending.producer_sequence, PRODUCER_SEQUENCE);
    assert_eq!(
        pending.published_producer_ready_tick,
        Some(PRODUCER_READY_TICK)
    );
    assert_eq!(pending.requested_wake_tick, Some(LIVE_TICK));
    assert_eq!(live.next_fetch_request_sequence, 3);
    assert_eq!(live.next_fetch_pc, Address::new(STORE_PC + 4));
    assert!(live.events.is_empty());
    assert!(live.executed_fetch_requests.is_empty());
    assert!(live.issued_fetch_requests.is_empty());
    assert!(live.rename_rows.is_empty());
    assert!(live.reservation.is_none());
    assert!(live.completed_result.is_none());
    assert!(live.writeback_counted_sequences.is_empty());
    assert!(live.writeback_published_sequences.is_empty());

    assert_eq!(core.read_register(reg(5)), COMMITTED_POINTER);
    assert_eq!(core.inner().next_sequence(), 3);
    assert_eq!(core.inner().fetch_events(), [pending.fetch.clone()]);
    assert!(core.execution_events().is_empty());
    assert_eq!(core.o3_runtime_snapshot(), *stable.snapshot());
    assert_eq!(core.pending_o3_live_data_access_retirement_count(), 1);
    assert_eq!(core.owned_o3_writeback_wakes(), [(scheduler, wake)]);
}
