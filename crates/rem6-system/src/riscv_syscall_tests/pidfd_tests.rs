use super::*;

const RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST: u64 = 424;
const RISCV_LINUX_PIDFD_OPEN_FOR_TEST: u64 = 434;
const RISCV_LINUX_PIDFD_GETFD_FOR_TEST: u64 = 438;
const RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST: u64 = 0x800;
const RISCV_LINUX_PIDFD_THREAD_FOR_TEST: u64 = 0o200;
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
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800a,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [99, 0x20, 0, 0, 0, 0],
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
                0x800b,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [(1_u64 << 32) | 41, 1_u64 << 32, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR | RISCV_LINUX_O_LARGEFILE_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800d,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [
                    42,
                    RISCV_LINUX_PIDFD_THREAD_FOR_TEST | RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800e,
                RISCV_LINUX_FCNTL,
                [4, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR
                | RISCV_LINUX_O_LARGEFILE_FOR_TEST
                | RISCV_LINUX_PIDFD_NONBLOCK_FOR_TEST
        })
    );
    assert!(state.guest_fds().entry(GuestFd::new(5).unwrap()).is_none());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pidfd_open_thread_flag_targets_current_thread_id() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIDFD_OPEN_FOR_TEST, [42, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PIDFD_OPEN_FOR_TEST,
                [42, RISCV_LINUX_PIDFD_THREAD_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 4, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
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
            RiscvSyscallRequest::new(
                0x8003,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [1, 0, 0, 8, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
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
                [3, 0, 0, 3, 0, 0],
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
                0x800e,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 1, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800f,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 2, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8011,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 4, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8012,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [3, 0, 0, 7, 0, 0],
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

#[test]
fn linux_table_pidfd_getfd_duplicates_current_process_fd() {
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
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIDFD_GETFD_FOR_TEST, [3, 3, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    let duplicate = GuestFd::new(4).unwrap();
    assert_eq!(
        state.guest_fds().entry(duplicate).unwrap().description(),
        state
            .guest_fds()
            .entry(GuestFd::new(3).unwrap())
            .unwrap()
            .description()
    );
    assert!(state.guest_fds().close_on_exec(duplicate).unwrap());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [4, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR | RISCV_LINUX_O_LARGEFILE_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [4, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8012,
                RISCV_LINUX_PIDFD_SEND_SIGNAL_FOR_TEST,
                [(1_u64 << 32) | 4, 0, 0, 1_u64 << 32, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pidfd_getfd_validates_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIDFD_GETFD_FOR_TEST, [1, 1, 1, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8002, RISCV_LINUX_PIDFD_GETFD_FOR_TEST, [1, 1, 0, 0, 0, 0],),
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
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_PIDFD_GETFD_FOR_TEST, [3, 1, 1, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800a,
                RISCV_LINUX_PIDFD_GETFD_FOR_TEST,
                [3, 99, 1_u64 << 32, 0, 0, 0],
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
                0x800b,
                RISCV_LINUX_PIDFD_GETFD_FOR_TEST,
                [(1_u64 << 32) | 3, (1_u64 << 32) | 3, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert!(state
        .guest_fds()
        .close_on_exec(GuestFd::new(4).unwrap())
        .unwrap());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PIDFD_GETFD_FOR_TEST,
                [3, 99, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
