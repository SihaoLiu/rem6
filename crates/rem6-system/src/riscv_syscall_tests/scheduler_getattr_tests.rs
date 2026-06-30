use super::*;

const RISCV_LINUX_SCHED_GETATTR_FOR_TEST: u64 = 275;
const RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST: usize = 56;
const RISCV_LINUX_SCHED_OTHER_FOR_TEST: u32 = 0;

#[test]
fn linux_table_sched_getattr_writes_current_process_attributes() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let attr_address = 0x9000;
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, pid, address, requested_size, expected_write_len) in [
        (
            0x8000,
            0,
            attr_address,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST,
        ),
        (
            0x8004,
            41,
            attr_address + 0x80,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST,
        ),
        (0x8008, 0, attr_address + 0x100, 48, 48),
        (
            0x800c,
            0,
            attr_address + 0x180,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64 + 8,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETATTR_FOR_TEST,
                    [pid, address, requested_size, 0, 0, 0],
                ),
                &mut state,
                15,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        let mut written = writes.lock().unwrap();
        assert_eq!(written.len(), 1);
        assert_eq!(written[0].0, address);
        assert_eq!(written[0].1.len(), expected_write_len);
        let attr = collect_guest_writes(&written, address, expected_write_len);
        assert_eq!(
            u32::from_le_bytes(attr[0..4].try_into().unwrap()),
            expected_write_len as u32
        );
        assert_eq!(
            u32::from_le_bytes(attr[4..8].try_into().unwrap()),
            RISCV_LINUX_SCHED_OTHER_FOR_TEST
        );
        assert_eq!(i32::from_le_bytes(attr[16..20].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(attr[20..24].try_into().unwrap()), 0);
        written.clear();
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getattr_rejects_invalid_requests_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("invalid sched_getattr must not write"));

    for (pc, pid, address, size, flags, errno) in [
        (
            0x8000,
            999,
            0x9000,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            0,
            RISCV_LINUX_ESRCH,
        ),
        (
            0x8004,
            u64::MAX,
            0x9000,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            0,
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            0,
            0,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            0,
            RISCV_LINUX_EINVAL,
        ),
        (0x800c, 0, 0x9000, 0, 0, RISCV_LINUX_EINVAL),
        (0x8010, 0, 0x9000, 47, 0, RISCV_LINUX_EINVAL),
        (
            0x8014,
            0,
            0x9000,
            RISCV_PAGE_BYTES + 1,
            0,
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8018,
            0,
            0x9000,
            RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
            1,
            RISCV_LINUX_EINVAL,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETATTR_FOR_TEST,
                    [pid, address, size, flags, 0, 0],
                ),
                &mut state,
                16,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getattr_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes.len(), RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST);
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETATTR_FOR_TEST,
                [
                    0,
                    0x9000,
                    RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            17,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getattr_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETATTR_FOR_TEST,
                [
                    0,
                    0x9000,
                    RISCV_LINUX_SCHED_ATTR_BYTES_FOR_TEST as u64,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            18,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}
