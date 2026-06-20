use super::*;

const RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST: u64 = 424;
const RISCV_LINUX_PIDFD_OPEN_FOR_TEST: u64 = 434;
const RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST: u64 = 0x800;
const RISCV_LINUX_O_LARGEFILE_FOR_TEST: u64 = 0x8000;
const SIGUSR1: u64 = 10;

#[test]
fn linux_table_pidfd_open_returns_current_process_fd() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [41, RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_FD_CLOEXEC
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR
                | RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST
                | RISCV_LINUX_O_LARGEFILE_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pidfd_open_rejects_bad_pid_and_flags() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIDFD_OPEN_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [41, 0x20, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_PIDFD_OPEN_FOR_TEST, [99, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.guest_fds().entry(GuestFd::new(3).unwrap()).is_none());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pidfd_send_signal_validates_fd_and_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [1, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8002,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [1, 65, 0x9000, 1, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIDFD_OPEN_FOR_TEST, [41, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 65, 0, 0, 0, 0],
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
                0x800c,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 1, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pidfd_send_signal_records_unimplemented_delivery() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIDFD_OPEN_FOR_TEST, [41, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, SIGUSR1, 0, 0, 0, 0],
            ),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8004,
            RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
            [3, SIGUSR1, 0, 0, 0, 0],
            9
        )]
    );
}
