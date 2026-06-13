#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{GuestTrap, GuestTrapKind};
use support::*;

const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

#[test]
fn supervisor_sbi_base_get_spec_version_returns_before_user_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(61);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

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

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(360), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8014)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 2);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_unknown_extension_returns_not_supported_before_user_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(62);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x55)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

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

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(380), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8014)]
    );
    assert_eq!(core.read_register(reg(5)), (-2_i64) as u64);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(10)), (-2_i64) as u64);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_base_identity_and_probe_calls_return_success() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(63);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 1)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, addi(16, 0, 2)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(7, 10, 0)),
        (0x8020, addi(8, 11, 0)),
        (0x8024, addi(10, 0, 0x10)),
        (0x8028, addi(16, 0, 3)),
        (0x802c, 0x0000_0073),
        (0x8030, addi(9, 10, 0)),
        (0x8034, addi(12, 11, 0)),
        (0x8038, addi(10, 0, 0x11)),
        (0x803c, addi(16, 0, 3)),
        (0x8040, 0x0000_0073),
        (0x8044, addi(13, 10, 0)),
        (0x8048, addi(14, 11, 0)),
        (0x804c, addi(16, 0, 4)),
        (0x8050, 0x0000_0073),
        (0x8054, addi(15, 10, 0)),
        (0x8058, addi(18, 11, 0)),
        (0x805c, addi(16, 0, 5)),
        (0x8060, 0x0000_0073),
        (0x8064, addi(19, 10, 0)),
        (0x8068, addi(20, 11, 0)),
        (0x806c, addi(16, 0, 6)),
        (0x8070, 0x0000_0073),
        (0x8074, addi(21, 10, 0)),
        (0x8078, addi(22, 11, 0)),
        (0x807c, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            200,
            |cpu| GuestEventId::new(400 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(400), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x807c)]
    );
    for register in [5, 7, 9, 13, 15, 19, 21] {
        assert_eq!(core.read_register(reg(register)), 0);
    }
    assert_eq!(core.read_register(reg(6)), REM6_SBI_IMPL_ID);
    assert_eq!(core.read_register(reg(8)), 0);
    assert_eq!(core.read_register(reg(12)), 1);
    assert_eq!(core.read_register(reg(14)), 0);
    assert_eq!(core.read_register(reg(18)), 0);
    assert_eq!(core.read_register(reg(20)), 0);
    assert_eq!(core.read_register(reg(22)), 0);
}

#[test]
fn delegated_supervisor_environment_call_is_not_consumed_as_sbi() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(63);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core.set_machine_exception_delegation(1 << 9);
    core.set_supervisor_trap_vector(0x9000);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, 0x0010_0073),
        (0x9000, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

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
            |cpu| GuestEventId::new(400 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(400), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8008)]
    );
    assert_eq!(core.pc().get(), 0x9000);
    assert_eq!(core.read_register(reg(5)), 0);
}
