use super::*;

const RISCV_LINUX_SYNC_FOR_TEST: u64 = 81;
const RISCV_LINUX_FSYNC_FOR_TEST: u64 = 82;
const RISCV_LINUX_FDATASYNC_FOR_TEST: u64 = 83;
const RISCV_LINUX_SYNC_FILE_RANGE_FOR_TEST: u64 = 84;
const RISCV_LINUX_SYNCFS_FOR_TEST: u64 = 267;

#[test]
fn linux_table_handles_sync_family_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SYNC_FOR_TEST, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for (pc, syscall) in [
        (0x8004, RISCV_LINUX_FSYNC_FOR_TEST),
        (0x8008, RISCV_LINUX_FDATASYNC_FOR_TEST),
        (0x800c, RISCV_LINUX_SYNCFS_FOR_TEST),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, syscall, [1, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc + 0x100, syscall, [99, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc + 0x200, syscall, [(1_u64 << 32) | 1, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sync_file_range_rejects_pipe_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let pipe_fds = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let pipe_fds_for_writer = std::sync::Arc::clone(&pipe_fds);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if address == 0x7000 && bytes.len() == 8 {
            pipe_fds_for_writer.lock().unwrap().extend_from_slice(bytes);
            true
        } else {
            false
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x9000, RISCV_LINUX_PIPE2, [0x7000, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let pipe_fds = pipe_fds.lock().unwrap();
    let read_fd = u32::from_le_bytes(pipe_fds[0..4].try_into().unwrap()) as u64;
    let write_fd = u32::from_le_bytes(pipe_fds[4..8].try_into().unwrap()) as u64;

    for fd in [read_fd, write_fd] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x9004,
                    RISCV_LINUX_SYNC_FILE_RANGE_FOR_TEST,
                    [fd, 0, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ESPIPE)
            })
        );
    }

    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_handles_sync_file_range_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SYNC_FILE_RANGE_FOR_TEST,
                [1, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for (pc, arguments, error) in [
        (0x8004, [99, 0, 0, 0, 0, 0], RISCV_LINUX_EBADF),
        (
            0x8008,
            [(1_u64 << 32) | 1, 0, 0, 0, 0, 0],
            RISCV_LINUX_EBADF,
        ),
        (0x800c, [1, 0, 0, 8, 0, 0], RISCV_LINUX_EINVAL),
        (0x8010, [1, u64::MAX, 0, 0, 0, 0], RISCV_LINUX_EINVAL),
        (0x8014, [1, 0, u64::MAX, 0, 0, 0], RISCV_LINUX_EINVAL),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SYNC_FILE_RANGE_FOR_TEST, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(error)
            })
        );
    }

    assert!(state.unknown_syscalls().is_empty());
}
