use super::*;
use crate::{
    GuestFd, GuestFileStatusFlags, GuestFutexAddress, GuestFutexKey, GuestFutexWaitRequest,
    GuestThreadGroupId, GuestThreadId,
};
use rem6_kernel::PartitionId;

#[test]
fn linux_table_maps_exit_numbers_to_stop_codes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 17 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT_GROUP, [19; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 19 })
    );
}

#[test]
fn linux_table_tracks_program_break() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [64, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 64 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_BRK, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 64 })
    );
    assert_eq!(state.program_break(), 64);
}

#[test]
fn linux_table_returns_process_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (number, value) in [
        (RISCV_LINUX_GETPID, 41),
        (RISCV_LINUX_GETTID, 42),
        (RISCV_LINUX_GETPPID, 43),
        (RISCV_LINUX_GETUID, 7),
        (RISCV_LINUX_GETEUID, 8),
        (RISCV_LINUX_GETGID, 9),
        (RISCV_LINUX_GETEGID, 10),
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
            Some(RiscvSyscallOutcome::Return { value })
        );
    }
}

#[test]
fn linux_table_uses_gem5_default_process_identity() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [
        RISCV_LINUX_GETPID,
        RISCV_LINUX_GETTID,
        RISCV_LINUX_GETUID,
        RISCV_LINUX_GETEUID,
        RISCV_LINUX_GETGID,
        RISCV_LINUX_GETEGID,
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
            Some(RiscvSyscallOutcome::Return { value: 100 })
        );
    }
}

#[test]
fn linux_table_returns_parent_process_identity() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPPID, [77, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_records_child_clear_tid_address() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SET_TID_ADDRESS, [0x1234, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    assert_eq!(state.child_clear_tid(), Some(0x1234));
}

#[test]
fn linux_table_clears_child_clear_tid_address_with_zero() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SET_TID_ADDRESS, [0x1234, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SET_TID_ADDRESS, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    assert_eq!(state.child_clear_tid(), None);
}

#[test]
fn linux_table_ignores_gem5_warn_once_startup_syscalls() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [
        RISCV_LINUX_SET_ROBUST_LIST,
        RISCV_LINUX_GET_ROBUST_LIST,
        RISCV_LINUX_NANOSLEEP,
        RISCV_LINUX_SCHED_YIELD,
        RISCV_LINUX_RT_SIGSUSPEND,
        RISCV_LINUX_RT_SIGACTION,
        RISCV_LINUX_RT_SIGPROCMASK,
        RISCV_LINUX_RT_SIGPENDING,
        RISCV_LINUX_RT_SIGTIMEDWAIT,
        RISCV_LINUX_RT_SIGQUEUEINFO,
        RISCV_LINUX_RT_SIGRETURN,
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
}

#[test]
fn linux_table_ignores_gem5_memory_management_advisory_syscalls() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [
        RISCV_LINUX_MPROTECT,
        RISCV_LINUX_MSYNC,
        RISCV_LINUX_MLOCK,
        RISCV_LINUX_MUNLOCK,
        RISCV_LINUX_MLOCKALL,
        RISCV_LINUX_MUNLOCKALL,
        RISCV_LINUX_MINCORE,
        RISCV_LINUX_MADVISE,
        RISCV_LINUX_MBIND,
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, number, [0x4000, 4096, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
}

#[test]
fn linux_table_returns_enosys_for_gem5_ignored_rseq() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_RSEQ, [0x4000, 32, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
}

#[test]
fn linux_table_handles_fcntl_descriptor_and_status_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let stdout = GuestFd::new(1).unwrap();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETFD, RISCV_LINUX_FD_CLOEXEC, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_fds().close_on_exec(stdout).unwrap());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
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
                [1, RISCV_LINUX_F_SETFL, RISCV_LINUX_O_NONBLOCK, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        state.guest_fds().status_flags(stdout).unwrap(),
        GuestFileStatusFlags::new(RISCV_LINUX_O_WRONLY as u32 | RISCV_LINUX_O_NONBLOCK as u32)
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_NONBLOCK
        })
    );
}

#[test]
fn linux_table_leaves_write_unhandled_without_guest_memory_reader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WRITE, [1, 0x9000, 5, 0, 0, 0]),
            &mut state,
            7,
            None,
        ),
        None
    );
    assert!(state.guest_writes().is_empty());
}

#[test]
fn linux_table_leaves_read_unhandled_without_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READ, [0, 0x9000, 5, 0, 0, 0]),
            &mut state,
            7,
            None,
        ),
        None
    );
    assert_eq!(state.stdin_byte_count(), 0);
}

#[test]
fn linux_table_leaves_openat_unhandled_without_guest_memory_reader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
        ),
        None
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_opens_registered_guest_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_path(b"/input.txt");
    let path = b"/input.txt\0".to_vec();
    let guest_memory = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_CLOEXEC, 0, 0, 0,],
            ),
            &mut state,
            7,
            Some(&guest_memory),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );

    let fd = GuestFd::new(3).unwrap();
    assert!(state.guest_fds().entry(fd).is_some());
    assert!(state.guest_fds().close_on_exec(fd).unwrap());
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap(),
        GuestFileStatusFlags::new(RISCV_LINUX_O_RDONLY as u32)
    );
    assert_eq!(state.guest_opens().len(), 1);
    let open = &state.guest_opens()[0];
    assert_eq!(open.fd(), fd);
    assert_eq!(open.path(), b"/input.txt");
    assert_eq!(open.flags(), RISCV_LINUX_O_CLOEXEC);
}

#[test]
fn linux_table_opened_guest_path_fd_does_not_read_stdin() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_path(b"/input.txt");
    state.push_stdin_bytes(b"Z");
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0x9100, 1, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(state.stdin_byte_count(), 1);
}

#[test]
fn linux_table_dup_preserves_stdin_read_source() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"Q");
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|address, bytes| address == 0x9100 && bytes == b"Q");

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_DUP, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0x9100, 1, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(state.stdin_byte_count(), 0);
}

#[test]
fn linux_table_closes_guest_fd_and_rejects_reuse() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let stdout = GuestFd::new(1).unwrap();
    let stdout_description = GuestFileDescriptionId::new(1);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CLOSE, [1, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_fds().entry(stdout).is_none());
    assert!(state.guest_fds().description(stdout_description).is_none());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_returns_ebadf_for_close_on_unknown_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CLOSE, [99, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_duplicates_guest_fd_to_lowest_free_slot() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let duplicate = GuestFd::new(3).unwrap();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_DUP, [1, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_fds().entry(duplicate).unwrap().description(),
        GuestFileDescriptionId::new(1)
    );
    assert!(!state.guest_fds().close_on_exec(duplicate).unwrap());
}

#[test]
fn linux_table_dup3_replaces_destination_and_honors_close_on_exec() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let stderr = GuestFd::new(2).unwrap();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_DUP3,
                [1, 2, RISCV_LINUX_O_CLOEXEC, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        state.guest_fds().entry(stderr).unwrap().description(),
        GuestFileDescriptionId::new(1)
    );
    assert!(state.guest_fds().close_on_exec(stderr).unwrap());
    assert!(state
        .guest_fds()
        .description(GuestFileDescriptionId::new(2))
        .is_none());
}

#[test]
fn linux_table_rejects_bad_dup3_requests() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_DUP3, [1, 1, 0, 0, 0, 0]),
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
                RISCV_LINUX_DUP3,
                [1, 3, RISCV_LINUX_O_NONBLOCK, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_DUP3, [99, 3, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_returns_ebadf_for_fcntl_on_unknown_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [99, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_returns_ebadf_for_fcntl_on_out_of_range_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [(1_u64 << 32) | 1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_leaves_unsupported_fcntl_commands_unhandled_before_fd_validation() {
    const RISCV_LINUX_F_DUPFD: u64 = 0;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [u64::MAX, RISCV_LINUX_F_DUPFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        None
    );
}

#[test]
fn linux_table_wakes_guest_futex_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x180);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(7),
            PartitionId::new(1),
            20,
            3,
            3,
        ))
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(8),
            PartitionId::new(2),
            21,
            3,
            3,
        ))
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 1, 1, 0, 0, 0]),
            &mut state,
            40,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(8)]
    );
}

#[test]
fn linux_table_wakes_guest_futex_waiters_by_bitset() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x184);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(9), PartitionId::new(1), 22, 4, 4)
                .with_bitset(0b01),
        )
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(10), PartitionId::new(2), 23, 4, 4)
                .with_bitset(0b10),
        )
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 0, 0, 0, 0b01]),
            &mut state,
            41,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(10)]
    );
}

#[test]
fn linux_table_allocates_anonymous_mmap_regions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 64, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 1, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES
        })
    );
    assert_eq!(
        state.mmap_next(),
        RISCV64_LINUX_MMAP_BASE + (2 * RISCV_PAGE_BYTES)
    );
}

#[test]
fn linux_table_rejects_invalid_mmap_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 0, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MMAP, [1, 64, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.mmap_regions().is_empty());
}

#[test]
fn linux_table_fixed_mmap_preserves_non_overlapping_fragments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let fixed_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [
                    fixed_start,
                    RISCV_PAGE_BYTES,
                    1,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: fixed_start })
    );
    assert_eq!(
        state.mmap_regions(),
        &[
            RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                fixed_start,
                RISCV_PAGE_BYTES,
                1,
                34 | RISCV_LINUX_MAP_FIXED,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                fixed_start + RISCV_PAGE_BYTES,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                2 * RISCV_PAGE_BYTES,
            ),
        ]
    );
}

#[test]
fn linux_table_munmap_removes_mapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let unmap_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MUNMAP,
                [unmap_start, RISCV_PAGE_BYTES, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        state.mmap_regions(),
        &[
            RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                unmap_start + RISCV_PAGE_BYTES,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                2 * RISCV_PAGE_BYTES,
            ),
        ]
    );
}

#[test]
fn linux_table_rejects_invalid_munmap_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    for arguments in [
        [RISCV64_LINUX_MMAP_BASE + 1, RISCV_PAGE_BYTES, 0, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, 0, 0, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, u64::MAX, 0, 0, 0, 0],
        [
            u64::MAX - (RISCV_PAGE_BYTES - 1),
            RISCV_PAGE_BYTES,
            0,
            0,
            0,
            0,
        ],
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MUNMAP, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
    }
}

#[test]
fn linux_table_rejects_overflowing_fixed_mmap() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [
                    u64::MAX - (RISCV_PAGE_BYTES - 1),
                    RISCV_PAGE_BYTES,
                    3,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.mmap_regions().is_empty());
}

#[test]
fn linux_table_leaves_unknown_numbers_for_the_trap_path() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new()
            .handle(RiscvSyscallRequest::new(0x8000, 9999, [0; 6]), &mut state,),
        None
    );
}
