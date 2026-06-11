#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

#[test]
fn linux_table_clock_gettime_writes_deterministic_timespec() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 113, [1, 0x9000, 0, 0, 0, 0]),
        &mut state,
        1_234_567_890,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), 1);
    assert_eq!(read_u64(&bytes, 8), 234_567_890);
}

#[test]
fn linux_table_clock_gettime_writes_line_straddling_timespec() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9008, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 113, [1, 0x9008, 0, 0, 0, 0]),
        &mut state,
        1_234_567_890,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = read_guest_bytes(Arc::clone(&store), 0x9008, 16);
    assert_eq!(read_u64(&bytes, 0), 1);
    assert_eq!(read_u64(&bytes, 8), 234_567_890);
}

#[test]
fn linux_table_clock_gettime_rejects_invalid_clock_id_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid clock id should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 113, [u64::MAX, 0x9000, 0, 0, 0, 0]),
        &mut state,
        8,
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
fn linux_table_clock_gettime_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 113, [1, 0x9000, 0, 0, 0, 0]),
        &mut state,
        8,
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
fn linux_table_gettimeofday_writes_deterministic_timeval() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 169, [0x9000, 0, 0, 0, 0, 0]),
        &mut state,
        1_234_567_890,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), 1);
    assert_eq!(read_u64(&bytes, 8), 234_567);
}

#[test]
fn linux_table_gettimeofday_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 169, [0x9000, 0, 0, 0, 0, 0]),
        &mut state,
        8,
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
fn user_ecall_clock_gettime_writes_timespec_before_exit() {
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 113)),
            (0x8004, addi(10, 0, 1)),
            (0x8008, lui(11, 9)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(5, 10, 0)),
            (0x8014, addi(17, 0, 93)),
            (0x8018, addi(10, 5, 0)),
            (0x801c, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; 16])],
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
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(560), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), 0);
    let nanoseconds = read_u64(&bytes, 8);
    assert!(nanoseconds > 0);
    assert!(nanoseconds < 1_000_000_000);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_gettimeofday_writes_timeval_before_exit() {
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 169)),
            (0x8004, lui(10, 9)),
            (0x8008, addi(11, 0, 0)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(5, 10, 0)),
            (0x8014, addi(17, 0, 93)),
            (0x8018, addi(10, 5, 0)),
            (0x801c, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; 16])],
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
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_ne!(bytes, vec![0xff; 16]);
    assert_eq!(read_u64(&bytes, 0), 0);
    let microseconds = read_u64(&bytes, 8);
    assert!(microseconds < 1_000_000);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
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
