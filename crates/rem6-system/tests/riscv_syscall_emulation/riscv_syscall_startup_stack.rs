use super::riscv_syscall_emulation_support::*;
use rem6_system::RiscvSeStartupConfig;

#[test]
fn user_program_reads_startup_stack_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(190);
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
    let startup = RiscvSeStartupConfig::new(Address::new(0xa000))
        .with_arg("rem6")
        .build()
        .unwrap();
    core.write_register(reg(2), startup.initial_stack_pointer().get());
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, ld(5, 2, 0)),
            (0x8004, ld(6, 2, 8)),
            (0x8008, lb(7, 6, 0)),
            (0x800c, addi(17, 0, 93)),
            (0x8010, addi(10, 7, 0)),
            (0x8014, 0x0000_0073),
        ],
        &[(startup.stack_range().start().get(), startup.stack_data())],
    );
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
            90,
            |cpu| GuestEventId::new(290 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(290),
        source,
        i32::from(b'r'),
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(core.read_register(reg(7)), u64::from(b'r'));
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
