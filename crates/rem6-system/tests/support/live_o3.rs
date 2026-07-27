use rem6_cpu::{
    CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    O3PhysicalRegisterId, O3RegisterClass, O3RenameMapEntry, O3ReorderBufferEntry,
    O3RuntimeCheckpointPayload, O3RuntimeSnapshot, RiscvCore, RiscvCoreCheckpointRestoreInput,
    RiscvO3LiveCheckpointEvent, RiscvO3LiveCheckpointFinalizedWriteback,
    RiscvO3LiveCheckpointIssueRow, RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile,
    RiscvO3LiveCheckpointService, RiscvO3LiveCheckpointTelemetry, RiscvO3LiveCheckpointWake,
};
use rem6_isa_riscv::{
    Register, RegisterWrite, RiscvCounterSnapshot, RiscvFloatStatus, RiscvGdbXlen,
    RiscvPrivilegeMode, RiscvStatusWord,
};
use rem6_kernel::{PartitionId, PartitionedScheduler, PendingEventSnapshot, ScheduledEventKind};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};
use std::collections::BTreeSet;

pub const LIVE_TICK: u64 = 20;
pub const O3LC: &str = "o3-live-checkpoint";
pub const O3LH: &str = "o3-live-hart-state";
pub const O3RT: &str = "o3-runtime-state";

pub struct SeededLiveCore {
    pub core: RiscvCore,
    pub live: RiscvO3LiveCheckpointPayload,
    pub wake: PendingEventSnapshot,
}

#[rustfmt::skip]
pub fn seed_live_core(cpu: u32, scheduler: &mut PartitionedScheduler, kind: ScheduledEventKind) -> SeededLiveCore {
    seed_live_core_at(cpu, scheduler, kind, LIVE_TICK, LIVE_TICK)
}

#[rustfmt::skip]
pub fn seed_live_core_at(
    cpu: u32,
    scheduler: &mut PartitionedScheduler,
    kind: ScheduledEventKind,
    wake_tick: u64,
    captured_tick: u64,
) -> SeededLiveCore {
    let core = core(cpu);
    let wake_id = match kind {
        ScheduledEventKind::Serial => scheduler.schedule_at(PartitionId::new(0), wake_tick, |_| {}),
        ScheduledEventKind::Parallel => scheduler.schedule_parallel_at(PartitionId::new(0), wake_tick, |_| {}),
    }.unwrap();
    let wake = scheduler.pending_event_snapshot(wake_id).unwrap();
    let request = MemoryRequestId::new(AgentId::new(7 + cpu), 1);
    let event = RiscvO3LiveCheckpointEvent {
        fetch: CpuFetchEvent::completed(
            CpuFetchRecord::new(10, PartitionId::new(0), MemoryRouteId::new(0), endpoint(cpu), request, Address::new(0x8000), AccessSize::new(4).unwrap()),
            0x0020_81b3_u32.to_le_bytes().to_vec(),
        ),
        execution_pc: 0x8000, next_pc: 0x8004, instruction_bytes: 4,
        register_writes: vec![RegisterWrite::new(Register::new(3).unwrap(), 13)],
        float_register_writes: Vec::new(), memory_access: None, data_access_event_kind: None, counts_as_retired_instruction: true,
    };
    let stable = O3RuntimeCheckpointPayload::from_snapshot(
        O3RuntimeSnapshot::new(
            [O3ReorderBufferEntry::new(1, Address::new(0x8000), Some(O3PhysicalRegisterId::new(1))).with_live_staged_rename_for_checkpoint(O3RegisterClass::Integer, 3)],
            [], [], RiscvCore::default_o3_runtime_checkpoint_payload().snapshot().pending_state().clone(),
        ).unwrap(),
    ).unwrap();
    let live = RiscvO3LiveCheckpointPayload {
        profile: RiscvO3LiveCheckpointProfile::ComputeQueue, captured_tick,
        next_fetch_pc: Address::new(0x8004), next_fetch_request_sequence: 2,
        events: vec![event],
        issue_rows: vec![RiscvO3LiveCheckpointIssueRow { sequence: 1, fetch_request: request }],
        rename_rows: vec![O3RenameMapEntry::new(O3RegisterClass::Integer, 3, O3PhysicalRegisterId::new(1))],
        resident_sequences: vec![1],
        executed_fetch_requests: vec![request],
        issued_fetch_requests: Vec::new(),
        service: RiscvO3LiveCheckpointService {
            requested_tick: wake_tick, mutation_generation: 1,
            last_service_generation: captured_tick.checked_sub(1).map(|previous| (previous, 0)),
            telemetry: RiscvO3LiveCheckpointTelemetry {
                enqueued_rows: 0, service_turns: 0, wake_requests: 0, current_occupancy: 1, peak_occupancy: 1,
                scalar_integer_issued_rows: 0, integer_mul_div_issued_rows: 0, memory_agu_issued_rows: 0,
                control_issued_rows: 0, scalar_float_issued_rows: 0, vector_to_scalar_issued_rows: 0,
            },
        },
        finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback {
            cycles: 0, admitted_rows: 0, deferred_rows: 0, deferred_row_cycles: 0,
            max_ready_rows_per_cycle: 0, max_deferred_rows: 0, partial_cycle_ticks: BTreeSet::new(),
            partial_ready_rows_by_tick: Default::default(), partial_deferred_rows_by_tick: Default::default(),
            closed_before_tick: captured_tick,
        },
        writeback_counted_sequences: Vec::new(), writeback_published_sequences: Vec::new(), reservation: None, completed_result: None,
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: scheduler.instance_id().checkpoint_raw(),
            partition: PartitionId::new(0), tick: wake_tick, scheduler_order: wake.order(), kind,
        },
    };
    let mut hart = core.checkpoint_hart_state();
    hart.set_xlen(RiscvGdbXlen::Rv32); hart.restore_counter_snapshot(&RiscvCounterSnapshot::with_time(31, 32, 33));
    hart.set_privilege_mode(RiscvPrivilegeMode::Supervisor); hart.set_status(RiscvStatusWord::new(0x8_000a)); hart.set_float_status(RiscvFloatStatus::new(0xa5));
    hart.set_supervisor_scratch(0x1234_5678); hart.set_machine_trap_value(0x8765_4321); hart.set_translation_satp(0x8000_0000_0000_0042);
    let prepared = core.prepare_checkpoint_restore(RiscvCoreCheckpointRestoreInput::new(
        hart, core.pmp_snapshot(), core.hart_run_state(), core.in_order_pipeline_snapshot(),
        core.branch_predictor_checkpoint_payload(), core.gshare_branch_predictor_checkpoint_payload(),
        core.bimode_branch_predictor_checkpoint_payload(), core.tournament_branch_predictor_checkpoint_payload(),
        core.tage_sc_l_branch_predictor_checkpoint_payload(), core.multiperspective_perceptron_checkpoint_payload(),
        stable, Some(live.clone()),
    )).unwrap();
    core.install_prepared_checkpoint_restore(prepared);
    core.mark_o3_writeback_wake_scheduled(scheduler.instance_id(), wake);
    SeededLiveCore { core, live, wake }
}

#[rustfmt::skip]
pub fn core(cpu: u32) -> RiscvCore {
    RiscvCore::new(CpuCore::new(
        CpuResetState::new(CpuId::new(cpu), PartitionId::new(0), AgentId::new(7 + cpu), Address::new(0x8000)),
        CpuFetchConfig::new(endpoint(cpu), MemoryRouteId::new(0), CacheLineLayout::new(16).unwrap(), AccessSize::new(4).unwrap()),
    ).unwrap())
}

fn endpoint(cpu: u32) -> TransportEndpointId {
    TransportEndpointId::new(format!("cpu{cpu}.ifetch")).unwrap()
}
