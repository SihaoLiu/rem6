#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{GuestTrap, GuestTrapKind};
use support::*;

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
