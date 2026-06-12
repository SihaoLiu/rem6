#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_READV: u64 = 65;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
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

fn readv_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let iov0 = rv64_iovec(0x9100, 2);
    let iov1 = rv64_iovec(0x9200, 3);
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8f00, b"/input.txt\0"),
            (0x9000, &iov0),
            (0x9010, &iov1),
            (0x9100, b"\0\0"),
            (0x9200, b"\0\0\0"),
        ],
    )
}

fn open_registered_input(state: &mut RiscvSyscallState, reader: &RiscvGuestMemoryReader) -> u64 {
    state.register_guest_file(b"/input.txt", b"abcdef");
    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_OPENAT,
            [RISCV_LINUX_AT_FDCWD, 0x8f00, 0, 0, 0, 0],
        ),
        state,
        0,
        Some(reader),
        None,
    );
    match outcome {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        other => panic!("unexpected openat outcome: {other:?}"),
    }
}

#[test]
fn linux_table_readv_writes_registered_file_bytes_across_iovecs() {
    let store = readv_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READV, [fd, 0x9000, 2, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 5 }));
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 2),
        Some(b"ab".to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9200, 3),
        Some(b"cde".to_vec())
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(fd as i32).unwrap())
            .unwrap()
            .get(),
        5
    );
}

#[test]
fn linux_table_readv_rejects_bad_or_write_only_fd_before_guest_reads() {
    let reader = RiscvGuestMemoryReader::new(|_address, _bytes| {
        panic!("fd validation should happen before iovec reads")
    });
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("fd validation should happen before guest writes")
    });
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READV, [99, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READV, [1, 0x9000, 1, 0, 0, 0]),
            &mut state,
            12,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_readv_rejects_excessive_iov_count_without_guest_reads() {
    let reader = RiscvGuestMemoryReader::new(|_address, _bytes| {
        panic!("invalid iov count should not read guest memory")
    });
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid iov count should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READV, [0, 0x9000, 1025, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_readv_faults_or_rejects_bad_iovecs_before_consuming_input() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("bad iovecs should not write guest memory")
    });
    let no_iovec_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"abcde");

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READV, [0, 0x9000, 1, 0, 0, 0]),
            &mut state,
            11,
            Some(&no_iovec_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.stdin_byte_count(), 5);

    let iov0 = rv64_iovec(0x9100, u64::MAX);
    let iov1 = rv64_iovec(0x9200, 1);
    let overflow_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            return Some(iov0.to_vec());
        }
        if address == 0x9010 && bytes == 16 {
            return Some(iov1.to_vec());
        }
        None
    });
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READV, [0, 0x9000, 2, 0, 0, 0]),
            &mut state,
            12,
            Some(&overflow_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(state.stdin_byte_count(), 5);
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(0).unwrap())
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn linux_table_readv_guest_write_failure_preserves_offset_and_stdin() {
    let iov0 = rv64_iovec(0x9100, 2);
    let reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        (address == 0x9000 && bytes == 16).then_some(iov0.to_vec())
    });
    let writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9100);
        assert_eq!(bytes, b"ab");
        false
    });
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"abcde");

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READV, [0, 0x9000, 1, 0, 0, 0]),
        &mut state,
        11,
        Some(&reader),
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.stdin_byte_count(), 5);
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(0).unwrap())
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn user_ecall_readv_returns_scatter_read_count_before_exit() {
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
            (0x8000, addi(17, 0, RISCV_LINUX_READV as i32)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 2)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8018, addi(10, 10, 0)),
            (0x801c, 0x0000_0073),
        ],
        &[
            (0x9000, &iov0),
            (0x9010, &iov1),
            (0x9100, b"\0\0"),
            (0x9200, b"\0\0\0"),
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
        .push_stdin_bytes(b"abcde");

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
            |cpu| GuestEventId::new(640 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(640), source, 5);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 2),
        Some(b"ab".to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9200, 3),
        Some(b"cde".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
