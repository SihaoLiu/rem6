#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use riscv_syscall_emulation_support::*;

#[test]
fn user_ecall_uname_writes_riscv64_machine_field() {
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
            (0x8000, addi(17, 0, 160)),
            (0x8004, lui(10, 9)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, lui(6, 9)),
            (0x8014, addi(6, 6, 0x104)),
            (0x8018, lb(7, 6, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &[0; 390])],
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
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(560), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(7)), u64::from(b'r'));
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9104, 8),
        Some(b"riscv64\0".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
