use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable,
};

const RISCV_LINUX_PIPE2: u64 = 59;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITEV: u64 = 66;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn rv64_iovec(base: u64, len: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    bytes
}

fn writev_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let iov0 = rv64_iovec(0x9100, 2);
    let iov1 = rv64_iovec(0x9200, 3);
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, &iov0),
            (0x9010, &iov1),
            (0x9100, b"he"),
            (0x9200, b"llo"),
        ],
    )
}

fn writev_pipe_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let fd_area = [0; 8];
    let read_area = [0; 5];
    let iov0 = rv64_iovec(0x9100, 2);
    let iov1 = rv64_iovec(0x9200, 3);
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &fd_area),
            (0x9000, &iov0),
            (0x9010, &iov1),
            (0x9100, b"he"),
            (0x9200, b"llo"),
            (0x9300, &read_area),
        ],
    )
}

fn create_pipe(
    state: &mut RiscvSyscallState,
    writer: &RiscvGuestMemoryWriter,
    store: &Arc<Mutex<PartitionedMemoryStore>>,
) -> (u64, u64) {
    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
        state,
        0,
        None,
        Some(writer),
    );
    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));

    let fds = guest_memory_reader(Arc::clone(store))(0x8800, 8).unwrap();
    let read_fd = i32::from_le_bytes(fds[..4].try_into().unwrap());
    let write_fd = i32::from_le_bytes(fds[4..].try_into().unwrap());
    (read_fd as u64, write_fd as u64)
}

#[test]
fn linux_table_writev_reads_iovecs_and_records_single_guest_write() {
    let store = writev_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 2, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 5 }));
    assert_eq!(state.guest_writes().len(), 1);
    let write = &state.guest_writes()[0];
    assert_eq!(write.fd().get(), 1);
    assert_eq!(write.address(), 0x9000);
    assert_eq!(write.tick(), 11);
    assert_eq!(write.bytes(), b"hello");
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(1).unwrap())
            .unwrap()
            .get(),
        5
    );
}

#[test]
fn linux_table_writev_to_pipe_buffers_bytes_without_guest_write_record() {
    let store = writev_pipe_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let (read_fd, write_fd) = create_pipe(&mut state, &writer, &store);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [write_fd, 0x9000, 2, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 5 }));
    assert!(state.guest_writes().is_empty());

    let read_outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [read_fd, 0x9300, 5, 0, 0, 0]),
        &mut state,
        12,
        None,
        Some(&writer),
    );

    assert_eq!(read_outcome, Some(RiscvSyscallOutcome::Return { value: 5 }));
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9300, 5),
        Some(b"hello".to_vec())
    );
    assert!(state.guest_writes().is_empty());
}

#[test]
fn linux_table_writev_zero_iov_count_returns_zero_without_guest_reads() {
    let reader = RiscvGuestMemoryReader::new(|_address, _bytes| {
        panic!("zero iov count should not read guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 0, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.guest_writes().is_empty());
}

#[test]
fn linux_table_writev_zero_total_length_returns_zero_without_guest_write_record() {
    let iov0 = rv64_iovec(0x9100, 0);
    let reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        (address == 0x9000 && bytes == 16).then_some(iov0.to_vec())
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 1, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.guest_writes().is_empty());
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(1).unwrap())
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn linux_table_writev_rejects_bad_or_read_only_fd_before_guest_reads() {
    let reader = RiscvGuestMemoryReader::new(|_address, _bytes| {
        panic!("fd validation should happen before guest memory reads")
    });
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [99, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [0, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.guest_writes().is_empty());
}

#[test]
fn linux_table_writev_rejects_excessive_iov_count() {
    let reader = RiscvGuestMemoryReader::new(|_address, _bytes| {
        panic!("invalid iov count should not read guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 1025, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_writes().is_empty());
}

#[test]
fn linux_table_writev_rejects_total_byte_overflow_before_payload_reads() {
    let iov0 = rv64_iovec(0x9100, u64::MAX);
    let iov1 = rv64_iovec(0x9200, 1);
    let payload_reads = Arc::new(Mutex::new(Vec::new()));
    let payload_reads_for_reader = Arc::clone(&payload_reads);
    let reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            return Some(iov0.to_vec());
        }
        if address == 0x9010 && bytes == 16 {
            return Some(iov1.to_vec());
        }
        payload_reads_for_reader
            .lock()
            .unwrap()
            .push((address, bytes));
        None
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 2, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(payload_reads.lock().unwrap().is_empty());
    assert!(state.guest_writes().is_empty());
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(1).unwrap())
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn linux_table_writev_faults_when_iovec_or_payload_is_unreadable() {
    let mut state = RiscvSyscallState::new(0);
    let no_iovec_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&no_iovec_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.guest_writes().is_empty());

    let iov0 = rv64_iovec(0x9100, 2);
    let payload_fault_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        (address == 0x9000 && bytes == 16).then_some(iov0.to_vec())
    });
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITEV, [1, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&payload_fault_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.guest_writes().is_empty());
}

#[test]
fn user_ecall_writev_records_scatter_gather_bytes_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(80);
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
    let iov0 = rv64_iovec(0x9100, 2);
    let iov1 = rv64_iovec(0x9200, 3);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, RISCV_LINUX_WRITEV as i32)),
            (0x8004, addi(10, 0, 1)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 2)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x801c, addi(10, 5, 0)),
            (0x8020, 0x0000_0073),
        ],
        &[
            (0x9000, &iov0),
            (0x9010, &iov1),
            (0x9100, b"he"),
            (0x9200, b"llo"),
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
        .with_riscv_syscall_emulation_and_guest_memory_reader(guest_memory_reader(Arc::clone(
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
            180,
            |cpu| GuestEventId::new(640 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(640), source, 5);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 5);
    let state = driver.riscv_syscall_emulation().unwrap().state();
    assert_eq!(state.guest_writes().len(), 1);
    let write = &state.guest_writes()[0];
    assert_eq!(write.fd().get(), 1);
    assert_eq!(write.address(), 0x9000);
    assert_eq!(write.bytes(), b"hello");
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
