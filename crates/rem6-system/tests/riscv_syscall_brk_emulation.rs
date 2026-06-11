#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use riscv_syscall_emulation_support::*;

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
fn user_ecall_brk_uses_boot_image_program_break_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(150);
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
    let image = boot_image_with_data(
        &[
            (0x8000, addi(17, 0, 214)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(17, 0, 93)),
            (0x8014, addi(10, 0, 0)),
            (0x8018, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver =
        RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation_for_boot_image(&image);

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
            |cpu| GuestEventId::new(230 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(230), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0xa000);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

fn check_getcwd_then_brk_with_boot_image_and_guest_writer(seed_before_writer: bool) {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(if seed_before_writer { 151 } else { 152 });
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
    let image = boot_image_with_data(
        &[
            (0x8000, lui(10, 0x9)),
            (0x8004, addi(11, 0, 8)),
            (0x8008, addi(17, 0, 17)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(6, 10, 0)),
            (0x8014, addi(17, 0, 214)),
            (0x8018, addi(10, 0, 0)),
            (0x801c, 0x0000_0073),
            (0x8020, addi(5, 10, 0)),
            (0x8024, addi(17, 0, 93)),
            (0x8028, addi(10, 0, 0)),
            (0x802c, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let read_guest = guest_memory_reader(Arc::clone(&store));
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = if seed_before_writer {
        RiscvSystemRunDriver::new(trap_port)
            .with_riscv_syscall_emulation_for_boot_image(&image)
            .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
                &store,
            )))
    } else {
        RiscvSystemRunDriver::new(trap_port)
            .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
                &store,
            )))
            .with_riscv_syscall_emulation_for_boot_image(&image)
    };

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            120,
            |cpu| GuestEventId::new(310 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(310), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 1);
    assert_eq!(core.read_register(reg(5)), 0xa000);
    assert_eq!(read_guest(0x9000, 4).unwrap(), b"/\0\0\0");
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );
}

#[test]
fn user_ecall_brk_preserves_boot_image_break_when_guest_writer_is_added() {
    check_getcwd_then_brk_with_boot_image_and_guest_writer(true);
}

#[test]
fn user_ecall_getcwd_preserves_guest_writer_when_boot_image_break_is_added() {
    check_getcwd_then_brk_with_boot_image_and_guest_writer(false);
}
