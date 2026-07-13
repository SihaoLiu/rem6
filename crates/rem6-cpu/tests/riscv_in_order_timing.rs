use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, InOrderPipelineConfig,
    InOrderPipelineCycleRecord, InOrderPipelineInstruction, InOrderPipelineSnapshot,
    InOrderPipelineStage, InOrderPipelineStageWidth, InOrderPipelineStallCause, RiscvCluster,
    RiscvCore, RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
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

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32) -> u32 {
    let raw = imm as u32;
    ((raw >> 12) & 0x1) << 31
        | ((raw >> 5) & 0x3f) << 25
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((raw >> 1) & 0xf) << 8
        | ((raw >> 11) & 0x1) << 7
        | 0x63
}

fn atomic_type(funct5: u32, aq: bool, rl: bool, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(aq) << 26)
        | (u32::from(rl) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn core(route: rem6_transport::MemoryRouteId, entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            endpoint("cpu0.ifetch"),
            route,
            layout(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn data_core(
    fetch_route: rem6_transport::MemoryRouteId,
    data_route: rem6_transport::MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn uniform_in_order_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program(entry, &[instruction])
}

fn loaded_program(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    let program = instructions
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect::<Vec<_>>();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), program)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_store_with_data(
    entry: u64,
    instruction: u32,
    data_address: u64,
    data: Vec<u8>,
) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program_with_data(entry, &[instruction], data_address, data)
}

fn loaded_program_with_data(
    entry: u64,
    instructions: &[u32],
    data_address: u64,
    data: Vec<u8>,
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    let program = instructions
        .iter()
        .flat_map(|instruction| instruction.to_le_bytes())
        .collect::<Vec<_>>();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), program)
        .unwrap()
        .add_segment(Address::new(data_address), data)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    let response = store
        .lock()
        .unwrap()
        .respond(delivery.request())
        .unwrap()
        .response()
        .cloned()
        .unwrap();
    TargetOutcome::Respond(response)
}

fn in_order_routes() -> (
    PartitionedScheduler,
    MemoryTransport,
    rem6_transport::MemoryRouteId,
    rem6_transport::MemoryRouteId,
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

fn fetch_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
}

fn fetch_one_parallel(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_fetch_parallel(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, context| {
            assert_eq!(context.partition(), PartitionId::new(1));
            memory_response(&store, &delivery)
        },
    )
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
}

fn issue_one_data_access(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            let response = store
                .lock()
                .unwrap()
                .respond(delivery.request())
                .unwrap()
                .response()
                .cloned()
                .unwrap();
            TargetOutcome::Respond(response)
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();
}

fn last_data_wait_cycles(core: &RiscvCore, completion_kind: RiscvDataAccessEventKind) -> Tick {
    let data_events = core.data_access_events();
    let completed_index = data_events
        .iter()
        .rposition(|event| event.kind() == completion_kind)
        .unwrap();
    let completed_request = data_events[completed_index].request_id();
    let issued_tick = data_events[..completed_index]
        .iter()
        .rfind(|event| {
            event.kind() == RiscvDataAccessEventKind::Issued
                && event.request_id() == completed_request
        })
        .unwrap()
        .tick();
    let completed_tick = data_events[completed_index].tick();
    completed_tick.saturating_sub(issued_tick)
}

fn assert_explicit_data_wait_stall_records(
    core: &RiscvCore,
    sequence: u64,
    data_wait_cycles: Tick,
) {
    let cycle_records = core.in_order_pipeline_cycle_records();
    assert!(cycle_records
        .windows(2)
        .all(|records| records[0].cycle() < records[1].cycle()));
    let wait_records = cycle_records
        .iter()
        .filter(|cycle| {
            cycle.summary().retired_count() == 0 && cycle.summary().stall_cycle_count() == 1
        })
        .filter(|cycle| {
            cycle.before().in_flight().iter().any(|instruction| {
                instruction.sequence() == sequence
                    && instruction.stage() == InOrderPipelineStage::Execute
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(wait_records.len() as u64, data_wait_cycles);
    if let Some(first_wait) = wait_records.first() {
        for (offset, record) in wait_records.iter().enumerate() {
            assert_eq!(record.cycle(), first_wait.cycle() + offset as u64);
        }
    }
    assert_eq!(
        cycle_records
            .iter()
            .filter(|cycle| {
                cycle
                    .plan()
                    .advanced()
                    .iter()
                    .any(|advance| advance.sequence() == sequence && advance.retires())
            })
            .count(),
        1
    );
}

fn mmio_bus_with_u64(address: u64, bytes: [u8; 8]) -> MmioBus {
    let mut bank =
        MmioRegisterBank::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        8,
        AccessSize::new(8).unwrap(),
        MmioAccess::ReadOnly,
        bytes.to_vec(),
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
        Mutex::new(bank),
    )
    .unwrap();
    bus
}

fn drive_next_serial_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> RiscvCoreDriveAction {
    let fetch_store = Arc::clone(&store);
    core.drive_next_action(
        scheduler,
        transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        move |delivery, _context| memory_response(&fetch_store, &delivery),
        |_delivery, _context| panic!("scalar ALU instruction must not issue data traffic"),
    )
    .unwrap()
    .expect("normal driver should make fetch or pipeline progress")
}

fn drive_next_parallel_cluster_action(
    cluster: &RiscvCluster,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> RiscvCoreDriveAction {
    let actions = cluster
        .drive_ready_cores_parallel(
            scheduler,
            transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| {
                let store = Arc::clone(&store);
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                move |_delivery, _context| {
                    panic!("scalar ALU instruction must not issue data traffic")
                }
            },
        )
        .unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].cpu(), CpuId::new(0));
    actions[0].action().clone()
}

fn sequence_stage(core: &RiscvCore, sequence: u64) -> Option<InOrderPipelineStage> {
    core.in_order_pipeline_snapshot()
        .in_flight()
        .iter()
        .find_map(|instruction| (instruction.sequence() == sequence).then_some(instruction.stage()))
}

fn in_flight_without_sequence(core: &RiscvCore, sequence: u64) -> Vec<InOrderPipelineInstruction> {
    core.in_order_pipeline_snapshot()
        .in_flight()
        .iter()
        .copied()
        .filter(|instruction| instruction.sequence() != sequence)
        .collect()
}

fn pipeline_rows(instructions: &[InOrderPipelineInstruction]) -> Vec<(u64, InOrderPipelineStage)> {
    instructions
        .iter()
        .map(|instruction| (instruction.sequence(), instruction.stage()))
        .collect()
}

fn last_pipeline_record(core: &RiscvCore) -> InOrderPipelineCycleRecord {
    core.in_order_pipeline_cycle_records()
        .last()
        .cloned()
        .expect("pipeline cycle record should have been recorded")
}

fn assert_execute_wait_record(
    record: &InOrderPipelineCycleRecord,
    sequence: u64,
    expected_ordering_blocked: &[InOrderPipelineInstruction],
) {
    assert_eq!(
        record.stall_cause(),
        Some(InOrderPipelineStallCause::ExecuteWait)
    );
    assert_eq!(record.stall_cycle_count(), 1);
    assert_eq!(record.summary().stall_cycle_count(), 1);
    assert_eq!(record.summary().retired_count(), 0);
    assert_eq!(record.summary().advanced_count(), 0);
    assert_eq!(
        pipeline_rows(record.plan().resource_blocked()),
        vec![(sequence, InOrderPipelineStage::Execute)]
    );
    assert_eq!(
        pipeline_rows(record.plan().ordering_blocked()),
        pipeline_rows(expected_ordering_blocked)
    );
    assert_eq!(
        pipeline_rows(record.before().in_flight()),
        pipeline_rows(record.after().in_flight())
    );
}

fn drive_serial_until_stage(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    sequence: u64,
    stage: InOrderPipelineStage,
    suppressed_register: Register,
) {
    for _ in 0..16 {
        if sequence_stage(core, sequence) == Some(stage) {
            return;
        }
        match drive_next_serial_action(core, Arc::clone(&store), scheduler, transport) {
            RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            | RiscvCoreDriveAction::FetchIssued { .. } => {
                scheduler.run_until_idle_conservative();
                assert_eq!(core.read_register(suppressed_register), 0);
                assert!(core.execution_events().is_empty());
            }
            RiscvCoreDriveAction::InstructionExecuted(event) => {
                panic!("instruction executed before reaching {stage:?}: {event:?}");
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => {
                panic!("scalar ALU instruction unexpectedly issued data traffic");
            }
        }
    }
    panic!("sequence {sequence} did not reach {stage:?}");
}

fn drive_parallel_until_stage(
    cluster: &RiscvCluster,
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    sequence: u64,
    stage: InOrderPipelineStage,
    suppressed_register: Register,
) {
    for _ in 0..16 {
        if sequence_stage(core, sequence) == Some(stage) {
            return;
        }
        match drive_next_parallel_cluster_action(cluster, Arc::clone(&store), scheduler, transport)
        {
            RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            | RiscvCoreDriveAction::FetchIssued { .. } => {
                scheduler.run_until_idle_parallel().unwrap();
                assert_eq!(core.read_register(suppressed_register), 0);
                assert!(core.execution_events().is_empty());
            }
            RiscvCoreDriveAction::InstructionExecuted(event) => {
                panic!("instruction executed before reaching {stage:?}: {event:?}");
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => {
                panic!("scalar ALU instruction unexpectedly issued data traffic");
            }
        }
    }
    panic!("sequence {sequence} did not reach {stage:?}");
}

fn assert_serial_execute_wait_ticks_before_commit(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    sequence: u64,
    wait_cycles: u64,
    suppressed_register: Register,
) {
    let expected_ordering_blocked = in_flight_without_sequence(core, sequence);
    for offset in 0..wait_cycles {
        match drive_next_serial_action(core, Arc::clone(&store), scheduler, transport) {
            RiscvCoreDriveAction::PipelineCycleScheduled { .. } => {}
            other => panic!("execute wait tick {offset} produced unexpected action: {other:?}"),
        }
        scheduler.run_until_idle_conservative();
        assert_eq!(
            sequence_stage(core, sequence),
            Some(InOrderPipelineStage::Execute)
        );
        assert_eq!(core.read_register(suppressed_register), 0);
        assert!(core.execution_events().is_empty());
        assert_execute_wait_record(
            &last_pipeline_record(core),
            sequence,
            &expected_ordering_blocked,
        );
    }
}

fn assert_parallel_execute_wait_ticks_before_commit(
    cluster: &RiscvCluster,
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    sequence: u64,
    wait_cycles: u64,
    suppressed_register: Register,
) {
    let expected_ordering_blocked = in_flight_without_sequence(core, sequence);
    assert!(
        !expected_ordering_blocked.is_empty(),
        "parallel setup should keep a younger row visible behind the stalled Execute instruction"
    );
    for offset in 0..wait_cycles {
        match drive_next_parallel_cluster_action(cluster, Arc::clone(&store), scheduler, transport)
        {
            RiscvCoreDriveAction::PipelineCycleScheduled { .. } => {}
            other => panic!("execute wait tick {offset} produced unexpected action: {other:?}"),
        }
        scheduler.run_until_idle_parallel().unwrap();
        assert_eq!(
            sequence_stage(core, sequence),
            Some(InOrderPipelineStage::Execute)
        );
        assert_eq!(core.read_register(suppressed_register), 0);
        assert!(core.execution_events().is_empty());
        assert_execute_wait_record(
            &last_pipeline_record(core),
            sequence,
            &expected_ordering_blocked,
        );
    }
}

fn drive_next_serial_pipeline_tick(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> InOrderPipelineCycleRecord {
    match drive_next_serial_action(core, store, scheduler, transport) {
        RiscvCoreDriveAction::PipelineCycleScheduled { .. } => {}
        other => panic!("expected scheduled pipeline tick, got {other:?}"),
    }
    scheduler.run_until_idle_conservative();
    last_pipeline_record(core)
}

fn drive_next_parallel_pipeline_tick(
    cluster: &RiscvCluster,
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> InOrderPipelineCycleRecord {
    match drive_next_parallel_cluster_action(cluster, store, scheduler, transport) {
        RiscvCoreDriveAction::PipelineCycleScheduled { .. } => {}
        other => panic!("expected scheduled pipeline tick, got {other:?}"),
    }
    scheduler.run_until_idle_parallel().unwrap();
    last_pipeline_record(core)
}

fn drive_until_serial_instruction_executed(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> rem6_cpu::RiscvCpuExecutionEvent {
    for _ in 0..16 {
        match drive_next_serial_action(core, Arc::clone(&store), scheduler, transport) {
            RiscvCoreDriveAction::InstructionExecuted(event) => return *event,
            RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            | RiscvCoreDriveAction::FetchIssued { .. } => {
                scheduler.run_until_idle_conservative();
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => {
                panic!("scalar ALU instruction unexpectedly issued data traffic")
            }
        }
    }
    panic!("instruction did not execute");
}

fn drive_until_parallel_instruction_executed(
    cluster: &RiscvCluster,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> rem6_cpu::RiscvCpuExecutionEvent {
    for _ in 0..16 {
        match drive_next_parallel_cluster_action(cluster, Arc::clone(&store), scheduler, transport)
        {
            RiscvCoreDriveAction::InstructionExecuted(event) => return *event,
            RiscvCoreDriveAction::PipelineCycleScheduled { .. }
            | RiscvCoreDriveAction::FetchIssued { .. } => {
                scheduler.run_until_idle_parallel().unwrap();
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => {
                panic!("scalar ALU instruction unexpectedly issued data traffic")
            }
        }
    }
    panic!("instruction did not execute");
}

#[test]
fn normal_driver_zero_wait_integer_add_has_no_execute_wait_before_commit() {
    let (mut scheduler, transport, fetch_route, _) = in_order_routes();
    let add = r_type(0x00, 2, 1, 0x0, 3, 0x33);
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.write_register(reg(1), 2);
    core.write_register(reg(2), 3);
    let store = loaded_program(0x8000, &[add, 0x0010_0073]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    drive_serial_until_stage(
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
        0,
        InOrderPipelineStage::Execute,
        reg(3),
    );

    let execute_to_commit =
        drive_next_serial_pipeline_tick(&core, Arc::clone(&store), &mut scheduler, &transport);
    assert_eq!(core.read_register(reg(3)), 0);
    assert_eq!(sequence_stage(&core, 0), Some(InOrderPipelineStage::Commit));
    assert_eq!(execute_to_commit.stall_cause(), None);
    assert_eq!(execute_to_commit.summary().stall_cycle_count(), 0);
    assert!(execute_to_commit.plan().advanced().iter().any(|advance| {
        advance.sequence() == 0
            && advance.source_stage() == InOrderPipelineStage::Execute
            && advance.destination_stage() == Some(InOrderPipelineStage::Commit)
    }));
    assert!(core
        .in_order_pipeline_cycle_records()
        .iter()
        .all(|record| record.stall_cause() != Some(InOrderPipelineStallCause::ExecuteWait)));

    let event = drive_until_serial_instruction_executed(&core, store, &mut scheduler, &transport);
    assert_eq!(event.instruction(), RiscvInstruction::decode(add).unwrap());
    assert_eq!(core.read_register(reg(3)), 5);
    assert_eq!(
        event
            .in_order_pipeline_cycle()
            .unwrap()
            .summary()
            .retired_count(),
        1
    );
}

#[test]
fn normal_driver_serial_mul_execute_wait_stalls_before_commit() {
    let (mut scheduler, transport, fetch_route, _) = in_order_routes();
    let mul = r_type(0x01, 2, 1, 0x0, 3, 0x33);
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.write_register(reg(1), 6);
    core.write_register(reg(2), 7);
    let store = loaded_program(0x8000, &[mul, 0x0010_0073]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    drive_serial_until_stage(
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
        0,
        InOrderPipelineStage::Execute,
        reg(3),
    );

    assert_serial_execute_wait_ticks_before_commit(
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
        0,
        2,
        reg(3),
    );

    let execute_to_commit =
        drive_next_serial_pipeline_tick(&core, Arc::clone(&store), &mut scheduler, &transport);
    assert_eq!(execute_to_commit.stall_cause(), None);
    assert_eq!(sequence_stage(&core, 0), Some(InOrderPipelineStage::Commit));
    assert_eq!(core.read_register(reg(3)), 0);
    assert!(core.execution_events().is_empty());

    let event = drive_until_serial_instruction_executed(&core, store, &mut scheduler, &transport);
    assert_eq!(event.instruction(), RiscvInstruction::decode(mul).unwrap());
    assert_eq!(core.read_register(reg(3)), 42);
    let retire_record = event.in_order_pipeline_cycle().unwrap();
    assert_eq!(retire_record.stall_cause(), None);
    assert_eq!(retire_record.summary().retired_count(), 1);
}

#[test]
fn normal_cluster_parallel_div_execute_wait_stalls_oldest_and_orders_younger() {
    let (mut scheduler, transport, fetch_route, _) = in_order_routes();
    let div = r_type(0x01, 2, 1, 0x4, 3, 0x33);
    let younger_addi = i_type(9, 0, 0x0, 4, 0x13);
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 7);
    let store = loaded_program(0x8000, &[div, younger_addi, 0x0010_0073]);

    fetch_one_parallel(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one_parallel(&core, Arc::clone(&store), &mut scheduler, &transport);
    drive_parallel_until_stage(
        &cluster,
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
        0,
        InOrderPipelineStage::Execute,
        reg(3),
    );
    assert!(!in_flight_without_sequence(&core, 0).is_empty());

    assert_parallel_execute_wait_ticks_before_commit(
        &cluster,
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
        0,
        19,
        reg(3),
    );

    let execute_to_commit = drive_next_parallel_pipeline_tick(
        &cluster,
        &core,
        Arc::clone(&store),
        &mut scheduler,
        &transport,
    );
    assert_eq!(execute_to_commit.stall_cause(), None);
    assert_eq!(sequence_stage(&core, 0), Some(InOrderPipelineStage::Commit));
    assert_eq!(core.read_register(reg(3)), 0);
    assert_eq!(core.read_register(reg(4)), 0);
    assert!(core.execution_events().is_empty());

    let event =
        drive_until_parallel_instruction_executed(&cluster, store, &mut scheduler, &transport);
    assert_eq!(event.instruction(), RiscvInstruction::decode(div).unwrap());
    assert_eq!(core.read_register(reg(3)), 12);
    assert_eq!(core.read_register(reg(4)), 0);
    let retire_record = event.in_order_pipeline_cycle().unwrap();
    assert_eq!(retire_record.stall_cause(), None);
    assert_eq!(retire_record.summary().retired_count(), 1);
}

#[test]
fn normal_driver_schedules_pipeline_stages_before_architectural_execution() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw = i_type(7, 0, 0x0, 1, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));
    let store = loaded_program(0x8000, &[raw, 0x0010_0073]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);

    let mut scheduled_cycles = 0;
    while core
        .in_order_pipeline_snapshot()
        .in_flight()
        .iter()
        .find(|instruction| instruction.sequence() == 0)
        .is_none_or(|instruction| instruction.stage() != InOrderPipelineStage::Commit)
    {
        let fetch_store = Arc::clone(&store);
        let action = core
            .drive_next_action(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                move |delivery, _context| {
                    let response = fetch_store
                        .lock()
                        .unwrap()
                        .respond(delivery.request())
                        .unwrap()
                        .response()
                        .cloned()
                        .unwrap();
                    TargetOutcome::Respond(response)
                },
                |_delivery, _context| panic!("integer ALU must not issue data traffic"),
            )
            .unwrap()
            .expect("normal driver should make fetch or pipeline progress");

        match action {
            RiscvCoreDriveAction::FetchIssued { .. } => {}
            RiscvCoreDriveAction::PipelineCycleScheduled { .. } => {
                scheduled_cycles += 1;
                assert_eq!(core.read_register(reg(1)), 0);
                assert!(core.execution_events().is_empty());
            }
            RiscvCoreDriveAction::InstructionExecuted(event) => {
                panic!("instruction executed before commit stage: {event:?}")
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => {
                panic!("integer ALU unexpectedly issued data traffic")
            }
        }
        scheduler.run_until_idle_conservative();
    }

    assert_eq!(scheduled_cycles, 4);
    assert_eq!(core.read_register(reg(1)), 0);
    assert!(core.execution_events().is_empty());
    assert_eq!(
        core.in_order_pipeline_snapshot()
            .in_flight()
            .iter()
            .find(|instruction| instruction.sequence() == 0)
            .map(|instruction| instruction.stage()),
        Some(InOrderPipelineStage::Commit)
    );

    let event = loop {
        let fetch_store = Arc::clone(&store);
        let action = core
            .drive_next_action(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                MemoryTrace::new(),
                move |delivery, _context| {
                    let response = fetch_store
                        .lock()
                        .unwrap()
                        .respond(delivery.request())
                        .unwrap()
                        .response()
                        .cloned()
                        .unwrap();
                    TargetOutcome::Respond(response)
                },
                |_delivery, _context| panic!("integer ALU must not issue data traffic"),
            )
            .unwrap()
            .expect("commit-ready instruction should execute or issue fetch-ahead");
        match action {
            RiscvCoreDriveAction::FetchIssued { .. } => {
                assert_eq!(core.read_register(reg(1)), 0);
                scheduler.run_until_idle_conservative();
            }
            RiscvCoreDriveAction::InstructionExecuted(event) => break event,
            other => panic!("commit-ready instruction made unexpected progress: {other:?}"),
        }
    };
    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(core.read_register(reg(1)), 7);
    assert_eq!(
        event
            .in_order_pipeline_cycle()
            .unwrap()
            .summary()
            .retired_count(),
        1
    );
}

#[test]
fn riscv_retired_instruction_records_in_order_pipeline_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw = i_type(5, 0, 0x0, 1, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    let record = event.in_order_pipeline_cycle().unwrap();
    assert_eq!(record.cycle(), 4);
    assert_eq!(record.before().cycle(), 4);
    assert_eq!(
        record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(0, InOrderPipelineStage::Commit)]
    );
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(record.summary().advanced_count(), 1);
    assert!(record.after().in_flight().is_empty());

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 5);
    assert!(snapshot.in_flight().is_empty());
}

#[test]
fn riscv_pipeline_snapshot_restore_discards_later_cycle_history() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw = i_type(5, 0, 0x0, 1, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));
    let snapshot = core.in_order_pipeline_snapshot();

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert!(event.in_order_pipeline_cycle().is_some());
    assert!(!core.in_order_pipeline_cycle_records().is_empty());

    core.restore_in_order_pipeline_snapshot(snapshot).unwrap();

    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 0);
    assert!(core.in_order_pipeline_cycle_records().is_empty());
}

#[test]
fn riscv_direct_fetch_issue_defers_younger_row_without_advancing_time() {
    let (mut scheduler, transport, fetch_route, _) = in_order_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(1));
    let store = loaded_program(
        0x8000,
        &[i_type(5, 0, 0x0, 1, 0x13), i_type(7, 1, 0x0, 2, 0x13)],
    );

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 0);
    assert_eq!(
        snapshot.in_flight(),
        &[InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1
        )]
    );
    assert!(core.in_order_pipeline_cycle_records().is_empty());

    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(first.fetch_pc(), Address::new(0x8000));
    let second = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(second.fetch_pc(), Address::new(0x8004));
}

#[test]
fn riscv_completed_fetches_overlap_in_order_pipeline_before_retire() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let first_raw = i_type(5, 0, 0x0, 1, 0x13);
    let second_raw = i_type(7, 1, 0x0, 2, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
    let store = loaded_program(0x8000, &[first_raw, second_raw]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one(&core, store, &mut scheduler, &transport);

    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight(),
        &[
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Fetch1),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
        ]
    );
    assert!(core.in_order_pipeline_cycle_records().is_empty());

    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(first_raw).unwrap()
    );
    let first_record = first.in_order_pipeline_cycle().unwrap();
    assert_eq!(first_record.cycle(), 4);
    assert_eq!(
        first_record
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );

    let second = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        second.instruction(),
        RiscvInstruction::decode(second_raw).unwrap()
    );
    let second_record = second.in_order_pipeline_cycle().unwrap();
    assert_eq!(second_record.cycle(), 5);
    assert_eq!(
        second_record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );
    assert!(second_record.after().in_flight().is_empty());
}

#[test]
fn riscv_late_completed_fetch_does_not_advance_older_pipeline_work_without_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let first_raw = i_type(5, 0, 0x0, 1, 0x13);
    let second_raw = i_type(7, 1, 0x0, 2, 0x13);
    let third_raw = i_type(11, 2, 0x0, 3, 0x13);
    let core = RiscvCore::new(core(route, 0x8000));
    let store = loaded_program(0x8000, &[first_raw, second_raw, third_raw]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight(),
        &[InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1
        )]
    );
    assert!(core.in_order_pipeline_cycle_records().is_empty());

    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(first.in_order_pipeline_cycle().unwrap().cycle(), 4);
    assert_eq!(
        first
            .in_order_pipeline_cycle()
            .unwrap()
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        Vec::<(u64, InOrderPipelineStage)>::new()
    );

    fetch_one(&core, store, &mut scheduler, &transport);
    assert_eq!(
        core.in_order_pipeline_snapshot().in_flight(),
        &[InOrderPipelineInstruction::new(
            1,
            InOrderPipelineStage::Fetch1
        )]
    );
    assert_eq!(core.in_order_pipeline_cycle_records().len(), 5);

    let second = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        second.instruction(),
        RiscvInstruction::decode(second_raw).unwrap()
    );
    let second_record = second.in_order_pipeline_cycle().unwrap();
    assert_eq!(second_record.cycle(), 9);
    assert_eq!(
        second_record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );
    assert!(second_record.after().in_flight().is_empty());
}

#[test]
fn riscv_taken_branch_records_in_order_prediction_redirect_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw = b_type(8, 0, 0, 0x0);
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(&core, loaded_store(0x8000, raw), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(core.pc(), Address::new(0x8008));
    let record = event.in_order_pipeline_cycle().unwrap();
    let prediction = &record.branch_predictions()[0];
    assert_eq!(prediction.fetch_pc(), 0x8000);
    assert!(!prediction.predicted_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
    assert!(prediction.mispredicted());
    let summary = record.summary();
    assert_eq!(summary.branch_prediction_count(), 1);
    assert_eq!(summary.branch_misprediction_count(), 1);
    assert_eq!(summary.redirect_target_pc(), Some(0x8008));
}

#[test]
fn riscv_predicted_taken_branch_records_fallthrough_redirect_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw_taken = b_type(8, 0, 0, 0x0);
    let raw_not_taken = b_type(8, 0, 0, 0x1);
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(
        &core,
        loaded_store(0x8000, raw_taken),
        &mut scheduler,
        &transport,
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    core.redirect_pc(Address::new(0x8000));
    fetch_one(
        &core,
        loaded_store(0x8000, raw_not_taken),
        &mut scheduler,
        &transport,
    );
    let event = core
        .execute_next_completed_fetch()
        .expect("fall-through branch retires with a pipeline redirect")
        .unwrap();

    let update = event.branch_update().unwrap();
    assert!(update.predicted_taken());
    assert_eq!(update.predicted_target(), Some(Address::new(0x8008)));
    assert!(!update.actual_taken());
    assert_eq!(update.actual_target(), None);
    assert_eq!(core.pc(), Address::new(0x8004));
    let record = event.in_order_pipeline_cycle().unwrap();
    let prediction = &record.branch_predictions()[0];
    assert_eq!(prediction.fetch_pc(), 0x8000);
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
    assert!(!prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8004));
    assert!(prediction.mispredicted());
    assert_eq!(prediction.repair_target_pc(), Some(0x8004));
    let summary = record.summary();
    assert_eq!(summary.branch_prediction_count(), 1);
    assert_eq!(summary.branch_misprediction_count(), 1);
    assert_eq!(summary.redirect_target_pc(), Some(0x8004));
}

#[test]
fn riscv_completed_mmio_data_access_records_in_order_pipeline_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let route = transport
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
    let raw = i_type(8, 2, 0x3, 5, 0x03);
    let core = RiscvCore::new(core(route, 0x8000));
    core.write_register(reg(2), 0x1000);
    let store = loaded_store(0x8000, raw);
    let bus = mmio_bus_with_u64(0x1000, [0x21, 0x43, 0x65, 0x87, 0xa9, 0xcb, 0xed, 0x0f]);

    fetch_one(&core, store, &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert!(event.in_order_pipeline_cycle().is_none());
    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 0);
    assert_eq!(core.read_register(reg(5)), 0);

    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .unwrap();
    scheduler.run_until_idle_parallel().unwrap();

    assert_eq!(core.read_register(reg(5)), 0x0fed_cba9_8765_4321);
    let data_wait_cycles = last_data_wait_cycles(&core, RiscvDataAccessEventKind::Completed);
    assert!(data_wait_cycles > 0);
    let events = core.execution_events();
    assert_eq!(events.len(), 1);
    let record = events[0].in_order_pipeline_cycle().unwrap();
    let sequence = events[0].fetch().request_id().sequence();
    assert_eq!(
        events[0].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(record.cycle(), 4 + data_wait_cycles);
    assert_eq!(record.summary().stall_cycle_count(), 0);
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(record.summary().advanced_count(), 1);
    assert!(record.after().in_flight().is_empty());

    assert_explicit_data_wait_stall_records(&core, sequence, data_wait_cycles);

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 5 + data_wait_cycles);
    assert!(snapshot.in_flight().is_empty());
}

#[test]
fn riscv_completed_data_access_records_in_order_pipeline_cycle() {
    let (mut scheduler, transport, fetch_route, data_route) = in_order_routes();
    let raw = i_type(8, 2, 0x3, 5, 0x03);
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        raw,
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(&core, store.clone(), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert!(event.in_order_pipeline_cycle().is_none());
    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 0);
    assert_eq!(core.read_register(reg(5)), 0);

    issue_one_data_access(&core, store, &mut scheduler, &transport);

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    let data_wait_cycles = last_data_wait_cycles(&core, RiscvDataAccessEventKind::Completed);
    assert!(data_wait_cycles > 0);
    let events = core.execution_events();
    assert_eq!(events.len(), 1);
    let record = events[0].in_order_pipeline_cycle().unwrap();
    let sequence = events[0].fetch().request_id().sequence();
    assert_eq!(
        events[0].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(record.cycle(), 4 + data_wait_cycles);
    assert_eq!(record.before().cycle(), 4 + data_wait_cycles);
    assert_eq!(record.summary().stall_cycle_count(), 0);
    assert_eq!(
        record
            .before()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(sequence, InOrderPipelineStage::Commit)]
    );
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(record.summary().advanced_count(), 1);
    assert!(record.after().in_flight().is_empty());

    assert_explicit_data_wait_stall_records(&core, sequence, data_wait_cycles);

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 5 + data_wait_cycles);
    assert!(snapshot.in_flight().is_empty());
}

#[test]
fn riscv_completed_data_access_records_wait_when_instruction_already_in_execute() {
    let (mut scheduler, transport, fetch_route, data_route) = in_order_routes();
    let raw = i_type(8, 2, 0x3, 5, 0x03);
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        raw,
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(&core, store.clone(), &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert!(event.in_order_pipeline_cycle().is_none());
    let sequence = event.fetch().request_id().sequence();
    let config = core.in_order_pipeline_snapshot().config().clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        4,
        [InOrderPipelineInstruction::new(
            sequence,
            InOrderPipelineStage::Execute,
        )],
    ))
    .unwrap();

    issue_one_data_access(&core, store, &mut scheduler, &transport);

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    let data_wait_cycles = last_data_wait_cycles(&core, RiscvDataAccessEventKind::Completed);
    assert!(data_wait_cycles > 0);
    let events = core.execution_events();
    let record = events[0].in_order_pipeline_cycle().unwrap();
    assert_eq!(
        events[0].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(record.summary().stall_cycle_count(), 0);
    assert_eq!(record.summary().retired_count(), 1);
    assert_explicit_data_wait_stall_records(&core, sequence, data_wait_cycles);
}

#[test]
fn riscv_local_store_conditional_failure_records_in_order_pipeline_cycle() {
    let (mut scheduler, transport, fetch_route, data_route) = in_order_routes();
    let raw = atomic_type(0x03, false, true, 6, 2, 0x3, 7);
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(6), 0x1122_3344_5566_7788);
    let store = loaded_store_with_data(
        0x8000,
        raw,
        0x9000,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(&core, store, &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert!(event.in_order_pipeline_cycle().is_none());
    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 0);

    core.issue_next_data_access(&mut scheduler, &transport, MemoryTrace::new(), |_, _| {
        unreachable!("local store-conditional failure does not issue a memory request")
    })
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(7)), 1);
    let events = core.execution_events();
    let record = events[0].in_order_pipeline_cycle().unwrap();
    assert_eq!(record.cycle(), 4);
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(core.in_order_pipeline_snapshot().cycle(), 5);
}

#[test]
fn riscv_response_store_conditional_failure_records_in_order_pipeline_cycle() {
    let (mut scheduler, transport, fetch_route, data_route) = in_order_routes();
    let load_reserved = atomic_type(0x02, false, false, 0, 2, 0x3, 5);
    let store_conditional = atomic_type(0x03, false, true, 6, 2, 0x3, 7);
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(6), 0x1122_3344_5566_7788);
    let store = loaded_program_with_data(
        0x8000,
        &[load_reserved, store_conditional],
        0x9000,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(&core, store.clone(), &mut scheduler, &transport);
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(&core, store.clone(), &mut scheduler, &transport);

    assert!(core.load_reservation().is_some());
    let load_reserved_wait_cycles =
        last_data_wait_cycles(&core, RiscvDataAccessEventKind::Completed);
    assert_eq!(
        core.in_order_pipeline_snapshot().cycle(),
        5 + load_reserved_wait_cycles
    );

    fetch_one(&core, store, &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(store_conditional).unwrap()
    );
    assert!(event.in_order_pipeline_cycle().is_none());

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::store_conditional_failed(delivery.request()).unwrap(),
            )
        },
    )
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.load_reservation(), None);
    let data_wait_cycles =
        last_data_wait_cycles(&core, RiscvDataAccessEventKind::ConditionalFailed);
    assert!(data_wait_cycles > 0);
    let events = core.execution_events();
    let record = events[1].in_order_pipeline_cycle().unwrap();
    let sequence = events[1].fetch().request_id().sequence();
    assert_eq!(
        events[1].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(
        record.cycle(),
        9 + load_reserved_wait_cycles + data_wait_cycles
    );
    assert_eq!(record.summary().retired_count(), 1);
    assert_explicit_data_wait_stall_records(&core, sequence, data_wait_cycles);
    assert_eq!(
        core.in_order_pipeline_snapshot().cycle(),
        10 + load_reserved_wait_cycles + data_wait_cycles
    );
}
