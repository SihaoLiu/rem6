#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use rem6_system::{GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestWaitStatus};
use riscv_syscall_emulation_support::*;

#[test]
fn user_ecall_wait4_consumes_pending_child_status_and_resumes() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(82);
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
    let status_buffer = [0xff; 4];
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 260)),
            (0x8004, addi(10, 0, -1)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, addi(13, 0, 0)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &status_buffer)],
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
        .push_wait_child(GuestChildStatus::new(
            GuestProcessId::new(123).unwrap(),
            GuestProcessGroupId::new(100).unwrap(),
            GuestWaitStatus::exited(7),
        ));

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
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(560),
        source,
        123,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 123);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9000, 4),
        Some(((7_i32) << 8).to_le_bytes().to_vec())
    );
    assert!(driver
        .riscv_syscall_emulation()
        .unwrap()
        .state()
        .guest_wait_queue()
        .is_empty());
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
