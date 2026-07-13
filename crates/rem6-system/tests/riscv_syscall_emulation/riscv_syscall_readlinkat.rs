use super::riscv_syscall_emulation_support::*;

#[test]
fn user_ecall_readlinkat_copies_registered_executable_link() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(75);
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
            (0x8000, addi(17, 0, 78)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, lui(12, 9)),
            (0x8010, addi(12, 12, 0x100)),
            (0x8014, addi(13, 0, 5)),
            (0x8018, 0x0000_0073),
            (0x801c, addi(5, 10, 0)),
            (0x8020, lui(6, 9)),
            (0x8024, addi(6, 6, 0x100)),
            (0x8028, lb(7, 6, 4)),
            (0x802c, addi(17, 0, 93)),
            (0x8030, addi(10, 5, 0)),
            (0x8034, 0x0000_0073),
        ],
        &[(0x9000, b"/proc/self/exe\0"), (0x9100, b"\0\0\0\0\0")],
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
        .register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            280,
            |cpu| GuestEventId::new(540 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(540), source, 5);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 5);
    assert_eq!(core.read_register(reg(7)), u64::from(b'/'));
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 5),
        Some(b"/bin/".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_readlinkat_copies_proc_self_fd_guest_path() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(76);
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
            (0x8000, addi(17, 0, 56)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(18, 10, 0)),
            (0x8018, addi(17, 0, 78)),
            (0x801c, addi(10, 0, -100)),
            (0x8020, lui(11, 9)),
            (0x8024, addi(11, 11, 0x100)),
            (0x8028, lui(12, 9)),
            (0x802c, addi(12, 12, 0x200)),
            (0x8030, addi(13, 0, 16)),
            (0x8034, 0x0000_0073),
            (0x8038, addi(5, 10, 0)),
            (0x803c, lui(6, 9)),
            (0x8040, addi(6, 6, 0x200)),
            (0x8044, lb(7, 6, 15)),
            (0x8048, addi(17, 0, 78)),
            (0x804c, addi(10, 0, -100)),
            (0x8050, lui(11, 9)),
            (0x8054, addi(11, 11, 0x140)),
            (0x8058, lui(12, 9)),
            (0x805c, addi(12, 12, 0x240)),
            (0x8060, addi(13, 0, 16)),
            (0x8064, 0x0000_0073),
            (0x8068, addi(8, 10, 0)),
            (0x806c, addi(17, 0, 93)),
            (0x8070, addi(10, 5, 0)),
            (0x8074, 0x0000_0073),
        ],
        &[
            (0x9000, b"/guest/readlink.txt\0"),
            (0x9100, b"/proc/self/fd/3\0"),
            (0x9140, b"/proc/self/fd/99\0"),
            (0x9200, b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
            (0x9240, b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0"),
        ],
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
        .register_guest_file(b"/guest/readlink.txt", b"fd-target");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            360,
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(560),
        source,
        16,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(18)), 3);
    assert_eq!(core.read_register(reg(5)), 16);
    assert_eq!(core.read_register(reg(7)), u64::from(b'.'));
    assert_eq!(core.read_register(reg(8)), (-2_i64) as u64);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9200, 16),
        Some(b"/guest/readlink.".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
