use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction,
    RiscvCpuError, RiscvDataAccessEventKind, RiscvLoadReservation,
};
use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, Register, RiscvFenceSet, RiscvInstruction, RiscvMemoryOrdering,
    RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmpAccessKind, RiscvPmpAddressMode,
    RiscvPmpConfig, RiscvPmpError, RiscvPrivilegeMode, RiscvTrap, RiscvTrapKind,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
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

fn fence_type(mode: u32, predecessor: u32, successor: u32, funct3: u32) -> u32 {
    (mode << 28) | (predecessor << 24) | (successor << 20) | (funct3 << 12) | 0x0f
}

fn locked_tor_without_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor).with_locked(true)
}

fn tor_with_all_permissions() -> RiscvPmpConfig {
    RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
        .with_read(true)
        .with_write(true)
        .with_execute(true)
}

fn j_type(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 0x1) << 31)
        | (((imm >> 1) & 0x3ff) << 21)
        | (((imm >> 11) & 0x1) << 20)
        | (((imm >> 12) & 0xff) << 12)
        | (u32::from(rd) << 7)
        | 0x6f
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn data_read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(99), sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn read_store_bytes(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    size: u64,
    sequence: u64,
) -> Vec<u8> {
    store
        .lock()
        .unwrap()
        .respond(&data_read(address, size, sequence))
        .unwrap()
        .response()
        .unwrap()
        .data()
        .unwrap()
        .to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

type AtomicBinary = fn(u64, u64) -> u64;
type LogicalAmoCase = (u32, AtomicBinary);
type WordAmoCase = (u32, u32, u32);

fn sign_extend_word(raw: u32) -> u64 {
    i64::from(raw as i32) as u64
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

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::with_data(
        core(fetch_route, entry),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

fn loaded_store(entry: u64, instruction: u32) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
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
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), word(instruction))
        .unwrap()
        .add_segment(Address::new(data_address), data)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_program_store(
    entry: u64,
    instructions: &[u32],
    data_segments: &[(u64, Vec<u8>)],
) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x3000).unwrap(),
        )
        .unwrap();

    let mut instruction_bytes = Vec::new();
    for instruction in instructions {
        instruction_bytes.extend(word(*instruction));
    }
    let mut image = BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), instruction_bytes)
        .unwrap();
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.clone())
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn data_routes() -> (
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

fn fetch_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch(scheduler, transport, trace, move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap();
    scheduler.run_until_idle_conservative();
}

fn fetch_one_parallel(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    core.issue_next_fetch_parallel(scheduler, transport, trace, move |delivery, context| {
        assert_eq!(context.partition(), PartitionId::new(1));
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap();
    scheduler.run_until_idle_parallel().unwrap();
}

fn drive_one_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> Option<RiscvCoreDriveAction> {
    let fetch_store = store.clone();
    let data_store = store;
    core.drive_next_action(
        scheduler,
        transport,
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
        move |delivery, _context| {
            let response = data_store
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
}

fn issue_one_data_access(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) {
    let _ = issue_one_data_access_with_request_operations(core, store, scheduler, transport, trace);
}

fn issue_one_data_access_with_request_operations(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    trace: MemoryTrace,
) -> Vec<MemoryOperation> {
    let operations = Arc::new(Mutex::new(Vec::new()));
    let observed_operations = operations.clone();
    core.issue_next_data_access(scheduler, transport, trace, move |delivery, _context| {
        observed_operations
            .lock()
            .unwrap()
            .push(delivery.request().operation());
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    })
    .unwrap()
    .unwrap();
    scheduler.run_until_idle_conservative();
    let recorded_operations = operations.lock().unwrap().clone();
    recorded_operations
}

#[test]
fn riscv_core_driver_sequences_fetch_execute_load_and_next_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(7, 0, 0x0, 1, 0x13),
            i_type(8, 2, 0x3, 5, 0x03),
            i_type(9, 0, 0x0, 6, 0x13),
        ],
        &[(0x9008, vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11])],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );
    scheduler.run_until_idle_conservative();

    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(first) = action else {
        panic!("expected completed instruction execution");
    };
    assert_eq!(
        first.instruction(),
        RiscvInstruction::decode(i_type(7, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 7);
    assert_eq!(core.pc(), Address::new(0x8004));

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(load) = action else {
        panic!("expected completed load execution");
    };
    assert!(matches!(
        load.execution().memory_access(),
        Some(MemoryAccessKind::Load { .. })
    ));
    assert_eq!(core.read_register(reg(5)), 0);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );
    scheduler.run_until_idle_conservative();
    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);

    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_executes_fence_barriers_without_data_requests() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_program_store(
        0x8000,
        &[
            fence_type(0, 0b1010, 0b0101, 0x0),
            fence_type(0, 0, 0, 0x1),
            i_type(9, 0, 0x0, 6, 0x13),
        ],
        &[],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(fence) = action else {
        panic!("expected fence execution");
    };
    assert_eq!(
        fence.instruction(),
        RiscvInstruction::Fence {
            predecessor: RiscvFenceSet::new(true, false, true, false),
            successor: RiscvFenceSet::new(false, true, false, true),
            mode: 0,
        }
    );
    assert_eq!(fence.execution().memory_access(), None);
    assert_eq!(core.data_access_events(), &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(fence_i) = action else {
        panic!("expected fence.i execution");
    };
    assert_eq!(fence_i.instruction(), RiscvInstruction::FenceI);
    assert_eq!(fence_i.execution().memory_access(), None);
    assert_eq!(core.data_access_events(), &[]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(addi) = action else {
        panic!("expected addi execution");
    };
    assert_eq!(
        addi.instruction(),
        RiscvInstruction::decode(i_type(9, 0, 0x0, 6, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(6)), 9);
}

#[test]
fn riscv_core_driver_waits_for_store_response_before_next_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 0x1122_3344_5566_7788);
    let store = loaded_program_store(
        0x8000,
        &[s_type(8, 3, 2, 0x3, 0x23), i_type(4, 0, 0x0, 4, 0x13)],
        &[(0x9000, vec![0; 16])],
    );

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::InstructionExecuted(_))
    ));
    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::DataAccessIssued { .. })
    ));
    assert_eq!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        None
    );

    scheduler.run_until_idle_conservative();
    let line = store
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x9000))
        .unwrap();
    assert_eq!(
        &line[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
    );
    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_pmp_rejects_locked_physical_data_load_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_pmp_addr(0, 0x8800 >> 2).unwrap();
    core.write_pmp_config(0, tor_with_all_permissions())
        .unwrap();
    core.write_pmp_addr(1, 0xa000 >> 2).unwrap();
    core.write_pmp_config(1, locked_tor_without_permissions())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMP-denied data load must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmpAccess {
            fetch,
            error: RiscvPmpError::AccessDenied {
                address: 0x9008,
                size: 8,
                kind: RiscvPmpAccessKind::Read,
                privilege: RiscvPrivilegeMode::Machine,
                matched_entry: Some(1),
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_unissued_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_pma_rejects_misaligned_physical_data_load_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9001);
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x3, 5, 0x03),
        0x9001,
        vec![0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMA-denied data load must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmaAccess {
            fetch,
            error: RiscvPmaError::MisalignedDataAccess {
                address: 0x9001,
                size: 8,
                kind: RiscvPmaAccessKind::Read,
            },
        } if fetch == execution.fetch().request_id()
    ));
    assert!(core.data_access_events().is_empty());
    assert!(core.has_unissued_data_access());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_pma_allows_misaligned_physical_data_load_inside_supported_region() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9001);
    core.add_pma_misaligned_range(RiscvPmaRange::new(0x9000, 0x9100).unwrap())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x3, 5, 0x03),
        0x9001,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    assert_eq!(
        core.data_access_events()
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
}

#[test]
fn riscv_core_pma_marks_uncacheable_data_load_requests_strict_order() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9100).unwrap())
        .unwrap();
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );
    let data_store = store.clone();

    fetch_one(&core, store, &mut scheduler, &transport, MemoryTrace::new());
    core.execute_next_completed_fetch().unwrap().unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            assert!(delivery.request().is_uncacheable());
            assert!(delivery.request().is_strict_ordered());
            let response = data_store
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

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
}

#[test]
fn riscv_core_pma_marks_uncacheable_instruction_fetch_requests_strict_order() {
    let (mut scheduler, transport, fetch_route, _data_route) = data_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x8000, 0x9000).unwrap())
        .unwrap();
    let store = loaded_store(0x8000, i_type(5, 0, 0x0, 1, 0x13));

    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            assert!(delivery.request().is_uncacheable());
            assert!(delivery.request().is_strict_ordered());
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

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(event.fetch_pc(), Address::new(0x8000));
}

#[test]
fn riscv_core_records_system_trap_and_stops_issuing_fetches() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let store = loaded_store(0x8000, 0x0000_0073);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("expected trap execution event");
    };

    assert_eq!(
        event.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );
    assert!(core.has_pending_trap());
    assert_eq!(core.pc(), Address::new(0));
    assert_eq!(
        drive_one_action(
            &core,
            Arc::new(Mutex::new(PartitionedMemoryStore::new())),
            &mut scheduler,
            &transport,
        ),
        None
    );
    assert!(scheduler.is_idle());
}

#[test]
fn riscv_core_executes_completed_fetch_and_updates_registers() {
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
    let core = RiscvCore::new(core(route, 0x8000));
    let trace = MemoryTrace::new();

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(5, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        trace,
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(i_type(5, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 5);
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
    assert_eq!(core.execution_events(), vec![event]);
}

#[test]
fn riscv_core_pmp_rejects_locked_instruction_fetch_before_memory_issue() {
    let (mut scheduler, transport, fetch_route, _data_route) = data_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.write_pmp_addr(0, 0x9000 >> 2).unwrap();
    core.write_pmp_config(0, locked_tor_without_permissions())
        .unwrap();

    let error = core
        .issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMP-denied fetch must not issue to memory"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::FetchPmpAccess {
            pc,
            error: RiscvPmpError::AccessDenied {
                address: 0x8000,
                size: 4,
                kind: RiscvPmpAccessKind::Execute,
                privilege: RiscvPrivilegeMode::Machine,
                matched_entry: Some(0),
            },
        } if pc == Address::new(0x8000)
    ));
    assert!(scheduler.is_idle());
    assert_eq!(core.pc(), Address::new(0x8000));
    assert!(core.inner().fetch_events().is_empty());
}

#[test]
fn riscv_core_executes_completed_parallel_fetch_and_updates_registers() {
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
    let core = RiscvCore::new(core(route, 0x8000));
    let trace = MemoryTrace::new();

    fetch_one_parallel(
        &core,
        loaded_store(0x8000, i_type(5, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        trace,
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x8000));
    assert_eq!(
        event.instruction(),
        RiscvInstruction::decode(i_type(5, 0, 0x0, 1, 0x13)).unwrap()
    );
    assert_eq!(core.read_register(reg(1)), 5);
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
    assert_eq!(core.execution_events(), vec![event]);
}

#[test]
fn riscv_core_redirects_cpu_fetch_pc_after_control_flow() {
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
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(
        &core,
        loaded_store(0x8000, j_type(16, 0)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.execution().next_pc(), 0x8010);
    assert_eq!(core.pc(), Address::new(0x8010));
    assert_eq!(core.inner().pc(), Address::new(0x8010));
}

#[test]
fn riscv_core_reports_load_store_accesses_without_memory_side_effects() {
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
    let core = RiscvCore::new(core(route, 0x8000));
    core.write_register(reg(2), 0x9000);

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(8, 2, 0x3, 5, 0x03)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            signed: true,
        })
    );
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_issues_load_access_and_updates_register_after_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(8, 2, 0x3, 5, 0x03),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(core.read_register(reg(5)), 0);

    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0x1122_3344_5566_7788);
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed
        ]
    );
    assert_eq!(events[0].request_id().sequence(), 1);
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::Load {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            signed: true,
        }
    );
    assert_eq!(events[0].operation(), MemoryOperation::ReadShared);
    assert_eq!(
        events[1].data(),
        Some(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11][..])
    );
}

#[test]
fn riscv_core_issues_load_reserved_and_records_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x02, true, false, 0, 2, 0x3, 5),
        0x9008,
        vec![0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        })
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.load_reservation(), None);

    let delivered_operations = issue_one_data_access_with_request_operations(
        &core,
        store,
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(5)), 0x90ab_cdef_1234_5678);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed
        ]
    );
    assert_eq!(delivered_operations, vec![MemoryOperation::LoadLocked]);
    assert_eq!(events[0].operation(), MemoryOperation::LoadLocked);
    assert_eq!(
        events[0].access(),
        &MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            acquire: true,
            release: false,
        }
    );
    assert_eq!(
        events[1].data(),
        Some(&[0x78, 0x56, 0x34, 0x12, 0xef, 0xcd, 0xab, 0x90][..])
    );
}

#[test]
fn riscv_core_store_conditional_succeeds_with_matching_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_program_store(
        0x8000,
        &[
            atomic_type(0x02, false, false, 0, 2, 0x3, 5),
            atomic_type(0x03, false, true, 6, 2, 0x3, 7),
        ],
        &[(0x9008, vec![0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88])],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(8).unwrap()
        ))
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        })
    );
    let delivered_operations = issue_one_data_access_with_request_operations(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 40),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(
        delivered_operations,
        vec![MemoryOperation::StoreConditional]
    );
    assert_eq!(events[2].operation(), MemoryOperation::StoreConditional);
    assert_eq!(
        events[2].access(),
        &MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x0102_0304_0506_0708,
            acquire: false,
            release: true,
        }
    );
    assert_eq!(events[3].data(), None);
}

#[test]
fn riscv_core_word_reserved_pair_uses_word_size_and_sign_extends_load() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_8506_0708);
    let store = loaded_program_store(
        0x8000,
        &[
            atomic_type(0x02, true, false, 0, 2, 0x2, 5),
            atomic_type(0x03, false, true, 6, 2, 0x2, 7),
        ],
        &[(0x9008, vec![0xf0, 0xff, 0xff, 0xff, 0xaa, 0xbb, 0xcc, 0xdd])],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::LoadReserved {
            rd: reg(5),
            address: 0x9008,
            width: MemoryWidth::Word,
            acquire: true,
            release: false,
        })
    );
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(5)), 0xffff_ffff_ffff_fff0);
    assert_eq!(
        core.load_reservation(),
        Some(RiscvLoadReservation::new(
            Address::new(0x9008),
            AccessSize::new(4).unwrap()
        ))
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Word,
            value: 0x0102_0304_8506_0708,
            acquire: false,
            release: true,
        })
    );
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 42),
        vec![0x08, 0x07, 0x06, 0x85, 0xaa, 0xbb, 0xcc, 0xdd]
    );
}

#[test]
fn riscv_core_store_conditional_fails_without_matching_reservation() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x1112_1314_1516_1718);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x03, true, true, 6, 2, 0x3, 7),
        0x9008,
        vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(
        event.execution().memory_access(),
        Some(&MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9008,
            width: MemoryWidth::Doubleword,
            value: 0x1112_1314_1516_1718,
            acquire: true,
            release: true,
        })
    );

    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.load_reservation(), None);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 41),
        vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00, 0x11]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::ConditionalFailed,
        ]
    );
    assert_eq!(events[0].operation(), MemoryOperation::StoreConditional);
    assert_eq!(events[1].data(), None);
}

#[test]
fn riscv_core_amoswapd_writes_new_value_and_returns_old_value() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x01, true, true, 6, 2, 0x3, 7),
        0x9008,
        vec![0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0x1122_3344_5566_7788);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 42),
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].operation(), MemoryOperation::Atomic);
    assert_eq!(
        events[0].memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), Some(RiscvFenceSet::memory()))
    );
    assert_eq!(
        events[1].memory_ordering(),
        RiscvMemoryOrdering::new(Some(RiscvFenceSet::memory()), Some(RiscvFenceSet::memory()))
    );
    assert_eq!(
        events[1].data(),
        Some(&[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11][..])
    );
}

#[test]
fn riscv_core_amoaddd_writes_sum_and_returns_old_value() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9008);
    core.write_register(reg(6), 0x0102_0304_0506_0708);
    let store = loaded_store_with_data(
        0x8000,
        atomic_type(0x00, false, true, 6, 2, 0x3, 7),
        0x9008,
        vec![8, 9, 10, 11, 12, 13, 14, 15],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert_eq!(core.read_register(reg(7)), 0x0f0e_0d0c_0b0a_0908);
    assert_eq!(
        read_store_bytes(&store, 0x9008, 8, 43),
        0x1010_1010_1010_1010u64.to_le_bytes()
    );
    let events = core.data_access_events();
    assert_eq!(
        events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
        vec![
            RiscvDataAccessEventKind::Issued,
            RiscvDataAccessEventKind::Completed,
        ]
    );
    assert_eq!(events[0].operation(), MemoryOperation::Atomic);
    assert_eq!(events[1].data(), Some(&[8, 9, 10, 11, 12, 13, 14, 15][..]));
}

#[test]
fn riscv_core_logical_amo_ops_write_bitwise_result_and_return_old_value() {
    let cases: [LogicalAmoCase; 3] = [
        (0x04, |old: u64, operand: u64| old ^ operand),
        (0x08, |old: u64, operand: u64| old | operand),
        (0x0c, |old: u64, operand: u64| old & operand),
    ];

    for (index, (funct5, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        let old = 0xf0f0_0f0f_aaaa_5555u64;
        let operand = 0x0ff0_f00f_5555_3333u64;
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), operand);
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, true, false, 6, 2, 0x3, 7),
            0x9008,
            old.to_le_bytes().to_vec(),
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), old);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 44 + index as u64),
            expected(old, operand).to_le_bytes()
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_min_max_amo_ops_write_selected_value_and_return_old_value() {
    let negative = 0xffff_ffff_ffff_fff0u64;
    let positive = 7u64;
    let cases: [(u32, u64, u64, u64); 4] = [
        (0x10, negative, positive, negative),
        (0x14, negative, positive, positive),
        (0x18, negative, positive, positive),
        (0x1c, negative, positive, negative),
    ];

    for (index, (funct5, old, operand, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), operand);
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, false, true, 6, 2, 0x3, 7),
            0x9008,
            old.to_le_bytes().to_vec(),
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), old);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 47 + index as u64),
            expected.to_le_bytes()
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_word_amo_ops_write_word_and_sign_extend_old_value() {
    let cases: [WordAmoCase; 9] = [
        (0x00, 0x20, 0x10),
        (0x01, 0x0000_0007, 0x0000_0007),
        (0x04, 0x0000_0007, 0xffff_fff7),
        (0x08, 0x0000_0007, 0xffff_fff7),
        (0x0c, 0x0000_0007, 0x0000_0000),
        (0x10, 0x0000_0007, 0xffff_fff0),
        (0x14, 0x0000_0007, 0x0000_0007),
        (0x18, 0x0000_0007, 0x0000_0007),
        (0x1c, 0x0000_0007, 0xffff_fff0),
    ];

    for (index, (funct5, operand, expected)) in cases.into_iter().enumerate() {
        let (mut scheduler, transport, fetch_route, data_route) = data_routes();
        let core = data_core(fetch_route, data_route, 0x8000);
        let old = 0xffff_fff0u32;
        core.write_register(reg(2), 0x9008);
        core.write_register(reg(6), u64::from(operand));
        let store = loaded_store_with_data(
            0x8000,
            atomic_type(funct5, true, true, 6, 2, 0x2, 7),
            0x9008,
            vec![0xf0, 0xff, 0xff, 0xff, 0xaa, 0xbb, 0xcc, 0xdd],
        );

        fetch_one(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );
        core.execute_next_completed_fetch().unwrap().unwrap();
        issue_one_data_access(
            &core,
            store.clone(),
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
        );

        assert_eq!(core.read_register(reg(7)), sign_extend_word(old));
        let mut expected_bytes = expected.to_le_bytes().to_vec();
        expected_bytes.extend([0xaa, 0xbb, 0xcc, 0xdd]);
        assert_eq!(
            read_store_bytes(&store, 0x9008, 8, 48 + index as u64),
            expected_bytes
        );
        let events = core.data_access_events();
        assert_eq!(
            events.iter().map(|event| event.kind()).collect::<Vec<_>>(),
            vec![
                RiscvDataAccessEventKind::Issued,
                RiscvDataAccessEventKind::Completed,
            ]
        );
        assert_eq!(events[0].operation(), MemoryOperation::Atomic);
        assert_eq!(events[1].data(), Some(&old.to_le_bytes()[..]));
    }
}

#[test]
fn riscv_core_sign_extends_signed_load_response() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    let store = loaded_store_with_data(
        0x8000,
        i_type(0, 2, 0x2, 5, 0x03),
        0x9000,
        vec![0x00, 0x00, 0x00, 0x80],
    );

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(&core, store, &mut scheduler, &transport, MemoryTrace::new());

    assert_eq!(core.read_register(reg(5)), 0xffff_ffff_8000_0000);
}

#[test]
fn riscv_core_issues_store_access_through_memory_transport() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 0x1122_3344_5566_7788);
    let store = loaded_store_with_data(0x8000, s_type(8, 3, 2, 0x3, 0x23), 0x9000, vec![0; 16]);

    fetch_one(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.execute_next_completed_fetch().unwrap().unwrap();
    issue_one_data_access(
        &core,
        store.clone(),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    let line = store
        .lock()
        .unwrap()
        .line_data(MemoryTargetId::new(0), Address::new(0x9000))
        .unwrap();
    assert_eq!(
        &line[8..16],
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
    );
    let events = core.data_access_events();
    assert_eq!(events[0].operation(), MemoryOperation::Write);
    assert_eq!(events[1].kind(), RiscvDataAccessEventKind::Completed);
    assert_eq!(events[1].data(), None);
}

#[test]
fn riscv_core_does_not_execute_completed_fetch_twice() {
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
    let core = RiscvCore::new(core(route, 0x8000));

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(1, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );

    assert!(core.execute_next_completed_fetch().unwrap().is_some());
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.execution_events().len(), 1);
}

#[test]
fn riscv_core_rejects_pc_mismatch_before_execution() {
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
    let core = RiscvCore::new(core(route, 0x8000));
    core.write_register(reg(1), 1);

    fetch_one(
        &core,
        loaded_store(0x8000, i_type(1, 0, 0x0, 1, 0x13)),
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
    );
    core.redirect_pc(Address::new(0x9000));

    assert_eq!(
        core.execute_next_completed_fetch().unwrap_err(),
        RiscvCpuError::PcMismatch {
            fetch: Address::new(0x8000),
            architectural: Address::new(0x9000),
        }
    );
}
