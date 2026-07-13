use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::{Register, RiscvHartState, RiscvPrivilegeMode};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
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

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn u_type(imm: i32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32) & 0xffff_f000) | (u32::from(rd) << 7) | opcode
}

fn core(route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::new(
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
        .unwrap(),
    )
}

fn data_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId, entry: u64) -> RiscvCore {
    RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, layout()),
    )
}

fn routes() -> (PartitionedScheduler, MemoryTransport, MemoryRouteId) {
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
    (scheduler, transport, fetch_route)
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

fn loaded_program(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
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

    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for instruction in instructions {
        bytes.extend(instruction.to_le_bytes());
    }
    BootImage::new(Address::new(entry))
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn loaded_program_bytes(entry: u64, bytes: Vec<u8>) -> Arc<Mutex<PartitionedMemoryStore>> {
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
        .add_segment(Address::new(entry), bytes)
        .unwrap()
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn drive_one_action(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) -> Option<RiscvCoreDriveAction> {
    let fetch_store = store.clone();
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
}

fn retire_one(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    assert!(matches!(
        drive_one_action(core, store.clone(), scheduler, transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    for _ in 0..8 {
        match drive_one_action(core, store.clone(), scheduler, transport) {
            Some(RiscvCoreDriveAction::InstructionExecuted(_)) => return,
            Some(RiscvCoreDriveAction::FetchIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                panic!("checker test program should not issue data traffic");
            }
            None => {}
        }
    }
    panic!("expected one retired instruction from the checker test program");
}

fn retire_one_allowing_data(
    core: &RiscvCore,
    store: Arc<Mutex<PartitionedMemoryStore>>,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    for _ in 0..16 {
        match drive_one_action(core, store.clone(), scheduler, transport) {
            Some(RiscvCoreDriveAction::InstructionExecuted(_)) => return,
            Some(RiscvCoreDriveAction::FetchIssued { .. })
            | Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
            | Some(RiscvCoreDriveAction::DataAccessIssued { .. }) => {
                scheduler.run_until_idle_conservative();
            }
            None => {}
        }
    }
    panic!("expected one retired instruction from the checker test program");
}

#[test]
fn riscv_checker_cpu_follows_retired_fetch_stream_execution() {
    let (mut scheduler, transport, fetch_route) = routes();
    let core = core(fetch_route, 0x8000);
    let store = loaded_program(0x8000, &[i_type(5, 0, 0x0, 2, 0x13)]);

    core.enable_checker_cpu();
    retire_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 1);
    assert!(snapshot.mismatches().is_empty());
    assert_eq!(snapshot.hart().pc(), 0x8004);
    assert_eq!(snapshot.hart().read(reg(2)), 5);
}

#[test]
fn riscv_checker_cpu_records_reference_hart_mismatches_from_normal_execution() {
    let (mut scheduler, transport, fetch_route) = routes();
    let core = core(fetch_route, 0x8000);
    let store = loaded_program(0x8000, &[i_type(5, 1, 0x0, 2, 0x13)]);
    let mut checker_hart = RiscvHartState::with_hart_id(0x8000, 0);
    checker_hart.write(reg(1), 7);

    core.enable_checker_cpu_with_hart(checker_hart);
    retire_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 1);
    let [mismatch] = snapshot.mismatches() else {
        panic!("expected one checker mismatch");
    };
    assert_eq!(mismatch.sequence(), 0);
    assert_eq!(mismatch.pc(), Address::new(0x8000));
    assert_eq!(mismatch.primary_execution().register_writes()[0].value(), 5);
    assert_eq!(
        mismatch.checker_execution().register_writes()[0].value(),
        12
    );
    assert_eq!(mismatch.primary_hart().read(reg(2)), 5);
    assert_eq!(mismatch.checker_hart().read(reg(2)), 12);
}

#[test]
fn riscv_checker_cpu_follows_public_register_writes_after_enable() {
    let (mut scheduler, transport, fetch_route) = routes();
    let core = core(fetch_route, 0x8000);
    let store = loaded_program(0x8000, &[i_type(5, 1, 0x0, 2, 0x13)]);

    core.enable_checker_cpu();
    core.write_register(reg(1), 7);
    retire_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 1);
    assert!(snapshot.mismatches().is_empty());
    assert_eq!(snapshot.hart().read(reg(2)), 12);
}

#[test]
fn riscv_checker_cpu_follows_memory_load_writeback_before_dependent_retire() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    let mut program = Vec::new();
    for instruction in [
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        0x0000_0073,                 // ecall
    ] {
        program.extend(instruction.to_le_bytes());
    }
    program.extend([0; 4]);
    program.extend(7_u64.to_le_bytes());
    let store = loaded_program_bytes(0x8000, program);

    core.enable_checker_cpu();
    retire_one_allowing_data(&core, store.clone(), &mut scheduler, &transport);
    retire_one_allowing_data(&core, store.clone(), &mut scheduler, &transport);
    retire_one_allowing_data(&core, store.clone(), &mut scheduler, &transport);
    retire_one_allowing_data(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 4);
    assert!(snapshot.mismatches().is_empty());
    assert_eq!(snapshot.hart().read(reg(5)), 7);
    assert_eq!(snapshot.hart().read(reg(6)), 8);
}

#[test]
fn riscv_checker_cpu_follows_user_environment_call_completion() {
    let (mut scheduler, transport, fetch_route) = routes();
    let core = core(fetch_route, 0x8000);
    let store = loaded_program(0x8000, &[0x0000_0073, i_type(5, 0, 0x0, 2, 0x13)]);

    core.set_privilege_mode(RiscvPrivilegeMode::User);
    core.enable_checker_cpu();
    retire_one(&core, store.clone(), &mut scheduler, &transport);
    core.complete_pending_user_environment_call(0)
        .expect("user environment call should complete");
    retire_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 2);
    assert!(snapshot.mismatches().is_empty());
    assert_eq!(snapshot.hart().pc(), 0x8008);
    assert_eq!(snapshot.hart().read(reg(2)), 5);
}

#[test]
fn riscv_checker_cpu_follows_supervisor_environment_call_completion() {
    let (mut scheduler, transport, fetch_route) = routes();
    let core = core(fetch_route, 0x8000);
    let store = loaded_program(0x8000, &[0x0000_0073, i_type(5, 0, 0x0, 2, 0x13)]);

    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core.enable_checker_cpu();
    retire_one(&core, store.clone(), &mut scheduler, &transport);
    core.complete_pending_supervisor_environment_call(0, 0)
        .expect("supervisor environment call should complete");
    retire_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.checker_cpu_snapshot().unwrap();
    assert_eq!(snapshot.checked_instructions(), 2);
    assert!(snapshot.mismatches().is_empty());
    assert_eq!(snapshot.hart().pc(), 0x8008);
    assert_eq!(snapshot.hart().read(reg(2)), 5);
}
