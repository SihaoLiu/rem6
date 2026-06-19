use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, InOrderPipelineStage, RiscvCore,
    RiscvDataAccessEventKind,
};
use rem6_isa_riscv::{Register, RiscvInstruction};
use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::{MmioAccess, MmioBus, MmioRegisterBank, MmioRoute};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
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
    let store = loaded_program(0x8000, &[first_raw, second_raw]);

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one(&core, store, &mut scheduler, &transport);

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
fn riscv_late_completed_fetch_does_not_retire_older_pipeline_work_without_event() {
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
        vec![(1, InOrderPipelineStage::Commit)]
    );

    fetch_one(&core, store, &mut scheduler, &transport);

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
        vec![
            (1, InOrderPipelineStage::Commit),
            (2, InOrderPipelineStage::Fetch1)
        ]
    );
    assert_eq!(
        second_record
            .after()
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(2, InOrderPipelineStage::Fetch2)]
    );
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
    assert_eq!(
        events[0].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(record.cycle(), 4 + data_wait_cycles);
    assert_eq!(record.summary().stall_cycle_count(), data_wait_cycles);
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(record.summary().advanced_count(), 1);
    assert!(record.after().in_flight().is_empty());

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
    assert_eq!(
        events[0].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(record.cycle(), 4 + data_wait_cycles);
    assert_eq!(record.before().cycle(), 4 + data_wait_cycles);
    assert_eq!(record.summary().stall_cycle_count(), data_wait_cycles);
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
    assert_eq!(snapshot.cycle(), 5 + data_wait_cycles);
    assert!(snapshot.in_flight().is_empty());
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
    assert_eq!(
        events[1].in_order_pipeline_data_wait_cycles(),
        data_wait_cycles
    );
    assert_eq!(
        record.cycle(),
        9 + load_reserved_wait_cycles + data_wait_cycles
    );
    assert_eq!(record.summary().retired_count(), 1);
    assert_eq!(
        core.in_order_pipeline_snapshot().cycle(),
        10 + load_reserved_wait_cycles + data_wait_cycles
    );
}
