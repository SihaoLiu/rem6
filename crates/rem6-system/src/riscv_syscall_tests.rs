use super::*;
use crate::{
    GuestFd, GuestFileStatusFlags, GuestFutexAddress, GuestFutexKey, GuestFutexWaitRequest,
    GuestThreadGroupId, GuestThreadId,
};
use rem6_kernel::PartitionId;

#[path = "riscv_syscall_tests/boot_image_tests.rs"]
mod boot_image_tests;
#[path = "riscv_syscall_tests/copy_file_range_tests.rs"]
mod copy_file_range_tests;
#[path = "riscv_syscall_tests/cpu_tests.rs"]
mod cpu_tests;
#[path = "riscv_syscall_tests/cwd_tests.rs"]
mod cwd_tests;
#[path = "riscv_syscall_tests/dirent_tests.rs"]
mod dirent_tests;
#[path = "riscv_syscall_tests/exit_tests.rs"]
mod exit_tests;
#[path = "riscv_syscall_tests/fcntl_tests.rs"]
mod fcntl_tests;
#[path = "riscv_syscall_tests/fd_tests.rs"]
mod fd_tests;
#[path = "riscv_syscall_tests/futex_tests.rs"]
mod futex_tests;
#[path = "riscv_syscall_tests/hwprobe_tests.rs"]
mod hwprobe_tests;
#[path = "riscv_syscall_tests/link_tests.rs"]
mod link_tests;
#[path = "riscv_syscall_tests/memory_policy_tests.rs"]
mod memory_policy_tests;
#[path = "riscv_syscall_tests/mkdir_tests.rs"]
mod mkdir_tests;
#[path = "riscv_syscall_tests/mlock_tests.rs"]
mod mlock_tests;
#[path = "riscv_syscall_tests/mmap_tests.rs"]
mod mmap_tests;
#[path = "riscv_syscall_tests/msync_tests.rs"]
mod msync_tests;
#[path = "riscv_syscall_tests/nanosleep_tests.rs"]
mod nanosleep_tests;
#[path = "riscv_syscall_tests/open_tests.rs"]
mod open_tests;
#[path = "riscv_syscall_tests/permissions_tests.rs"]
mod permissions_tests;
#[path = "riscv_syscall_tests/poll_tests.rs"]
mod poll_tests;
#[path = "riscv_syscall_tests/positioned_io_tests.rs"]
mod positioned_io_tests;
#[path = "riscv_syscall_tests/process_tests.rs"]
mod process_tests;
#[path = "riscv_syscall_tests/random_tests.rs"]
mod random_tests;
#[path = "riscv_syscall_tests/readlink_tests.rs"]
mod readlink_tests;
#[path = "riscv_syscall_tests/rename_tests.rs"]
mod rename_tests;
#[path = "riscv_syscall_tests/robust_tests.rs"]
mod robust_tests;
#[path = "riscv_syscall_tests/scheduler_tests.rs"]
mod scheduler_tests;
#[path = "riscv_syscall_tests/sendfile_tests.rs"]
mod sendfile_tests;
#[path = "riscv_syscall_tests/signal_action_tests.rs"]
mod signal_action_tests;
#[path = "riscv_syscall_tests/signal_tests.rs"]
mod signal_tests;
#[path = "riscv_syscall_tests/startup_tests.rs"]
mod startup_tests;
#[path = "riscv_syscall_tests/stat_tests.rs"]
mod stat_tests;
#[path = "riscv_syscall_tests/statfs_tests.rs"]
mod statfs_tests;
#[path = "riscv_syscall_tests/sync_tests.rs"]
mod sync_tests;
#[path = "riscv_syscall_tests/sysinfo_tests.rs"]
mod sysinfo_tests;
#[path = "riscv_syscall_tests/time_tests.rs"]
mod time_tests;
#[path = "riscv_syscall_tests/truncate_tests.rs"]
mod truncate_tests;
#[path = "riscv_syscall_tests/unknown_syscall_tests.rs"]
mod unknown_syscall_tests;
#[path = "riscv_syscall_tests/unlink_tests.rs"]
mod unlink_tests;
#[path = "riscv_syscall_tests/utsname_tests.rs"]
mod utsname_tests;
#[path = "riscv_syscall_tests/wait4_tests.rs"]
mod wait4_tests;
#[path = "riscv_syscall_tests/xattr_tests.rs"]
mod xattr_tests;

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

    let utsname = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 390);
    assert_eq!(&utsname[0..6], b"Linux\0");
    assert_eq!(&utsname[65..78], b"sim.gem5.org\0");
    assert_eq!(&utsname[130..136], b"5.1.0\0");
    assert_eq!(&utsname[195..227], b"#1 Mon Aug 18 11:32:15 EDT 2003\0");
    assert_eq!(&utsname[260..268], b"riscv64\0");
    assert!(utsname[325..390].iter().all(|byte| *byte == 0));
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
            RiscvSyscallRequest::new(
                0x8000,
                thread::RISCV_LINUX_SET_TID_ADDRESS,
                [0x1234, 0, 0, 0, 0, 0],
            ),
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
            RiscvSyscallRequest::new(
                0x8000,
                thread::RISCV_LINUX_SET_TID_ADDRESS,
                [0x1234, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, thread::RISCV_LINUX_SET_TID_ADDRESS, [0; 6]),
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

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SCHED_YIELD, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigreturn_records_unsupported_frame_restore() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_RT_SIGRETURN, [0; 6]),
            &mut state,
            43,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_RT_SIGRETURN,
            [0; 6],
            43
        )]
    );
}

#[test]
fn linux_table_setrlimit_updates_stack_limit() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let requested_limit = [2_u64 * 1024 * 1024, RISCV_LINUX_STACK_LIMIT_BYTES]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            Some(requested_limit.clone())
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETRLIMIT, [3, 0x9000, 0, 0, 0, 0]),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETRLIMIT, [3, 0x9100, 0, 0, 0, 0]),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let after = collect_guest_writes(&writes.lock().unwrap(), 0x9100, 16);
    assert_eq!(read_le_u64(&after, 0), 2 * 1024 * 1024);
    assert_eq!(read_le_u64(&after, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_setrlimit_null_limit_returns_efault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETRLIMIT, [3, 0, 0, 0, 0, 0]),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_setrlimit_bad_limit_precedes_invalid_resource() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, 16);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETRLIMIT, [999, 0x9000, 0, 0, 0, 0]),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_prlimit64_updates_stack_limit_and_reports_previous_limit() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let requested_limit = [4_u64 * 1024 * 1024, RISCV_LINUX_STACK_LIMIT_BYTES]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            Some(requested_limit.clone())
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRLIMIT64, [0, 3, 0x9000, 0x9100, 0, 0],),
            &mut state,
            10,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PRLIMIT64, [0, 3, 0, 0x9200, 0, 0],),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    let previous_writes = writes
        .iter()
        .filter(|(address, _)| *address >= 0x9100 && *address < 0x9110)
        .cloned()
        .collect::<Vec<_>>();
    let after_writes = writes
        .iter()
        .filter(|(address, _)| *address >= 0x9200 && *address < 0x9210)
        .cloned()
        .collect::<Vec<_>>();

    let previous = collect_guest_writes(&previous_writes, 0x9100, 16);
    assert_eq!(read_le_u64(&previous, 0), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(read_le_u64(&previous, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
    let after = collect_guest_writes(&after_writes, 0x9200, 16);
    assert_eq!(read_le_u64(&after, 0), 4 * 1024 * 1024);
    assert_eq!(read_le_u64(&after, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_prlimit64_write_fault_after_set_commits_stack_limit() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let requested_limit = [4_u64 * 1024 * 1024, RISCV_LINUX_STACK_LIMIT_BYTES]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            Some(requested_limit.clone())
        } else {
            None
        }
    });
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if (0x9100..0x9110).contains(&address) {
            return false;
        }
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRLIMIT64, [0, 3, 0x9000, 0x9100, 0, 0],),
            &mut state,
            10,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PRLIMIT64, [0, 3, 0, 0x9200, 0, 0],),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let after = collect_guest_writes(&writes.lock().unwrap(), 0x9200, 16);
    assert_eq!(read_le_u64(&after, 0), 4 * 1024 * 1024);
    assert_eq!(read_le_u64(&after, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_prlimit64_bad_limit_precedes_missing_pid_and_invalid_resource() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, 16);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRLIMIT64, [999_999, 3, 0x9000, 0, 0, 0],),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PRLIMIT64, [0, 999, 0x9000, 0, 0, 0],),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_prlimit64_updates_data_and_nproc_limits() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let data_limit = [128_u64 * 1024 * 1024, 256_u64 * 1024 * 1024]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let nproc_limit = [0_u64, 1_u64]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == 16 {
            Some(data_limit.clone())
        } else if address == 0xa000 && bytes == 16 {
            Some(nproc_limit.clone())
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRLIMIT64, [0, 2, 0x9000, 0, 0, 0]),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PRLIMIT64, [0, 6, 0xa000, 0, 0, 0]),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_PRLIMIT64, [0, 2, 0, 0x9100, 0, 0]),
            &mut state,
            12,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_PRLIMIT64, [0, 6, 0, 0x9200, 0, 0]),
            &mut state,
            13,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let data_writes = writes
        .iter()
        .filter(|(address, _)| *address >= 0x9100 && *address < 0x9110)
        .cloned()
        .collect::<Vec<_>>();
    let nproc_writes = writes
        .iter()
        .filter(|(address, _)| *address >= 0x9200 && *address < 0x9210)
        .cloned()
        .collect::<Vec<_>>();
    let data = collect_guest_writes(&data_writes, 0x9100, 16);
    let nproc = collect_guest_writes(&nproc_writes, 0x9200, 16);

    assert_eq!(read_le_u64(&data, 0), 128 * 1024 * 1024);
    assert_eq!(read_le_u64(&data, 8), 256 * 1024 * 1024);
    assert_eq!(read_le_u64(&nproc, 0), 0);
    assert_eq!(read_le_u64(&nproc, 8), 1);
}

#[test]
fn linux_table_ignores_gem5_memory_management_advisory_syscalls() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MUNLOCKALL, [0x4000, 4096, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_rseq_registration_requires_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, thread::RISCV_LINUX_RSEQ, [0x4000, 32, 0, 0, 0, 0],),
            &mut state,
        ),
        None
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
fn linux_table_openat_create_rejects_existing_symlink_without_mutation() {
    const RISCV_LINUX_EEXIST_FOR_TEST: u64 = 17;
    const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;

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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o644,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_TEST)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0x9100, 9, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(&*writes.lock().unwrap(), &[(0x9100, b"/bin/rem6".to_vec())]);
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
    const NEWLIB_O_CLOEXEC: u64 = 0x40000;

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
                [0x9000, NEWLIB_O_CLOEXEC, 0o644, 0, 0, 0],
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
fn linux_table_openat_creates_writable_guest_file_and_reads_written_contents() {
    const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"out.txt\0".to_vec();
    let payload = b"new file bytes".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY
                        | RISCV_LINUX_O_CREAT_FOR_TEST
                        | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0o644,
                    0,
                    0,
                ],
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [3, 0xa000, 14, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 14 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xb000, 32, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 14 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"new file bytes".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"out.txt").unwrap().size(), 14);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_writev_persists_created_guest_file_contents() {
    const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"vector.txt\0".to_vec();
    let first = b"vector ".to_vec();
    let second = b"write".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address == 0xa000 && bytes == 16 {
            let mut iovec = Vec::new();
            iovec.extend_from_slice(&0xa100_u64.to_le_bytes());
            iovec.extend_from_slice(&(first.len() as u64).to_le_bytes());
            return Some(iovec);
        }
        if address == 0xa010 && bytes == 16 {
            let mut iovec = Vec::new();
            iovec.extend_from_slice(&0xa200_u64.to_le_bytes());
            iovec.extend_from_slice(&(second.len() as u64).to_le_bytes());
            return Some(iovec);
        }
        if address >= 0xa100 && address < 0xa100 + first.len() as u64 {
            let start = usize::try_from(address - 0xa100).ok()?;
            let end = start.checked_add(bytes)?;
            return first.get(start..end).map(Vec::from);
        }
        if address >= 0xa200 && address < 0xa200 + second.len() as u64 {
            let start = usize::try_from(address - 0xa200).ok()?;
            let end = start.checked_add(bytes)?;
            return second.get(start..end).map(Vec::from);
        }
        None
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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY
                        | RISCV_LINUX_O_CREAT_FOR_TEST
                        | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0o644,
                    0,
                    0,
                ],
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITEV, [3, 0xa000, 2, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 12 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xb000, 32, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 12 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"vector write".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"vector.txt").unwrap().size(), 12);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_read_only_create_makes_empty_guest_file() {
    const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"empty.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        None
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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o644,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(state.guest_path_stat(b"empty.txt").unwrap().size(), 0);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0xa000, 16, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_openat_read_only_truncate_clears_guest_file() {
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file("existing.txt", b"old bytes");
    let path = b"existing.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        None
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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0o644,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(state.guest_path_stat(b"existing.txt").unwrap().size(), 0);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0xa000, 16, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_open_guest_file_write_is_visible_to_already_open_reader() {
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file("shared.txt", b"before");
    let path = b"shared.txt\0".to_vec();
    let payload = b"after".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && (0x9000..0xa000).contains(&address) {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
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
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [4, 0xa000, 5, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0xb000, 16, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(&*writes.lock().unwrap(), &[(0xb000, b"after".to_vec())]);
    assert_eq!(state.guest_path_stat(b"shared.txt").unwrap().size(), 5);
}

#[test]
fn linux_table_open_guest_file_write_updates_open_hard_link_reader() {
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file("source.txt", b"old");
    let source = b"source.txt\0".to_vec();
    let alias = b"alias.txt\0".to_vec();
    let payload = b"new".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && (0x9000..0x9100).contains(&address) {
            return source
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if bytes == 1 && (0x9100..0xa000).contains(&address) {
            return alias
                .get((address - 0x9100) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_LINK, [0x9000, 0x9100, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9100, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_WRITE, [4, 0xa000, 3, 0, 0, 0]),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xb000, 16, 0, 0, 0]),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(&*writes.lock().unwrap(), &[(0xb000, b"new".to_vec())]);
    assert_eq!(state.guest_path_stat(b"alias.txt").unwrap().size(), 3);
}

#[test]
fn linux_table_write_rejects_dense_guest_file_growth_past_limit() {
    const RISCV_LINUX_EFBIG_FOR_TEST: u64 = 27;
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;
    const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file("sparse.txt", b"");
    let path = b"sparse.txt\0".to_vec();
    let payload = b"x".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && (0x9000..0xa000).contains(&address) {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_LSEEK,
                [3, 64 * 1024 * 1024, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0,],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: 64 * 1024 * 1024
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [3, 0xa000, 1, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFBIG_FOR_TEST)
        })
    );
    assert_eq!(state.guest_path_stat(b"sparse.txt").unwrap().size(), 0);
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_LSEEK,
                [3, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xb000, 4, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_writev_rejects_dense_guest_file_growth_past_limit() {
    const RISCV_LINUX_EFBIG_FOR_TEST: u64 = 27;
    const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;
    const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file("vector-sparse.txt", b"");
    let path = b"vector-sparse.txt\0".to_vec();
    let payload = b"x".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && (0x9000..0xa000).contains(&address) {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address == 0xa000 && bytes == 16 {
            let mut iovec = Vec::new();
            iovec.extend_from_slice(&0xa100_u64.to_le_bytes());
            iovec.extend_from_slice(&(payload.len() as u64).to_le_bytes());
            return Some(iovec);
        }
        if address >= 0xa100 {
            let start = usize::try_from(address - 0xa100).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_LSEEK,
                [3, 64 * 1024 * 1024, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0,],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: 64 * 1024 * 1024
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITEV, [3, 0xa000, 1, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFBIG_FOR_TEST)
        })
    );
    assert_eq!(
        state.guest_path_stat(b"vector-sparse.txt").unwrap().size(),
        0
    );
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
