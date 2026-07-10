use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};

const RISCV_LINUX_GETRUSAGE: u64 = 165;
const RISCV_LINUX_RUSAGE_BYTES: usize = 144;
const RISCV_LINUX_RUSAGE_SELF: u64 = 0;
const RISCV_LINUX_RUSAGE_CHILDREN: u64 = (-1_i64) as u64;
const RISCV_LINUX_RUSAGE_THREAD: u64 = 1;

#[test]
fn linux_table_getrusage_writes_zero_self_rusage() {
    let store = loaded_program_store_with_data(
        &[(0x8000, addi(0, 0, 0))],
        &[(0x9008, &[0xff; RISCV_LINUX_RUSAGE_BYTES])],
    );
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRUSAGE,
            [RISCV_LINUX_RUSAGE_SELF, 0x9008, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert_eq!(
        read_guest_bytes(Arc::clone(&store), 0x9008, RISCV_LINUX_RUSAGE_BYTES),
        vec![0; RISCV_LINUX_RUSAGE_BYTES]
    );
}

#[test]
fn linux_table_getrusage_accepts_children_and_thread_selectors() {
    let store = loaded_program_store_with_data(
        &[(0x8000, addi(0, 0, 0))],
        &[
            (0x9010, &[0xff; RISCV_LINUX_RUSAGE_BYTES]),
            (0x9100, &[0xff; RISCV_LINUX_RUSAGE_BYTES]),
        ],
    );
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    for (who, address) in [
        (RISCV_LINUX_RUSAGE_CHILDREN, 0x9010),
        (RISCV_LINUX_RUSAGE_THREAD, 0x9100),
    ] {
        let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETRUSAGE, [who, address, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        );
        assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
        assert_eq!(
            read_guest_bytes(Arc::clone(&store), address, RISCV_LINUX_RUSAGE_BYTES),
            vec![0; RISCV_LINUX_RUSAGE_BYTES]
        );
    }
}

#[test]
fn linux_table_getrusage_rejects_invalid_selector_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid getrusage selector should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETRUSAGE, [7, 0x9000, 0, 0, 0, 0]),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(22),
        })
    );
}

#[test]
fn linux_table_getrusage_reports_null_usage_as_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("null getrusage buffer should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRUSAGE,
            [RISCV_LINUX_RUSAGE_SELF, 0, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(14),
        })
    );
}

#[test]
fn linux_table_getrusage_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRUSAGE,
            [RISCV_LINUX_RUSAGE_SELF, 0x9000, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(14),
        })
    );
}

#[test]
fn user_ecall_getrusage_writes_rusage_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(83);
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
            (0x8000, addi(17, 0, RISCV_LINUX_GETRUSAGE as i32)),
            (0x8004, addi(10, 0, RISCV_LINUX_RUSAGE_SELF as i32)),
            (0x8008, lui(11, 9)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(5, 10, 0)),
            (0x8014, addi(17, 0, 93)),
            (0x8018, addi(10, 0, 0)),
            (0x801c, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; RISCV_LINUX_RUSAGE_BYTES])],
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
            90,
            |cpu| GuestEventId::new(570 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(570), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(
        read_guest_bytes(Arc::clone(&store), 0x9000, RISCV_LINUX_RUSAGE_BYTES),
        vec![0; RISCV_LINUX_RUSAGE_BYTES]
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

fn read_guest_bytes(
    store: Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    bytes: usize,
) -> Vec<u8> {
    let reader = guest_memory_reader(store);
    (0..bytes)
        .map(|offset| reader(address + offset as u64, 1).unwrap()[0])
        .collect()
}
