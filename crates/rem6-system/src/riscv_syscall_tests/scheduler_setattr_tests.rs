use super::*;

const RISCV_LINUX_SCHED_SETATTR_FOR_TEST: u64 = 274;
const RISCV_LINUX_SCHED_GETATTR_FOR_TEST: u64 = 275;
const RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST: u64 = 120;
const RISCV_LINUX_GETPRIORITY_FOR_TEST: u64 = 141;
const RISCV_LINUX_SCHED_OTHER_FOR_TEST: u32 = 0;
const RISCV_LINUX_SCHED_BATCH_FOR_TEST: u32 = 3;
const RISCV_LINUX_SCHED_FIFO_FOR_TEST: u32 = 1;
const RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST: usize = 56;
const RISCV_LINUX_SCHED_ATTR_BYTES_VER0_FOR_TEST: u32 = 48;

#[test]
fn linux_table_sched_setattr_updates_current_process_scheduler_state() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let attr_address = 0x9000;
    let guest_memory_reader = sched_attr_reader_at(
        attr_address,
        sched_attr_bytes(
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u32,
            RISCV_LINUX_SCHED_BATCH_FOR_TEST,
            0,
            7,
            0,
        ),
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [0, attr_address, 0, 0, 0, 0],
            ),
            &mut state,
            19,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 13 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST,
                [41, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_SCHED_BATCH_FOR_TEST as u64
        })
    );

    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SCHED_GETATTR_FOR_TEST,
                [
                    0,
                    0x9100,
                    RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            20,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let attr = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9100,
        RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST,
    );
    assert_eq!(
        u32::from_le_bytes(attr[4..8].try_into().unwrap()),
        RISCV_LINUX_SCHED_BATCH_FOR_TEST
    );
    assert_eq!(i32::from_le_bytes(attr[16..20].try_into().unwrap()), 7);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_accepts_legacy_attr_size() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let attr_address = 0x9000;

    for (pc, size, expected_priority) in [
        (0x8000, RISCV_LINUX_SCHED_ATTR_BYTES_VER0_FOR_TEST, 17),
        (0x8004, 0, 16),
    ] {
        let guest_memory_reader = sched_attr_reader_at(
            attr_address,
            sched_attr_bytes(
                size,
                RISCV_LINUX_SCHED_OTHER_FOR_TEST,
                0,
                20 - expected_priority,
                0,
            ),
        );

        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                    [0, attr_address, 0, 0, 0, 0],
                ),
                &mut state,
                21,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }

    let guest_memory_reader = sched_attr_reader_at(
        attr_address,
        sched_attr_bytes(56, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, 25, 0),
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [0, attr_address, 0, 0, 0, 0],
            ),
            &mut state,
            21,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_rejects_invalid_request_headers_without_reading() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader =
        RiscvGuestMemoryReader::new(|_, _| panic!("invalid sched_setattr header must not read"));

    for (pc, pid, attr, flags) in [
        (0x8000, u64::MAX, 0x9000, 0),
        (0x8004, 0, 0, 0),
        (0x8008, 0, 0x9000, 1),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                    [pid, attr, flags, 0, 0, 0],
                ),
                &mut state,
                22,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_rejects_invalid_attributes_without_state_change() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    let size_writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let size_writes_for_writer = std::sync::Arc::clone(&size_writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        size_writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, bytes, errno) in [
        (
            0x8000,
            sched_attr_bytes(47, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, 0, 0),
            RISCV_LINUX_E2BIG,
        ),
        (
            0x8004,
            sched_attr_bytes(
                (RISCV_PAGE_BYTES + 1) as u32,
                RISCV_LINUX_SCHED_OTHER_FOR_TEST,
                0,
                0,
                0,
            ),
            RISCV_LINUX_E2BIG,
        ),
        (0x8008, sched_attr_bytes(56, 4, 0, 0, 0), RISCV_LINUX_EINVAL),
        (
            0x800c,
            sched_attr_bytes(56, RISCV_LINUX_SCHED_OTHER_FOR_TEST, u64::MAX, 0, 0),
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8010,
            sched_attr_bytes(56, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, 0, 1),
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8014,
            sched_attr_bytes(56, RISCV_LINUX_SCHED_FIFO_FOR_TEST, 0, 0, 1),
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8018,
            sched_attr_bytes(56, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, -1, 0),
            RISCV_LINUX_EPERM,
        ),
    ] {
        let guest_memory_reader = sched_attr_reader_at(0x9000, bytes);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                    [0, 0x9000, 0, 0, 0, 0],
                ),
                &mut state,
                23,
                Some(&guest_memory_reader),
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8020, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 20 })
    );
    assert_eq!(
        collect_guest_writes(&size_writes.lock().unwrap(), 0x9000, 4),
        (RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u32).to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_rejects_unknown_positive_pid_after_reading_attr() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_reader = sched_attr_reader_at(
        0x9000,
        sched_attr_bytes(56, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, 0, 0),
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [999, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            24,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_reports_guest_read_faults() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let size_fault_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, 4);
        None
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            25,
            Some(&size_fault_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    let body_fault_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        (bytes == 4).then(|| 56_u32.to_le_bytes().to_vec())
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            26,
            Some(&body_fault_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setattr_without_guest_reader_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETATTR_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            27,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn sched_attr_reader_at(
    address: u64,
    bytes: [u8; RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST],
) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, read_bytes| {
        assert_eq!(read_address, address);
        match read_bytes {
            4 => Some(bytes[0..4].to_vec()),
            read_bytes if read_bytes <= bytes.len() => Some(bytes[..read_bytes].to_vec()),
            _ => None,
        }
    })
}

fn sched_attr_bytes(
    size: u32,
    policy: u32,
    flags: u64,
    nice: i32,
    priority: u32,
) -> [u8; RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST] {
    let mut bytes = [0; RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST];
    bytes[0..4].copy_from_slice(&size.to_le_bytes());
    bytes[4..8].copy_from_slice(&policy.to_le_bytes());
    bytes[8..16].copy_from_slice(&flags.to_le_bytes());
    bytes[16..20].copy_from_slice(&nice.to_le_bytes());
    bytes[20..24].copy_from_slice(&priority.to_le_bytes());
    bytes
}
