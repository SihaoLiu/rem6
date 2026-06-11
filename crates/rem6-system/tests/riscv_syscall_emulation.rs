use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
use rem6_kernel::{PartitionId, PartitionedScheduler, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvMmapRegion, RiscvSystemRunDriver,
    RiscvTrapEventPort, StopRequest, SystemActionOutcome, SystemHostController,
    SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn word(raw: u32) -> Vec<u8> {
    raw.to_le_bytes().to_vec()
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x13)
}

fn lb(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x03)
}

fn lui(rd: u8, imm: u32) -> u32 {
    (imm << 12) | (u32::from(rd) << 7) | 0x37
}

fn loaded_program_store(instructions: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program_store_with_data(instructions, &[])
}

fn loaded_program_store_with_data(
    instructions: &[(u64, u32)],
    data_segments: &[(u64, &[u8])],
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

    let mut image = BootImage::new(Address::new(instructions[0].0));
    for (address, instruction) in instructions {
        image = image
            .add_segment(Address::new(*address), word(*instruction))
            .unwrap();
    }
    for (address, data) in data_segments {
        image = image
            .add_segment(Address::new(*address), data.to_vec())
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn riscv_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[allow(clippy::too_many_arguments)]
fn riscv_data_core(
    cpu: u32,
    partition: u32,
    agent: u32,
    fetch_endpoint: &str,
    fetch_route: MemoryRouteId,
    data_endpoint: &str,
    data_route: MemoryRouteId,
    entry: u64,
) -> RiscvCore {
    RiscvCore::with_data(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(cpu),
                PartitionId::new(partition),
                AgentId::new(agent),
                Address::new(entry),
            ),
            CpuFetchConfig::new(
                endpoint(fetch_endpoint),
                fetch_route,
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
        CpuDataConfig::new(endpoint(data_endpoint), data_route, layout()),
    )
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

fn responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
    move |delivery, _context| memory_response(&store, &delivery)
}

fn guest_memory_reader(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static {
    move |address, bytes| {
        let request = MemoryRequest::read_shared(
            MemoryRequestId::new(AgentId::new(99), 0),
            Address::new(address),
            AccessSize::new(bytes as u64).ok()?,
            layout(),
        )
        .ok()?;
        let outcome = store.lock().unwrap().respond(&request).ok()?;
        outcome
            .response()
            .and_then(|response| response.data())
            .map(Vec::from)
    }
}

fn guest_memory_writer(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl Fn(u64, &[u8]) -> bool + Send + Sync + 'static {
    move |address, bytes| {
        let Ok(size) = AccessSize::new(bytes.len() as u64) else {
            return false;
        };
        let Ok(byte_mask) = ByteMask::full(size) else {
            return false;
        };
        let Ok(request) = MemoryRequest::write(
            MemoryRequestId::new(AgentId::new(100), 0),
            Address::new(address),
            size,
            bytes.to_vec(),
            byte_mask,
            layout(),
        ) else {
            return false;
        };
        store.lock().unwrap().respond(&request).is_ok()
    }
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn user_ecall_exit_is_consumed_as_riscv_se_syscall() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(41);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 93)),
        (0x8004, addi(10, 0, 17)),
        (0x8008, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            30,
            |cpu| GuestEventId::new(160 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(160),
        source,
        17,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(17)), 93);
    assert_eq!(core.read_register(reg(10)), 17);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_brk_returns_to_guest_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(42);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 214)),
        (0x8004, addi(10, 0, 64)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 214)),
        (0x8014, addi(10, 0, 0)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(6, 10, 0)),
        (0x8020, addi(17, 0, 93)),
        (0x8024, addi(10, 6, 0)),
        (0x8028, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(180 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(180),
        source,
        64,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 64);
    assert_eq!(core.read_register(reg(6)), 64);
    assert_eq!(core.read_register(reg(10)), 64);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_getpid_returns_identity_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(45);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 172)),
        (0x8004, 0x0000_0073),
        (0x8008, addi(5, 10, 0)),
        (0x800c, addi(17, 0, 93)),
        (0x8010, addi(10, 5, 0)),
        (0x8014, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(240 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(240),
        source,
        100,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 100);
    assert_eq!(core.read_register(reg(10)), 100);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_getuid_returns_default_identity_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(46);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 174)),
        (0x8004, 0x0000_0073),
        (0x8008, addi(5, 10, 0)),
        (0x800c, addi(17, 0, 93)),
        (0x8010, addi(10, 5, 0)),
        (0x8014, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(260 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(260),
        source,
        100,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 100);
    assert_eq!(core.read_register(reg(10)), 100);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_getppid_overwrites_a0_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(47);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 77)),
        (0x8004, addi(17, 0, 173)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 93)),
        (0x8014, addi(10, 5, 0)),
        (0x8018, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(280 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(280), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_set_tid_address_records_pointer_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(48);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 80)),
        (0x8004, addi(17, 0, 96)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 93)),
        (0x8014, addi(10, 5, 0)),
        (0x8018, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(300 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(300),
        source,
        100,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 100);
    assert_eq!(core.read_register(reg(10)), 100);
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .child_clear_tid(),
        Some(80)
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_rt_sigaction_returns_zero_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(49);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 77)),
        (0x8004, addi(17, 0, 134)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 93)),
        (0x8014, addi(10, 5, 0)),
        (0x8018, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(320 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(320), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_mprotect_returns_zero_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(50);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 77)),
        (0x8004, addi(17, 0, 226)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 93)),
        (0x8014, addi(10, 5, 0)),
        (0x8018, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(340 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(340), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_rseq_returns_enosys_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(51);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 293)),
        (0x8004, 0x0000_0073),
        (0x8008, addi(5, 10, 0)),
        (0x800c, addi(17, 0, 93)),
        (0x8010, addi(10, 0, 0)),
        (0x8014, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(360 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(360), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0u64.wrapping_sub(38));
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_futex_wake_private_returns_zero_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(52);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 80)),
        (0x8004, addi(11, 0, 129)),
        (0x8008, addi(12, 0, 3)),
        (0x800c, addi(17, 0, 98)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(17, 0, 93)),
        (0x801c, addi(10, 5, 0)),
        (0x8020, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(380 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(380), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_fcntl_sets_and_reads_close_on_exec_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(53);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 1)),
        (0x8004, addi(11, 0, 2)),
        (0x8008, addi(12, 0, 1)),
        (0x800c, addi(17, 0, 25)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(6, 10, 0)),
        (0x8018, addi(10, 0, 1)),
        (0x801c, addi(11, 0, 1)),
        (0x8020, addi(17, 0, 25)),
        (0x8024, 0x0000_0073),
        (0x8028, addi(5, 10, 0)),
        (0x802c, addi(17, 0, 93)),
        (0x8030, addi(10, 5, 0)),
        (0x8034, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            100,
            |cpu| GuestEventId::new(390 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(390), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(core.read_register(reg(10)), 1);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_close_removes_fd_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(54);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 1)),
        (0x8004, addi(17, 0, 57)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(6, 10, 0)),
        (0x8010, addi(10, 0, 1)),
        (0x8014, addi(11, 0, 1)),
        (0x8018, addi(17, 0, 25)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(17, 0, 93)),
        (0x8028, addi(10, 0, 23)),
        (0x802c, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            100,
            |cpu| GuestEventId::new(410 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(410),
        source,
        23,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(5)), 0u64.wrapping_sub(9));
    assert_eq!(core.read_register(reg(10)), 23);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_dup_and_dup3_resume_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(55);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 1)),
        (0x8004, addi(17, 0, 23)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(6, 10, 0)),
        (0x8010, addi(10, 6, 0)),
        (0x8014, addi(11, 0, 1)),
        (0x8018, addi(17, 0, 25)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(10, 0, 1)),
        (0x8028, addi(11, 0, 4)),
        (0x802c, lui(12, 0x80)),
        (0x8030, addi(17, 0, 24)),
        (0x8034, 0x0000_0073),
        (0x8038, addi(10, 0, 4)),
        (0x803c, addi(11, 0, 1)),
        (0x8040, addi(17, 0, 25)),
        (0x8044, 0x0000_0073),
        (0x8048, addi(7, 10, 0)),
        (0x804c, addi(17, 0, 93)),
        (0x8050, addi(10, 0, 23)),
        (0x8054, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            140,
            |cpu| GuestEventId::new(430 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(430),
        source,
        23,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 3);
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(7)), 1);
    assert_eq!(core.read_register(reg(10)), 23);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_mmap_returns_mapping_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(43);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 222)),
        (0x8004, addi(10, 0, 0)),
        (0x8008, addi(11, 0, 64)),
        (0x800c, addi(12, 0, 3)),
        (0x8010, addi(13, 0, 34)),
        (0x8014, addi(14, 0, -1)),
        (0x8018, addi(15, 0, 0)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(17, 0, 93)),
        (0x8028, addi(10, 0, 17)),
        (0x802c, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            100,
            |cpu| GuestEventId::new(200 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(200),
        source,
        17,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0x4000_0000_0000_0000);
    assert_eq!(core.read_register(reg(10)), 17);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_munmap_updates_mapping_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(44);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 222)),
        (0x8004, addi(10, 0, 0)),
        (0x8008, lui(11, 3)),
        (0x800c, addi(12, 0, 3)),
        (0x8010, addi(13, 0, 34)),
        (0x8014, addi(14, 0, -1)),
        (0x8018, addi(15, 0, 0)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(17, 0, 215)),
        (0x8028, addi(10, 5, 2047)),
        (0x802c, addi(10, 10, 2047)),
        (0x8030, addi(10, 10, 2)),
        (0x8034, lui(11, 1)),
        (0x8038, 0x0000_0073),
        (0x803c, addi(6, 10, 0)),
        (0x8040, addi(17, 0, 93)),
        (0x8044, addi(10, 0, 17)),
        (0x8048, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            140,
            |cpu| GuestEventId::new(220 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(220),
        source,
        17,
    );
    let mmap_base = 0x4000_0000_0000_0000;
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), mmap_base);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(10)), 17);
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .mmap_regions(),
        &[
            RiscvMmapRegion::new(mmap_base, 4096, 3, 34, u64::MAX, 0),
            RiscvMmapRegion::new(mmap_base + 8192, 4096, 3, 34, u64::MAX, 8192),
        ]
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_write_reads_guest_memory_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(68);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 64)),
            (0x8004, addi(10, 0, 1)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 5)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(17, 0, 93)),
            (0x801c, addi(10, 5, 0)),
            (0x8020, 0x0000_0073),
        ],
        &[(0x9000, b"hello")],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_reader(guest_memory_reader(Arc::clone(
            &store,
        )));

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(420 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(420), source, 5);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 5);
    let state = driver.riscv_syscall_emulation().unwrap().state();
    assert_eq!(state.guest_writes().len(), 1);
    let write = &state.guest_writes()[0];
    assert_eq!(write.fd().get(), 1);
    assert_eq!(write.address(), 0x9000);
    assert_eq!(write.bytes(), b"hello");
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_read_writes_guest_memory_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(69);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
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
                endpoint("l1d"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        fetch_route,
        "cpu0.dmem",
        data_route,
        0x8000,
    );
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 63)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 1)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, lui(6, 9)),
            (0x801c, lb(7, 6, 0)),
            (0x8020, addi(17, 0, 93)),
            (0x8024, addi(10, 7, 0)),
            (0x8028, 0x0000_0073),
        ],
        &[(0x9000, b"\0")],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
            &store,
        )));
    driver
        .riscv_syscall_emulation()
        .unwrap()
        .push_stdin_bytes(b"A");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            90,
            |cpu| GuestEventId::new(440 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(440),
        source,
        65,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(core.read_register(reg(7)), 65);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9000, 1),
        Some(b"A".to_vec())
    );
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .stdin_byte_count(),
        0
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_read_then_write_uses_bidirectional_guest_memory_io() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(70);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 63)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 1)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(17, 0, 64)),
            (0x801c, addi(10, 0, 1)),
            (0x8020, lui(11, 9)),
            (0x8024, addi(12, 0, 1)),
            (0x8028, 0x0000_0073),
            (0x802c, addi(6, 10, 0)),
            (0x8030, addi(17, 0, 93)),
            (0x8034, addi(10, 6, 0)),
            (0x8038, 0x0000_0073),
        ],
        &[(0x9000, b"\0")],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_io(
            guest_memory_reader(Arc::clone(&store)),
            guest_memory_writer(Arc::clone(&store)),
        );
    driver
        .riscv_syscall_emulation()
        .unwrap()
        .push_stdin_bytes(b"Q");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            110,
            |cpu| GuestEventId::new(460 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(460), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(core.read_register(reg(6)), 1);
    let state = driver.riscv_syscall_emulation().unwrap().state();
    assert_eq!(state.stdin_byte_count(), 0);
    assert_eq!(state.guest_writes().len(), 1);
    assert_eq!(state.guest_writes()[0].fd().get(), 1);
    assert_eq!(state.guest_writes()[0].bytes(), b"Q");
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_openat_reads_guest_path_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(71);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 56)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, addi(13, 0, 0)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, b"/input.txt\0")],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_reader(guest_memory_reader(Arc::clone(
            &store,
        )));
    driver
        .riscv_syscall_emulation()
        .unwrap()
        .register_guest_path(b"/input.txt");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            90,
            |cpu| GuestEventId::new(480 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(480), source, 3);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 3);
    let state = driver.riscv_syscall_emulation().unwrap().state();
    assert_eq!(state.guest_opens().len(), 1);
    assert_eq!(state.guest_opens()[0].path(), b"/input.txt");
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_openat_then_read_copies_registered_guest_file_contents() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(72);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 56)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, addi(13, 0, 0)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, 63)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, lui(11, 9)),
            (0x8028, addi(11, 11, 0x100)),
            (0x802c, addi(12, 0, 3)),
            (0x8030, 0x0000_0073),
            (0x8034, addi(6, 10, 0)),
            (0x8038, addi(17, 0, 93)),
            (0x803c, addi(10, 6, 0)),
            (0x8040, 0x0000_0073),
        ],
        &[(0x9000, b"/input.txt\0"), (0x9100, b"\0\0\0")],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_io(
            guest_memory_reader(Arc::clone(&store)),
            guest_memory_writer(Arc::clone(&store)),
        );
    driver
        .riscv_syscall_emulation()
        .unwrap()
        .register_guest_file(b"/input.txt", b"abcde");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            140,
            |cpu| GuestEventId::new(500 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(500), source, 3);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 3);
    assert_eq!(core.read_register(reg(6)), 3);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 3),
        Some(b"abc".to_vec())
    );
    let state = driver.riscv_syscall_emulation().unwrap().state();
    assert_eq!(state.guest_opens().len(), 1);
    assert_eq!(state.guest_opens()[0].path(), b"/input.txt");
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
