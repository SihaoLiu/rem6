use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuFetchEvent, CpuFetchEventKind, CpuId,
    O3LoadStoreQueueKind, O3PhysicalRegisterId, O3RegisterClass, O3RuntimeCheckpointPayload,
    RiscvCluster, RiscvCore, RiscvDataAccessEventKind, RiscvO3LiveCheckpointCapture,
    RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{
    ParallelSchedulerContext, PartitionId, PartitionedScheduler, PendingEventSnapshot,
    ScheduledEventKind, SchedulerInstanceId,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    riscv_execution_mode_target_for_cpu, ExecutionMode, GuestEventId, GuestSourceId,
    HostEventPolicy, RiscvSystemRunDriver, RiscvTrapEventPort, SystemHostController,
    SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

const MAX_SEED_TICK: u64 = 160;
pub const ROOT_PC: u64 = 0x8000;
pub const FIRST_LOAD_PC: u64 = 0x8004;
pub const ROOT_ADDRESS: u64 = 0x9000;
pub const ROOT_VALUE: u64 = 0xa000;
pub const FETCH_ROUTE: MemoryRouteId = MemoryRouteId::new(0);
pub const DATA_ROUTE: MemoryRouteId = MemoryRouteId::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingLoadGraphProgram {
    MixedFanout,
    DivergentRegisters,
}

impl PendingLoadGraphProgram {
    const fn root_register(self) -> u8 {
        match self {
            Self::MixedFanout => 5,
            Self::DivergentRegisters => 10,
        }
    }

    const fn first_destination_register(self) -> u8 {
        match self {
            Self::MixedFanout => 6,
            Self::DivergentRegisters => 11,
        }
    }

    const fn producers(self) -> [u8; 3] {
        match self {
            Self::MixedFanout => [5, 5, 7],
            Self::DivergentRegisters => [10, 10, 12],
        }
    }

    const fn producer_sequences(self) -> [u64; 3] {
        match self {
            Self::MixedFanout => [0, 0, 2],
            Self::DivergentRegisters => [0, 0, 2],
        }
    }

    const fn root_ready(self) -> [bool; 3] {
        match self {
            Self::MixedFanout => [true, true, false],
            Self::DivergentRegisters => [true, true, false],
        }
    }
}

pub struct SeededPendingLoadGraphCore {
    pub core: RiscvCore,
    pub stable: O3RuntimeCheckpointPayload,
    pub live: RiscvO3LiveCheckpointPayload,
    pub scheduler: Arc<Mutex<PartitionedScheduler>>,
    pub wake: PendingEventSnapshot,
    pub wake_kind: ScheduledEventKind,
    pub capture_tick: u64,
}

pub fn seed_pending_load_graph_core() -> SeededPendingLoadGraphCore {
    seed_pending_load_graph_core_with_program(PendingLoadGraphProgram::MixedFanout)
}

pub fn seed_pending_load_graph_core_with_program(
    program: PendingLoadGraphProgram,
) -> SeededPendingLoadGraphCore {
    let scheduler = Arc::new(Mutex::new(
        PartitionedScheduler::with_min_remote_delay(4, 2).unwrap(),
    ));
    seed_pending_load_graph_core_on_scheduler(program, scheduler)
}

pub fn seed_pending_load_graph_core_on_scheduler(
    program: PendingLoadGraphProgram,
    scheduler: Arc<Mutex<PartitionedScheduler>>,
) -> SeededPendingLoadGraphCore {
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                fetch_endpoint(0),
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
                data_endpoint(0),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(fetch_route, FETCH_ROUTE);
    assert_eq!(data_route, DATA_ROUTE);

    let core = pending_load_graph_core(fetch_route, data_route);
    core.write_register(reg(2), ROOT_ADDRESS);
    let cluster = RiscvCluster::new([core]).unwrap();
    let core = cluster.core(CpuId::new(0)).unwrap();
    let memory_target = MemoryTargetId::new(0);
    let memory = loaded_program_store(program, memory_target);
    let driver = pending_load_graph_driver();

    let (scheduler_id, wake, stable, live) = {
        let mut scheduler = scheduler.lock().unwrap();
        let mut next_limit = 1;
        loop {
            if next_limit > MAX_SEED_TICK {
                panic!(
                    "pending-load graph source did not reach production capture by tick {MAX_SEED_TICK}"
                );
            }
            driver
                .drive_until_host_stop_or_tick_limit_parallel(
                    &cluster,
                    &mut scheduler,
                    &transport,
                    MemoryTrace::new(),
                    MemoryTrace::new(),
                    |_cpu| responder(Arc::clone(&memory)),
                    |_cpu| responder(Arc::clone(&memory)),
                    next_limit,
                    |cpu| GuestEventId::new(600 + u64::from(cpu.get())),
                )
                .unwrap();
            if let Some(capture) = production_pending_load_graph_capture(&core, program) {
                break capture;
            }
            next_limit = scheduler.now().saturating_add(1).max(next_limit + 1);
        }
    };
    assert_eq!(scheduler_id, scheduler.lock().unwrap().instance_id());
    assert_eq!(wake.kind(), ScheduledEventKind::Parallel);
    let capture_tick = wake.tick();
    assert_pending_load_graph_shape(
        &core,
        &stable,
        &live,
        scheduler_id,
        wake,
        scheduler_id,
        wake,
        program,
        capture_tick,
    );

    SeededPendingLoadGraphCore {
        core,
        stable,
        live,
        scheduler,
        wake,
        wake_kind: ScheduledEventKind::Parallel,
        capture_tick,
    }
}

pub fn graph_fetches(live: &RiscvO3LiveCheckpointPayload) -> Vec<rem6_cpu::CpuFetchEvent> {
    live.pending_addresses
        .iter()
        .map(|pending| pending.fetch.clone())
        .collect()
}

fn production_pending_load_graph_capture(
    core: &RiscvCore,
    program: PendingLoadGraphProgram,
) -> Option<(
    SchedulerInstanceId,
    PendingEventSnapshot,
    O3RuntimeCheckpointPayload,
    RiscvO3LiveCheckpointPayload,
)> {
    let owned_wakes = core.owned_o3_writeback_wakes();
    let [(scheduler_id, wake)] = owned_wakes.as_slice() else {
        return None;
    };
    let projection = core.capture_checkpoint_projection(wake.tick());
    let RiscvO3LiveCheckpointCapture::Captured(live) = projection.live_capture() else {
        return None;
    };
    if live.profile != RiscvO3LiveCheckpointProfile::PendingDataAddress
        || live.pending_addresses.len() != 3
    {
        return None;
    }
    assert_eq!(
        projection.replay().hart.read(reg(program.root_register())),
        ROOT_VALUE
    );
    Some((
        *scheduler_id,
        *wake,
        projection.stable().clone(),
        live.clone(),
    ))
}

pub fn assert_pending_load_graph_shape(
    core: &RiscvCore,
    stable: &O3RuntimeCheckpointPayload,
    live: &RiscvO3LiveCheckpointPayload,
    live_scheduler: rem6_kernel::SchedulerInstanceId,
    live_wake: PendingEventSnapshot,
    owned_scheduler: rem6_kernel::SchedulerInstanceId,
    owned_wake: PendingEventSnapshot,
    program: PendingLoadGraphProgram,
    capture_tick: u64,
) {
    live.encode().unwrap();
    assert_eq!(
        live.profile,
        RiscvO3LiveCheckpointProfile::PendingDataAddress
    );
    assert_eq!(live.captured_tick, capture_tick);
    assert_eq!(live.pending_addresses.len(), 3);
    assert_eq!(live.events, []);
    assert_eq!(live.issue_rows.len(), 3);
    assert_eq!(live.rename_rows, []);
    assert_eq!(live.resident_sequences, [1, 2, 3]);
    assert_eq!(live.executed_fetch_requests, []);
    assert_eq!(live.issued_fetch_requests, []);
    assert!(live.completed_result.is_none());
    assert!(live.reservation.is_none());
    assert_eq!(live.service.requested_tick, capture_tick);
    assert_eq!(live.wake.tick, capture_tick);
    assert_eq!(
        live.wake.scheduler_instance_raw,
        live_scheduler.checkpoint_raw()
    );
    assert_eq!(live.wake.scheduler_order, live_wake.order());
    assert_eq!(live.wake.kind, live_wake.kind());
    assert_eq!(owned_wake.tick(), capture_tick);
    assert_eq!(owned_wake.kind(), live_wake.kind());
    assert_completed_fetch_projection(core, live);
    let carrier = assert_o3_projection(core, stable, live, program);
    if carrier == GraphCarrier::Source {
        assert_source_execution_projection(core);
    }
    assert_eq!(
        core.pending_o3_live_data_access_retirement_count(),
        carrier.expected_pending_retirements()
    );
    assert_eq!(
        core.owned_o3_writeback_wakes(),
        [(owned_scheduler, owned_wake)]
    );

    let producers = program.producers();
    let producer_sequences = program.producer_sequences();
    let root_ready = program.root_ready();

    for (index, row) in live.pending_addresses.iter().enumerate() {
        let sequence = 1 + index as u64;
        let destination = row.destination.expect("pending load destination");
        assert_eq!(row.sequence, sequence);
        assert_eq!(row.fetch.request_id(), request(sequence));
        assert_eq!(
            row.fetch.pc(),
            Address::new(FIRST_LOAD_PC + 4 * index as u64)
        );
        assert_eq!(row.consumed_requests, [request(sequence)]);
        assert_eq!(row.fetch_predecessor_request, request(sequence - 1));
        assert_eq!(row.producer_register, reg(producers[index]));
        assert_eq!(row.producer_sequence, producer_sequences[index]);
        assert_eq!(row.root_sequence, 0);
        assert_eq!(row.root_fetch_request, request(0));
        assert_eq!(row.root_range, root_range());
        assert!(!row.root_atomic);
        assert_eq!(row.lsq_kind, O3LoadStoreQueueKind::Load);
        assert_eq!(row.expected_lsq_bytes, 8);
        assert_eq!(
            row.published_producer_ready_tick,
            root_ready[index].then_some(capture_tick)
        );
        assert_eq!(
            row.requested_wake_tick,
            root_ready[index].then_some(capture_tick)
        );
        assert_eq!(
            (destination.register_class(), destination.architectural()),
            (
                O3RegisterClass::Integer,
                u32::from(program.first_destination_register() + index as u8),
            )
        );
        assert!(live
            .issue_rows
            .iter()
            .any(|issue| issue.sequence == row.sequence
                && issue.fetch_request == row.fetch.request_id()));
    }

    let snapshot = stable.snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 3);
    assert_eq!(snapshot.load_store_queue().len(), 3);
    assert!(snapshot.rename_map().iter().any(|entry| {
        entry.register_class() == O3RegisterClass::Integer
            && entry.architectural() == u32::from(program.root_register())
    }));
    for (owner, pending) in snapshot
        .reorder_buffer()
        .iter()
        .zip(&live.pending_addresses)
    {
        let destination = pending.destination.expect("pending load destination");
        assert_eq!(owner.sequence(), pending.sequence);
        assert_eq!(owner.pc(), pending.fetch.pc());
        assert_eq!(owner.destination(), Some(destination.physical()));
        assert!(owner.is_live_staged());
        assert!(!owner.is_ready());
    }
    for (entry, pending) in snapshot
        .load_store_queue()
        .iter()
        .zip(&live.pending_addresses)
    {
        assert_eq!(entry.sequence(), pending.sequence);
        assert_eq!(entry.kind(), O3LoadStoreQueueKind::Load);
        assert_eq!(entry.bytes(), pending.expected_lsq_bytes);
        assert!(entry.address().is_none());
        assert!(!entry.is_completed());
    }
}

fn assert_completed_fetch_projection(core: &RiscvCore, live: &RiscvO3LiveCheckpointPayload) {
    let completed = completed_fetches(core);
    let graph = graph_fetches(live);
    if completed.len() == graph.len() + 1 {
        assert_eq!(
            completed.iter().map(|event| event.pc()).collect::<Vec<_>>(),
            [
                Address::new(ROOT_PC),
                Address::new(FIRST_LOAD_PC),
                Address::new(FIRST_LOAD_PC + 4),
                Address::new(FIRST_LOAD_PC + 8),
            ]
        );
        assert_eq!(&completed[1..], graph.as_slice());
    } else {
        assert_eq!(completed, graph);
    }
}

fn completed_fetches(core: &RiscvCore) -> Vec<CpuFetchEvent> {
    core.inner()
        .fetch_events()
        .into_iter()
        .filter(|event| event.kind() == CpuFetchEventKind::Completed)
        .collect()
}

fn assert_source_execution_projection(core: &RiscvCore) {
    let events = core.execution_events();
    let [root] = events.as_slice() else {
        panic!("pending-load graph source must only retain the root execution event")
    };
    assert_eq!(root.fetch_pc(), Address::new(ROOT_PC));
    assert_eq!(root.fetch().request_id(), request(0));
    assert_eq!(
        root.data_access_event_kind(),
        Some(RiscvDataAccessEventKind::Completed)
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GraphCarrier {
    Source,
    Restored,
}

impl GraphCarrier {
    const fn expected_pending_retirements(self) -> usize {
        match self {
            Self::Source => 4,
            Self::Restored => 3,
        }
    }
}

fn assert_o3_projection(
    core: &RiscvCore,
    stable: &O3RuntimeCheckpointPayload,
    live: &RiscvO3LiveCheckpointPayload,
    program: PendingLoadGraphProgram,
) -> GraphCarrier {
    let runtime = core.o3_runtime_snapshot();
    let root_rename = stable
        .snapshot()
        .rename_map()
        .iter()
        .find(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == u32::from(program.root_register())
        })
        .expect("stable projection must retain the root rename owner");
    let expected_rename = [
        *root_rename,
        live.pending_addresses[0]
            .destination
            .expect("first pending load destination"),
        live.pending_addresses[1]
            .destination
            .expect("second pending load destination"),
        live.pending_addresses[2]
            .destination
            .expect("third pending load destination"),
    ];

    if runtime.reorder_buffer() == stable.snapshot().reorder_buffer()
        && runtime.load_store_queue() == stable.snapshot().load_store_queue()
    {
        assert_eq!(core.read_register(reg(program.root_register())), ROOT_VALUE);
        assert_eq!(runtime.rename_map(), expected_rename);
        return GraphCarrier::Restored;
    }

    assert_eq!(core.read_register(reg(program.root_register())), 0);
    let rob = runtime.reorder_buffer();
    let lsq = runtime.load_store_queue();
    assert_eq!(rob.len(), live.pending_addresses.len() + 1);
    assert_eq!(lsq.len(), live.pending_addresses.len() + 1);
    assert_eq!(rob[0].sequence(), 0);
    assert_eq!(rob[0].pc(), Address::new(ROOT_PC));
    assert_eq!(rob[0].destination(), Some(root_rename.physical()));
    assert!(rob[0].is_live_staged());
    assert!(!rob[0].is_ready());
    assert_eq!(lsq[0].sequence(), 0);
    assert_eq!(lsq[0].kind(), O3LoadStoreQueueKind::Load);
    assert_eq!(lsq[0].address(), Some(root_range().start()));
    assert_eq!(lsq[0].bytes(), 8);
    assert!(lsq[0].is_completed());
    assert_eq!(runtime.rename_map(), expected_rename);
    assert_eq!(rob[1..], *stable.snapshot().reorder_buffer());
    assert_eq!(lsq[1..], *stable.snapshot().load_store_queue());
    GraphCarrier::Source
}

fn pending_load_graph_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let core = RiscvCore::with_data(
        CpuCore::new(
            rem6_cpu::CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(ROOT_PC),
            ),
            CpuFetchConfig::new(
                fetch_endpoint(0),
                fetch_route,
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(
            data_endpoint(0),
            data_route,
            CacheLineLayout::new(16).unwrap(),
        ),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_window_depths(4, 4);
    core.set_o3_issue_width(2);
    core.set_o3_memory_issue_width(2);
    core
}

fn loaded_program_store(
    program: PendingLoadGraphProgram,
    target: MemoryTargetId,
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(target, CacheLineLayout::new(16).unwrap())
        .unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();
    let mut image = BootImage::new(Address::new(ROOT_PC));
    for (address, instruction) in instructions(program) {
        image = image
            .add_segment(Address::new(address), instruction.to_le_bytes().to_vec())
            .unwrap();
    }
    image = image
        .add_segment(
            Address::new(ROOT_ADDRESS),
            ROOT_VALUE.to_le_bytes().to_vec(),
        )
        .unwrap();
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn instructions(program: PendingLoadGraphProgram) -> Vec<(u64, u32)> {
    let root_register = program.root_register();
    let first_destination = program.first_destination_register();
    let producers = program.producers();
    vec![
        (ROOT_PC, ld(root_register, 2, 0)),
        (FIRST_LOAD_PC, ld(first_destination, producers[0], 8)),
        (
            FIRST_LOAD_PC + 4,
            ld(first_destination + 1, producers[1], 8),
        ),
        (
            FIRST_LOAD_PC + 8,
            ld(first_destination + 2, producers[2], 8),
        ),
        (FIRST_LOAD_PC + 12, 0x0000_0073),
    ]
}

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static
{
    move |delivery, _context| match store.lock().unwrap().respond(delivery.request()) {
        Ok(outcome) => outcome
            .response()
            .cloned()
            .map(TargetOutcome::Respond)
            .unwrap_or(TargetOutcome::NoResponse),
        Err(_) => TargetOutcome::NoResponse,
    }
}

fn pending_load_graph_driver() -> RiscvSystemRunDriver {
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    controller
        .lock()
        .unwrap()
        .executor_mut()
        .set_execution_mode(
            riscv_execution_mode_target_for_cpu(CpuId::new(0)),
            ExecutionMode::Detailed,
        );
    let host_port =
        SystemHostEventPort::with_controller(PartitionId::new(3), 2, controller).unwrap();
    RiscvSystemRunDriver::new(RiscvTrapEventPort::new(host_port, GuestSourceId::new(601)))
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn fetch_endpoint(cpu: u32) -> TransportEndpointId {
    TransportEndpointId::new(format!("cpu{cpu}.ifetch")).unwrap()
}

fn data_endpoint(cpu: u32) -> TransportEndpointId {
    TransportEndpointId::new(format!("cpu{cpu}.dmem")).unwrap()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn root_range() -> AddressRange {
    AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap()).unwrap()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(offset, rs1, 0x3, rd, 0x03)
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

pub fn corrupt_second_row_destination(live: &mut RiscvO3LiveCheckpointPayload) {
    let destination = live.pending_addresses[1]
        .destination
        .expect("second pending load destination");
    live.pending_addresses[1].destination = Some(rem6_cpu::O3RenameMapEntry::new(
        destination.register_class(),
        destination.architectural(),
        O3PhysicalRegisterId::new(destination.physical().get() + 8),
    ));
}
