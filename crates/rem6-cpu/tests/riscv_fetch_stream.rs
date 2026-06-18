use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreDriveAction,
};
use rem6_isa_riscv::{Register, RiscvInstruction, RiscvPrivilegeMode, RiscvTrap, RiscvTrapKind};
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

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn csr_type(csr: u32, rs1_or_zimm: u8, funct3: u32, rd: u8) -> u32 {
    (csr << 20) | (u32::from(rs1_or_zimm) << 15) | (funct3 << 12) | (u32::from(rd) << 7) | 0x73
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn core(route: MemoryRouteId, entry: u64) -> CpuCore {
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

fn loaded_program(entry: u64, instructions: &[u32]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let mut bytes = Vec::with_capacity(instructions.len() * 4);
    for instruction in instructions {
        bytes.extend(word(*instruction));
    }
    loaded_program_bytes(entry, bytes)
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

#[test]
fn riscv_core_syscall_return_resets_fetch_stream_to_return_pc() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let store = loaded_program(0x8000, &[0x0000_0073, i_type(7, 0, 0x0, 1, 0x13)]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(trap) = action else {
        panic!("expected environment-call trap execution");
    };
    assert_eq!(
        trap.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );
    assert!(!core.inner().fetch_events().is_empty());

    assert_eq!(
        core.complete_pending_user_environment_call(17),
        Some(RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );

    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(10)), 17);
    assert!(core.inner().fetch_events().is_empty());
    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_supervisor_syscall_return_resets_fetch_stream_to_return_pc() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let store = loaded_program(0x8000, &[0x0000_0073, i_type(9, 0, 0x0, 1, 0x13)]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store.clone(), &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(trap) = action else {
        panic!("expected supervisor environment-call trap execution");
    };
    assert_eq!(
        trap.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );
    assert!(!core.inner().fetch_events().is_empty());

    assert_eq!(
        core.complete_pending_supervisor_environment_call(5, 23),
        Some(RiscvTrap::new(RiscvTrapKind::EnvironmentCall, 0x8000))
    );

    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.inner().pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(10)), 5);
    assert_eq!(core.read_register(reg(11)), 23);
    assert!(core.inner().fetch_events().is_empty());
    assert!(matches!(
        drive_one_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
}

#[test]
fn riscv_core_traps_machine_identity_csr_write_from_fetch_stream() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.set_machine_trap_vector(0x9000);
    core.write_register(reg(1), 0xffff);
    let raw = csr_type(0xf11, 1, 0x1, 5);
    let store = loaded_program(0x8000, &[raw, i_type(7, 0, 0x0, 6, 0x13)]);

    assert!(matches!(
        drive_one_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();
    let action = drive_one_action(&core, store, &mut scheduler, &transport).unwrap();
    let RiscvCoreDriveAction::InstructionExecuted(event) = action else {
        panic!("expected illegal CSR write to retire as a trap");
    };

    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(
        event.execution().trap(),
        Some(&RiscvTrap::new(RiscvTrapKind::IllegalInstruction, 0x8000))
    );
    assert_eq!(core.pc(), Address::new(0x9000));
    assert_eq!(core.inner().pc(), Address::new(0x9000));
    assert_eq!(core.machine_exception_pc(), 0x8000);
    assert_eq!(core.machine_trap_cause(), 2);
    assert_eq!(core.pending_trap(), event.execution().trap().copied());
    assert_eq!(core.read_register(reg(5)), 0);
}

#[test]
fn riscv_core_ignores_duplicate_completed_prefix_fetch_after_split_word_retire() {
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
    let core = RiscvCore::new(core(route, 0x800e));
    let store = loaded_program_bytes(0x800e, word(raw));

    for _ in 0..2 {
        let fetch_store = store.clone();
        core.issue_next_fetch(
            &mut scheduler,
            &transport,
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
        )
        .unwrap();
    }
    scheduler.run_until_idle_conservative();

    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    fetch_one(&core, store, &mut scheduler, &transport);
    let event = core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(event.fetch_pc(), Address::new(0x800e));
    assert_eq!(event.instruction(), RiscvInstruction::decode(raw).unwrap());
    assert_eq!(event.execution().next_pc(), 0x8012);
    assert_eq!(core.execute_next_completed_fetch().unwrap(), None);
    assert_eq!(core.pc(), Address::new(0x8012));
    assert_eq!(core.inner().pc(), Address::new(0x8012));
}
