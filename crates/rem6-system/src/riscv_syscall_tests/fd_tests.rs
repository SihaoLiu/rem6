use super::*;
use crate::GuestProcessGroupId;

const RISCV_LINUX_FLOCK_FOR_TEST: u64 = 32;
const RISCV_LINUX_FADVISE64_FOR_TEST: u64 = 223;
const RISCV_LINUX_LOCK_SH_FOR_TEST: u64 = 1;
const RISCV_LINUX_LOCK_EX_FOR_TEST: u64 = 2;
const RISCV_LINUX_LOCK_NB_FOR_TEST: u64 = 4;
const RISCV_LINUX_LOCK_UN_FOR_TEST: u64 = 8;
const RISCV_LINUX_POSIX_FADV_NORMAL_FOR_TEST: u64 = 0;
const RISCV_LINUX_POSIX_FADV_RANDOM_FOR_TEST: u64 = 1;
const RISCV_LINUX_POSIX_FADV_SEQUENTIAL_FOR_TEST: u64 = 2;
const RISCV_LINUX_POSIX_FADV_WILLNEED_FOR_TEST: u64 = 3;
const RISCV_LINUX_POSIX_FADV_DONTNEED_FOR_TEST: u64 = 4;
const RISCV_LINUX_POSIX_FADV_NOREUSE_FOR_TEST: u64 = 5;
const RISCV_LINUX_F_SETOWN_FOR_TEST: u64 = 8;
const RISCV_LINUX_F_GETOWN_FOR_TEST: u64 = 9;
const RISCV_LINUX_UNSUPPORTED_FCNTL_FOR_TEST: u64 = 0x7fff_ffff;

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
fn linux_table_reports_bad_fd_before_unsupported_fcntl_command() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [99, RISCV_LINUX_UNSUPPORTED_FCNTL_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_fcntl_setown_getown_tracks_shared_file_description_owner() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let current_process = RiscvSyscallIdentity::linux_single_process().thread_group_id();
    let current_process_group = -(current_process as i64) as u64;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETOWN_FOR_TEST, current_process, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: current_process
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_DUP, [1, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: current_process
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCNTL,
                [
                    3,
                    RISCV_LINUX_F_SETOWN_FOR_TEST,
                    current_process_group,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: current_process_group
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x801c,
                RISCV_LINUX_FCNTL,
                [
                    1,
                    RISCV_LINUX_F_SETOWN_FOR_TEST,
                    (1_u64 << 32) | current_process,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: current_process
        })
    );
}

#[test]
fn linux_table_fcntl_setown_validates_owner_targets() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::with_identity_process_group_and_session(
        0,
        RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10),
        GuestProcessGroupId::new(77).unwrap(),
        77,
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETOWN_FOR_TEST, 41, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETOWN_FOR_TEST, 999, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETOWN_FOR_TEST, (-999_i64) as u64, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [
                    1,
                    RISCV_LINUX_F_SETOWN_FOR_TEST,
                    (i32::MIN as i64) as u64,
                    0,
                    0,
                    0,
                ],
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
                0x8014,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETOWN_FOR_TEST, (-77_i64) as u64, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x801c,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: (-77_i64) as u64
        })
    );
}

#[test]
fn linux_table_flock_accepts_advisory_locks_for_open_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FLOCK_FOR_TEST,
                [
                    1,
                    RISCV_LINUX_LOCK_EX_FOR_TEST | RISCV_LINUX_LOCK_NB_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FLOCK_FOR_TEST,
                [1, RISCV_LINUX_LOCK_UN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_flock_reports_bad_fd_and_invalid_operations() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FLOCK_FOR_TEST,
                [99, RISCV_LINUX_LOCK_EX_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FLOCK_FOR_TEST, [1, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FLOCK_FOR_TEST,
                [
                    1,
                    RISCV_LINUX_LOCK_SH_FOR_TEST | RISCV_LINUX_LOCK_EX_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                ],
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
                RISCV_LINUX_FLOCK_FOR_TEST,
                [1, RISCV_LINUX_LOCK_UN_FOR_TEST | 16, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_fadvise64_accepts_known_advice_for_open_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let fd = open_regular_fadvise_fd(&table, &mut state, 0x7100);

    for advice in [
        RISCV_LINUX_POSIX_FADV_NORMAL_FOR_TEST,
        RISCV_LINUX_POSIX_FADV_RANDOM_FOR_TEST,
        RISCV_LINUX_POSIX_FADV_SEQUENTIAL_FOR_TEST,
        RISCV_LINUX_POSIX_FADV_WILLNEED_FOR_TEST,
        RISCV_LINUX_POSIX_FADV_DONTNEED_FOR_TEST,
        RISCV_LINUX_POSIX_FADV_NOREUSE_FOR_TEST,
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x9000,
                    RISCV_LINUX_FADVISE64_FOR_TEST,
                    [fd, 0, 4096, advice, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
}

#[test]
fn linux_table_fadvise64_rejects_bad_fd_and_advice() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let fd = open_regular_fadvise_fd(&table, &mut state, 0x7100);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x9000,
                RISCV_LINUX_FADVISE64_FOR_TEST,
                [99, 0, 4096, RISCV_LINUX_POSIX_FADV_NORMAL_FOR_TEST, 0, 0],
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
                0x9004,
                RISCV_LINUX_FADVISE64_FOR_TEST,
                [fd, 0, 4096, 6, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_fadvise64_rejects_pipe_fd_and_negative_len() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let fd = open_regular_fadvise_fd(&table, &mut state, 0x7100);
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
            11,
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
                    RISCV_LINUX_FADVISE64_FOR_TEST,
                    [fd, 0, 4096, RISCV_LINUX_POSIX_FADV_NORMAL_FOR_TEST, 0, 0,],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ESPIPE)
            })
        );
    }
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x9008,
                RISCV_LINUX_FADVISE64_FOR_TEST,
                [
                    fd,
                    0,
                    u64::MAX,
                    RISCV_LINUX_POSIX_FADV_NORMAL_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

fn open_regular_fadvise_fd(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    path_address: u64,
) -> u64 {
    state.register_guest_file(b"/fadvise.bin", b"advise\n");
    let path = b"/fadvise.bin\0".to_vec();
    let reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        let offset = address.checked_sub(path_address)?;
        let offset = usize::try_from(offset).ok()?;
        let end = offset.checked_add(bytes)?;
        path.get(offset..end).map(<[u8]>::to_vec)
    });
    match table.handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8ff0,
            RISCV_LINUX_OPENAT,
            [
                RISCV_LINUX_AT_FDCWD,
                path_address,
                RISCV_LINUX_O_RDONLY,
                0,
                0,
                0,
            ],
        ),
        state,
        10,
        Some(&reader),
        None,
    ) {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        outcome => panic!("openat for fadvise regular file failed: {outcome:?}"),
    }
}
