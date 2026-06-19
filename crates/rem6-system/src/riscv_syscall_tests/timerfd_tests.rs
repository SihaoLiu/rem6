use super::*;

const RISCV_LINUX_TIMERFD_CREATE_FOR_TEST: u64 = 85;
const RISCV_LINUX_TIMERFD_SETTIME_FOR_TEST: u64 = 86;
const RISCV_LINUX_TIMERFD_GETTIME_FOR_TEST: u64 = 87;
const RISCV_LINUX_READ_FOR_TEST: u64 = 63;
const RISCV_LINUX_WRITE_FOR_TEST: u64 = 64;
const RISCV_LINUX_CLOSE_FOR_TEST: u64 = 57;
const RISCV_LINUX_PPOLL_FOR_TEST: u64 = 73;
const RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST: u64 = 1;
const RISCV_LINUX_O_NONBLOCK_FOR_TEST: u64 = 0x800;
const RISCV_LINUX_POLLIN_FOR_TEST: i16 = 0x0001;

#[test]
fn linux_table_timerfd_expires_from_deterministic_tick_and_feeds_readiness() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    let fd = match table.handle(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_TIMERFD_CREATE_FOR_TEST,
            [
                RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST,
                RISCV_LINUX_O_NONBLOCK_FOR_TEST,
                0,
                0,
                0,
                0,
            ],
        ),
        &mut state,
    ) {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        outcome => panic!("unexpected timerfd_create outcome: {outcome:?}"),
    };
    assert_eq!(fd, 3);

    let read_writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let read_writes_for_writer = std::sync::Arc::clone(&read_writes);
    let read_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        read_writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ_FOR_TEST, [fd, 0x9000, 8, 0, 0, 0],),
            &mut state,
            10,
            None,
            Some(&read_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert!(read_writes.lock().unwrap().is_empty());

    let old_writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let old_writes_for_writer = std::sync::Arc::clone(&old_writes);
    let old_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        old_writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    let timer_spec = timer_spec_bytes(0, 0, 0, 1);
    let guest_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address != 0xa000 || bytes != timer_spec.len() {
            return None;
        }
        Some(timer_spec.clone())
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_TIMERFD_SETTIME_FOR_TEST,
                [fd, 0, 0xa000, 0xb000, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_reader),
            Some(&old_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&old_writes.lock().unwrap(), 0xb000, 32),
        timer_spec_bytes(0, 0, 0, 0)
    );

    let pollfd = pollfd_bytes(fd as i32, RISCV_LINUX_POLLIN_FOR_TEST);
    let zero_timeout = timespec_bytes(0, 0);
    let poll_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xd000 && bytes == zero_timeout.len() {
            return Some(zero_timeout.clone());
        }
        if address >= 0xc000 {
            let offset = usize::try_from(address - 0xc000).ok()?;
            let end = offset.checked_add(bytes)?;
            return pollfd.get(offset..end).map(Vec::from);
        }
        None
    });
    let poll_writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let poll_writes_for_writer = std::sync::Arc::clone(&poll_writes);
    let poll_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        poll_writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PPOLL_FOR_TEST,
                [0xc000, 1, 0xd000, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&poll_reader),
            Some(&poll_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    let poll_writes = poll_writes.lock().unwrap();
    assert_eq!(
        poll_writes
            .iter()
            .find(|(address, _bytes)| *address == 0xc006),
        Some(&(0xc006, RISCV_LINUX_POLLIN_FOR_TEST.to_le_bytes().to_vec()))
    );
    assert_eq!(
        poll_writes
            .iter()
            .find(|(address, _bytes)| *address == 0xd000),
        Some(&(0xd000, timespec_bytes(0, 0)))
    );
    drop(poll_writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ_FOR_TEST, [fd, 0x9000, 8, 0, 0, 0],),
            &mut state,
            11,
            None,
            Some(&read_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    let bytes = collect_guest_writes(&read_writes.lock().unwrap(), 0x9000, 8);
    assert_eq!(read_le_u64(&bytes, 0), 1);
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_CLOSE_FOR_TEST, [fd, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_timerfd_rejects_write_as_invalid_operation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    let fd = match table.handle(
        RiscvSyscallRequest::new(
            0x8018,
            RISCV_LINUX_TIMERFD_CREATE_FOR_TEST,
            [
                RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST,
                RISCV_LINUX_O_NONBLOCK_FOR_TEST,
                0,
                0,
                0,
                0,
            ],
        ),
        &mut state,
    ) {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        outcome => panic!("unexpected timerfd_create outcome: {outcome:?}"),
    };
    let payload = 1_u64.to_le_bytes();
    let guest_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == payload.len() {
            return Some(payload.to_vec());
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_WRITE_FOR_TEST, [fd, 0x9000, 8, 0, 0, 0],),
            &mut state,
            12,
            Some(&guest_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_timerfd_validates_args_and_reports_non_timer_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_TIMERFD_CREATE_FOR_TEST,
                [99, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_TIMERFD_CREATE_FOR_TEST,
                [RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST, 0x40, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_TIMERFD_GETTIME_FOR_TEST,
                [1, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x802c,
                RISCV_LINUX_TIMERFD_GETTIME_FOR_TEST,
                [99, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn timer_spec_bytes(
    interval_seconds: u64,
    interval_nanoseconds: u64,
    value_seconds: u64,
    value_nanoseconds: u64,
) -> Vec<u8> {
    [
        interval_seconds.to_le_bytes(),
        interval_nanoseconds.to_le_bytes(),
        value_seconds.to_le_bytes(),
        value_nanoseconds.to_le_bytes(),
    ]
    .concat()
}

fn timespec_bytes(seconds: u64, nanoseconds: u64) -> Vec<u8> {
    [seconds.to_le_bytes(), nanoseconds.to_le_bytes()].concat()
}

fn pollfd_bytes(fd: i32, events: i16) -> Vec<u8> {
    [
        fd.to_le_bytes().as_slice(),
        events.to_le_bytes().as_slice(),
        0_i16.to_le_bytes().as_slice(),
    ]
    .concat()
}
