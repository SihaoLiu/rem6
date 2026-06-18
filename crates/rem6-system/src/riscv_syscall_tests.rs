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
#[path = "riscv_syscall_tests/guest_file_io_tests.rs"]
mod guest_file_io_tests;
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
