use super::*;
use crate::{
    GuestFd, GuestFileStatusFlags, GuestFutexAddress, GuestFutexKey, GuestFutexWaitRequest,
    GuestThreadGroupId, GuestThreadId,
};
use rem6_kernel::PartitionId;

#[path = "riscv_syscall_tests/boot_image_tests.rs"]
mod boot_image_tests;
#[path = "riscv_syscall_tests/dirent_tests.rs"]
mod dirent_tests;
#[path = "riscv_syscall_tests/link_tests.rs"]
mod link_tests;
#[path = "riscv_syscall_tests/mmap_tests.rs"]
mod mmap_tests;
#[path = "riscv_syscall_tests/rename_tests.rs"]
mod rename_tests;
#[path = "riscv_syscall_tests/startup_tests.rs"]
mod startup_tests;
#[path = "riscv_syscall_tests/stat_tests.rs"]
mod stat_tests;
#[path = "riscv_syscall_tests/unlink_tests.rs"]
mod unlink_tests;
#[path = "riscv_syscall_tests/wait4_tests.rs"]
mod wait4_tests;

fn read_le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_le_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn collect_guest_writes(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    for (address, chunk) in writes {
        let offset = usize::try_from(address.checked_sub(base).unwrap()).unwrap();
        bytes[offset..offset + chunk.len()].copy_from_slice(chunk);
    }
    bytes
}

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
fn linux_table_returns_enosys_for_unknown_syscalls() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, 9999, [1, 2, 3, 4, 5, 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            9999,
            [1, 2, 3, 4, 5, 6],
            0
        )]
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
fn linux_table_uname_writes_riscv64_utsname() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x8000, 160, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let utsname = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 325);
    assert_eq!(&utsname[0..6], b"Linux\0");
    assert_eq!(&utsname[65..78], b"sim.gem5.org\0");
    assert_eq!(&utsname[130..136], b"5.1.0\0");
    assert_eq!(&utsname[195..227], b"#1 Mon Aug 18 11:32:15 EDT 2003\0");
    assert_eq!(&utsname[260..268], b"riscv64\0");
}

#[test]
fn linux_table_uname_returns_efault_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, 160, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_leaves_uname_unhandled_without_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, 160, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            None,
        ),
        None
    );
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
fn linux_table_get_robust_list_writes_recorded_head_and_length() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SET_ROBUST_LIST,
                [0x9000, 24, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GET_ROBUST_LIST,
                [0, 0x9100, 0x9108, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let robust_list = collect_guest_writes(&writes.lock().unwrap(), 0x9100, 16);
    assert_eq!(read_le_u64(&robust_list, 0), 0x9000);
    assert_eq!(read_le_u64(&robust_list, 8), 24);
}

#[test]
fn linux_table_get_robust_list_accepts_current_thread_id() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SET_ROBUST_LIST,
                [0x9400, 24, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GET_ROBUST_LIST,
                [100, 0x9500, 0x9508, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let robust_list = collect_guest_writes(&writes.lock().unwrap(), 0x9500, 16);
    assert_eq!(read_le_u64(&robust_list, 0), 0x9400);
    assert_eq!(read_le_u64(&robust_list, 8), 24);
}

#[test]
fn linux_table_get_robust_list_reports_efault_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SET_ROBUST_LIST,
                [0x9000, 24, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GET_ROBUST_LIST,
                [0, 0x9100, 0x9108, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_ignores_gem5_warn_once_startup_syscalls() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [
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
fn linux_table_getrandom_writes_deterministic_guest_bytes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 4, 4, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, 278, [0x9200, 2, 0, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );

    assert_eq!(
        &*writes.lock().unwrap(),
        &[
            (0x9100, vec![0x2b, 0x2a, 0x29, 0x28]),
            (0x9200, vec![0x2f, 0x2e]),
        ]
    );
}

#[test]
fn linux_table_getrandom_rejects_unknown_flags_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 1, 8, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_getrandom_rejects_random_insecure_flag_combo() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 1, 6, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, 278, [0x9200, 0, 7, 0, 0, 0]),
            &mut state,
            8,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_getrandom_caps_large_guest_count_to_bounded_chunk() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 4096, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 256 })
    );
    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 0x9100);
    assert_eq!(writes[0].1.len(), 256);
    assert_eq!(&writes[0].1[..4], &[0x2b, 0x2a, 0x29, 0x28]);
}

#[test]
fn linux_table_getrandom_guest_fault_does_not_advance_stream() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let faulting_guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
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
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 2, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&faulting_guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, 278, [0x9200, 2, 0, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(&*writes.lock().unwrap(), &[(0x9200, vec![0x2b, 0x2a])]);
}

#[test]
fn linux_table_getrandom_zero_count_returns_without_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, 278, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_readlinkat_writes_registered_guest_link_without_nul() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let path = b"/proc/self/exe\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
                0x8000,
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 5, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(&*writes.lock().unwrap(), &[(0x9100, b"/bin/".to_vec())]);
}

#[test]
fn linux_table_readlinkat_requires_guest_memory_io() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let path = b"/proc/self/exe\0".to_vec();
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
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 5, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 5, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        None
    );
}

#[test]
fn linux_table_readlinkat_rejects_zero_buffer_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let path = b"/proc/self/exe\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
                0x8000,
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_registered_symlink_does_not_open_or_stat_as_guest_file() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let path = b"/proc/self/exe\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_NEWFSTATAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
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
fn linux_table_legacy_open_uses_registered_guest_path_arguments() {
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
                RISCV_LINUX_OPEN,
                [0x9000, RISCV_LINUX_O_CLOEXEC, 0o644, 0, 0, 0],
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
    assert_eq!(open.dirfd(), RISCV_LINUX_AT_FDCWD);
    assert_eq!(open.path(), b"/input.txt");
    assert_eq!(open.flags(), RISCV_LINUX_O_CLOEXEC);
    assert_eq!(open.mode(), 0o644);
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
fn linux_table_reads_registered_guest_file_contents_by_open_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"hello");
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0x9100, 3, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [3, 0x9200, 8, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9300, 1, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0x9100, b"hel".to_vec()), (0x9200, b"lo".to_vec())]
    );
    assert_eq!(state.stdin_byte_count(), 1);
}

#[test]
fn linux_table_newfstatat_writes_registered_guest_file_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"hello");
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
                0x8000,
                RISCV_LINUX_NEWFSTATAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    0x9100,
                    RISCV_LINUX_AT_NO_AUTOMOUNT,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(stat.len(), 128);
    assert_eq!(read_le_u64(&stat, 48), 5);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
    assert_eq!(read_le_u32(&stat, 20), 1);
    assert_eq!(read_le_u32(&stat, 24), 100);
    assert_eq!(read_le_u32(&stat, 28), 100);
    assert_eq!(read_le_u64(&stat, 56), 8192);
}

#[test]
fn linux_table_fstat_writes_open_registered_guest_file_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FSTAT, [3, 0x9100, 0, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 6);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
}

#[test]
fn linux_table_fstat_keeps_registered_path_without_contents_regular() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_path(b"/empty.txt");
    let path = b"/empty.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FSTAT, [3, 0x9100, 0, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 0);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
}

#[test]
fn linux_table_newfstatat_empty_path_stats_guest_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        if address == 0x9000 && bytes == 1 {
            Some(vec![0])
        } else {
            None
        }
    });
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
                0x8000,
                RISCV_LINUX_NEWFSTATAT,
                [1, 0x9000, 0x9100, RISCV_LINUX_AT_EMPTY_PATH, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 0);
    assert_eq!(read_le_u32(&stat, 16), 0o020666);
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
fn linux_table_futex_wait_mismatch_returns_eagain_without_queueing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x188);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, 0, 0, 0]),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
    assert!(!state.guest_futexes().is_waiting(GuestThreadId::new(100)));
}

#[test]
fn linux_table_futex_wait_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18c);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| panic!("zero bitset must fail first"));

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 9, 2, 0, 0, 0]),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_futex_wake_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x190);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0]),
            &mut state,
            43,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_unknown_numbers_return_enosys_and_record_request() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 9999, [0; 6]),
            &mut state,
            43
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(state.program_break(), 0);
    assert!(state.guest_writes().is_empty());
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(0x8000, 9999, [0; 6], 43)]
    );
}
