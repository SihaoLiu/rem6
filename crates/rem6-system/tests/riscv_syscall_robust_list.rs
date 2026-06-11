#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use riscv_syscall_emulation_support::*;

#[test]
fn user_ecall_get_robust_list_writes_recorded_head_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(56);
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
    let zeros = [0; 16];
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(10, 0, 80)),
            (0x8004, addi(11, 0, 24)),
            (0x8008, addi(17, 0, 99)),
            (0x800c, 0x0000_0073),
            (0x8010, lui(6, 9)),
            (0x8014, addi(6, 6, 0x100)),
            (0x8018, addi(10, 0, 0)),
            (0x801c, addi(11, 6, 0)),
            (0x8020, addi(12, 6, 8)),
            (0x8024, addi(17, 0, 100)),
            (0x8028, 0x0000_0073),
            (0x802c, ld(7, 6, 0)),
            (0x8030, ld(8, 6, 8)),
            (0x8034, addi(17, 0, 93)),
            (0x8038, addi(10, 7, 0)),
            (0x803c, 0x0000_0073),
        ],
        &[(0x9100, zeros.as_slice())],
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
            120,
            |cpu| GuestEventId::new(320 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(320),
        source,
        80,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(7)), 80);
    assert_eq!(core.read_register(reg(8)), 24);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 8),
        Some(80u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9108, 8),
        Some(24u64.to_le_bytes().to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
