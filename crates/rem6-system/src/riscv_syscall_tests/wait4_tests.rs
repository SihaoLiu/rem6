use super::super::*;
use crate::{GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestSignal, GuestWaitStatus};

const RISCV_LINUX_ECHILD_FOR_TEST: u64 = 10;
const RISCV_LINUX_WAITID_FOR_TEST: u64 = 95;
const RISCV_LINUX_WNOHANG_FOR_TEST: u64 = 1;
const RISCV_LINUX_WSTOPPED_FOR_TEST: u64 = 2;
const RISCV_LINUX_WEXITED_FOR_TEST: u64 = 4;
const RISCV_LINUX_WCONTINUED_FOR_TEST: u64 = 8;
const RISCV_LINUX_WNOWAIT_FOR_TEST: u64 = 0x0100_0000;
const RISCV_LINUX_WAIT4_WNOTHREAD_FOR_TEST: u64 = 0x2000_0000;
const RISCV_LINUX_WAIT4_WALL_FOR_TEST: u64 = 0x4000_0000;
const RISCV_LINUX_WAIT4_WCLONE_FOR_TEST: u64 = 0x8000_0000;
const RISCV_LINUX_P_ALL_FOR_TEST: u64 = 0;
const RISCV_LINUX_P_PID_FOR_TEST: u64 = 1;
const RISCV_LINUX_P_PGID_FOR_TEST: u64 = 2;
const RISCV64_LINUX_RUSAGE_BYTES: usize = 144;
const RISCV64_LINUX_SIGINFO_BYTES: usize = 128;
const RISCV64_LINUX_SIGINFO_SIGCHLD: i32 = 17;
const RISCV64_LINUX_CLD_EXITED: i32 = 1;
const RISCV64_LINUX_CLD_KILLED: i32 = 2;
const RISCV64_LINUX_CLD_DUMPED: i32 = 3;
const RISCV64_LINUX_CLD_STOPPED: i32 = 5;
const RISCV64_LINUX_CLD_CONTINUED: i32 = 6;
const RISCV64_LINUX_SIGCONT: i32 = 18;

fn child(pid: u32, process_group: u32, status: GuestWaitStatus) -> GuestChildStatus {
    GuestChildStatus::new(
        GuestProcessId::new(pid).unwrap(),
        GuestProcessGroupId::new(process_group).unwrap(),
        status,
    )
}

#[test]
fn linux_table_wait4_reaps_child_when_status_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(child(123, 100, GuestWaitStatus::exited(7)));
    let faulting_guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, &((7_i32) << 8).to_le_bytes());
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&faulting_guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.guest_wait_queue().is_empty());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WAIT4, [(-1_i64) as u64, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_wait4_wnohang_empty_queue_returns_echild() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0, RISCV_LINUX_WNOHANG_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_wait4_wnohang_unmatched_selector_returns_echild() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let other_child = child(123, 100, GuestWaitStatus::exited(7));
    state.push_wait_child(other_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [456, 0, RISCV_LINUX_WNOHANG_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[other_child]
    );
}

#[test]
fn linux_table_wait4_pid_zero_uses_process_group_not_credential_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let credential_group_child = child(91, 9, GuestWaitStatus::exited(1));
    let current_group_child = child(52, 41, GuestWaitStatus::exited(5));
    state.push_wait_child(credential_group_child);
    state.push_wait_child(current_group_child);

    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 52 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[credential_group_child]
    );
}

#[test]
fn linux_table_wait4_writes_zero_rusage_for_reaped_child() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(child(123, 100, GuestWaitStatus::exited(7)));
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
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0x9000, 0, 0xa000, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 123 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        writes.as_slice(),
        &[
            (0x9000, ((7_i32) << 8).to_le_bytes().to_vec()),
            (0xa000, vec![0; RISCV64_LINUX_RUSAGE_BYTES]),
        ]
    );
    assert!(state.guest_wait_queue().is_empty());
}

#[test]
fn linux_table_wait4_accepts_linux_option_bits_without_children() {
    let table = RiscvSyscallTable::new();
    let valid_options = [
        RISCV_LINUX_WSTOPPED_FOR_TEST,
        RISCV_LINUX_WCONTINUED_FOR_TEST,
        RISCV_LINUX_WSTOPPED_FOR_TEST | RISCV_LINUX_WCONTINUED_FOR_TEST,
        RISCV_LINUX_WAIT4_WNOTHREAD_FOR_TEST,
        RISCV_LINUX_WAIT4_WALL_FOR_TEST,
        RISCV_LINUX_WAIT4_WCLONE_FOR_TEST,
        RISCV_LINUX_WNOHANG_FOR_TEST
            | RISCV_LINUX_WSTOPPED_FOR_TEST
            | RISCV_LINUX_WCONTINUED_FOR_TEST
            | RISCV_LINUX_WAIT4_WNOTHREAD_FOR_TEST
            | RISCV_LINUX_WAIT4_WALL_FOR_TEST
            | RISCV_LINUX_WAIT4_WCLONE_FOR_TEST,
    ];

    for options in valid_options {
        let mut state = RiscvSyscallState::new(0);
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_WAIT4,
                    [(-1_i64) as u64, 0, options, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
            }),
            "wait4 options {options:#x}"
        );
    }

    let mut state = RiscvSyscallState::new(0);
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0, RISCV_LINUX_WEXITED_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_wait4_filters_stopped_and_continued_statuses() {
    let table = RiscvSyscallTable::new();
    let stopped = child(
        123,
        100,
        GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
    );
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(stopped);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0, RISCV_LINUX_WNOHANG_FOR_TEST, 0, 0, 0,],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.guest_wait_queue().snapshot().pending(), &[stopped]);

    let stopped_writes = capture_wait4_writes(
        &table,
        &mut state,
        [
            (-1_i64) as u64,
            0x9000,
            RISCV_LINUX_WSTOPPED_FOR_TEST,
            0,
            0,
            0,
        ],
        123,
    );
    assert_eq!(
        stopped_writes,
        vec![(0x9000, (((19_i32) << 8) | 0x7f).to_le_bytes().to_vec())]
    );
    assert!(state.guest_wait_queue().is_empty());

    let continued = child(124, 100, GuestWaitStatus::continued());
    state.push_wait_child(continued);
    let continued_writes = capture_wait4_writes(
        &table,
        &mut state,
        [
            (-1_i64) as u64,
            0x9000,
            RISCV_LINUX_WCONTINUED_FOR_TEST,
            0,
            0,
            0,
        ],
        124,
    );
    assert_eq!(
        continued_writes,
        vec![(0x9000, 0xffff_i32.to_le_bytes().to_vec())]
    );
    assert!(state.guest_wait_queue().is_empty());
}

#[test]
fn linux_table_waitid_wnohang_empty_queue_returns_echild_without_writing_siginfo() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, _| {
        panic!("waitid should not write siginfo for no-child result at {address:#x}");
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAITID_FOR_TEST,
                [
                    RISCV_LINUX_P_ALL_FOR_TEST,
                    0,
                    0x9000,
                    RISCV_LINUX_WNOHANG_FOR_TEST | RISCV_LINUX_WEXITED_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_waitid_rejects_missing_waitable_options() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAITID_FOR_TEST,
                [RISCV_LINUX_P_ALL_FOR_TEST, 0, 0x9000, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_waitid_writes_siginfo_for_reaped_child() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 77, 78, 79, 80));
    state.push_wait_child(child(123, 41, GuestWaitStatus::exited(7)));
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
                RISCV_LINUX_WAITID_FOR_TEST,
                [
                    RISCV_LINUX_P_PID_FOR_TEST,
                    123,
                    0x9000,
                    RISCV_LINUX_WEXITED_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 0x9000);
    let siginfo = &writes[0].1;
    assert_eq!(siginfo.len(), RISCV64_LINUX_SIGINFO_BYTES);
    assert_eq!(
        i32::from_le_bytes(siginfo[0..4].try_into().unwrap()),
        RISCV64_LINUX_SIGINFO_SIGCHLD
    );
    assert_eq!(i32::from_le_bytes(siginfo[4..8].try_into().unwrap()), 0);
    assert_eq!(
        i32::from_le_bytes(siginfo[8..12].try_into().unwrap()),
        RISCV64_LINUX_CLD_EXITED
    );
    assert_eq!(i32::from_le_bytes(siginfo[16..20].try_into().unwrap()), 123);
    assert_eq!(u32::from_le_bytes(siginfo[20..24].try_into().unwrap()), 77);
    assert_eq!(i32::from_le_bytes(siginfo[24..28].try_into().unwrap()), 7);
    assert_eq!(i64::from_le_bytes(siginfo[32..40].try_into().unwrap()), 0);
    assert_eq!(i64::from_le_bytes(siginfo[40..48].try_into().unwrap()), 0);
    assert!(siginfo[48..].iter().all(|byte| *byte == 0));
    assert!(state.guest_wait_queue().is_empty());
}

#[test]
fn linux_table_waitid_null_siginfo_reaps_child() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(child(123, 100, GuestWaitStatus::exited(7)));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAITID_FOR_TEST,
                [
                    RISCV_LINUX_P_PID_FOR_TEST,
                    123,
                    0,
                    RISCV_LINUX_WEXITED_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_wait_queue().is_empty());
}

#[test]
fn linux_table_waitid_wnowait_leaves_child_waitable() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 77, 78, 79, 80));
    let ready_child = child(123, 41, GuestWaitStatus::exited(7));
    state.push_wait_child(ready_child);
    let siginfo = capture_waitid_writes(
        &table,
        &mut state,
        [
            RISCV_LINUX_P_PID_FOR_TEST,
            123,
            0x9000,
            RISCV_LINUX_WEXITED_FOR_TEST | RISCV_LINUX_WNOWAIT_FOR_TEST,
            0,
            0,
        ],
    );

    assert_waitid_siginfo(&siginfo[0].1, 123, 77, RISCV64_LINUX_CLD_EXITED, 7);
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[ready_child]
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WAIT4, [123, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 123 })
    );
}

#[test]
fn linux_table_waitid_wnohang_unwaitable_child_writes_zero_siginfo() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let stopped_child = child(
        123,
        100,
        GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
    );
    state.push_wait_child(stopped_child);
    let writes = capture_waitid_writes(
        &table,
        &mut state,
        [
            RISCV_LINUX_P_ALL_FOR_TEST,
            0,
            0x9000,
            RISCV_LINUX_WEXITED_FOR_TEST | RISCV_LINUX_WNOHANG_FOR_TEST,
            0,
            0,
        ],
    );

    assert_eq!(
        writes.as_slice(),
        &[(0x9000, vec![0; RISCV64_LINUX_SIGINFO_BYTES])]
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[stopped_child]
    );
}

#[test]
fn linux_table_waitid_blocking_unwaitable_child_blocks_without_writing_siginfo() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let stopped_child = child(
        123,
        100,
        GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
    );
    state.push_wait_child(stopped_child);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, _| {
        panic!("blocking waitid should not write siginfo before readiness at {address:#x}");
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAITID_FOR_TEST,
                [
                    RISCV_LINUX_P_ALL_FOR_TEST,
                    0,
                    0x9000,
                    RISCV_LINUX_WEXITED_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[stopped_child]
    );
}

#[test]
fn linux_table_waitid_process_group_selects_matching_child() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 77, 78, 79, 80));
    let other_group = child(91, 100, GuestWaitStatus::exited(1));
    let selected = child(223, 200, GuestWaitStatus::exited(9));
    state.push_wait_child(other_group);
    state.push_wait_child(selected);
    let writes = capture_waitid_writes(
        &table,
        &mut state,
        [
            RISCV_LINUX_P_PGID_FOR_TEST,
            200,
            0x9000,
            RISCV_LINUX_WEXITED_FOR_TEST,
            0,
            0,
        ],
    );

    assert_waitid_siginfo(&writes[0].1, 223, 77, RISCV64_LINUX_CLD_EXITED, 9);
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[other_group]
    );
}

#[test]
fn linux_table_waitid_reaps_status_matching_requested_options() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 77, 78, 79, 80));
    let stopped = child(
        123,
        41,
        GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
    );
    let exited = child(123, 41, GuestWaitStatus::exited(7));
    state.push_wait_child(stopped);
    state.push_wait_child(exited);
    let writes = capture_waitid_writes(
        &table,
        &mut state,
        [
            RISCV_LINUX_P_PID_FOR_TEST,
            123,
            0x9000,
            RISCV_LINUX_WEXITED_FOR_TEST,
            0,
            0,
        ],
    );

    assert_waitid_siginfo(&writes[0].1, 123, 77, RISCV64_LINUX_CLD_EXITED, 7);
    assert_eq!(state.guest_wait_queue().snapshot().pending(), &[stopped]);
}

#[test]
fn linux_table_waitid_writes_siginfo_for_signal_stop_and_continue_statuses() {
    let killed = waitid_siginfo_for(
        GuestWaitStatus::signaled(GuestSignal::new(11).unwrap(), false),
        RISCV_LINUX_WEXITED_FOR_TEST,
    );
    assert_waitid_siginfo(&killed, 123, 77, RISCV64_LINUX_CLD_KILLED, 11);

    let dumped = waitid_siginfo_for(
        GuestWaitStatus::signaled(GuestSignal::new(6).unwrap(), true),
        RISCV_LINUX_WEXITED_FOR_TEST,
    );
    assert_waitid_siginfo(&dumped, 123, 77, RISCV64_LINUX_CLD_DUMPED, 6);

    let stopped = waitid_siginfo_for(
        GuestWaitStatus::stopped(GuestSignal::new(19).unwrap()),
        RISCV_LINUX_WSTOPPED_FOR_TEST,
    );
    assert_waitid_siginfo(&stopped, 123, 77, RISCV64_LINUX_CLD_STOPPED, 19);

    let continued = waitid_siginfo_for(
        GuestWaitStatus::continued(),
        RISCV_LINUX_WCONTINUED_FOR_TEST,
    );
    assert_waitid_siginfo(
        &continued,
        123,
        77,
        RISCV64_LINUX_CLD_CONTINUED,
        RISCV64_LINUX_SIGCONT,
    );
}

fn capture_wait4_writes(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    arguments: [u64; 6],
    expected_pid: u64,
) -> Vec<(u64, Vec<u8>)> {
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WAIT4, arguments),
            state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: expected_pid
        })
    );

    let writes = writes.lock().unwrap();
    writes.clone()
}

fn capture_waitid_writes(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    arguments: [u64; 6],
) -> Vec<(u64, Vec<u8>)> {
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WAITID_FOR_TEST, arguments),
            state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    writes.clone()
}

fn waitid_siginfo_for(status: GuestWaitStatus, wait_options: u64) -> Vec<u8> {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 77, 78, 79, 80));
    state.push_wait_child(child(123, 41, status));
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
                RISCV_LINUX_WAITID_FOR_TEST,
                [RISCV_LINUX_P_ALL_FOR_TEST, 0, 0x9000, wait_options, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_wait_queue().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 0x9000);
    writes[0].1.clone()
}

fn assert_waitid_siginfo(siginfo: &[u8], pid: i32, uid: u32, code: i32, status: i32) {
    assert_eq!(siginfo.len(), RISCV64_LINUX_SIGINFO_BYTES);
    assert_eq!(
        i32::from_le_bytes(siginfo[0..4].try_into().unwrap()),
        RISCV64_LINUX_SIGINFO_SIGCHLD
    );
    assert_eq!(i32::from_le_bytes(siginfo[4..8].try_into().unwrap()), 0);
    assert_eq!(i32::from_le_bytes(siginfo[8..12].try_into().unwrap()), code);
    assert_eq!(i32::from_le_bytes(siginfo[16..20].try_into().unwrap()), pid);
    assert_eq!(u32::from_le_bytes(siginfo[20..24].try_into().unwrap()), uid);
    assert_eq!(
        i32::from_le_bytes(siginfo[24..28].try_into().unwrap()),
        status
    );
    assert_eq!(i64::from_le_bytes(siginfo[32..40].try_into().unwrap()), 0);
    assert_eq!(i64::from_le_bytes(siginfo[40..48].try_into().unwrap()), 0);
    assert!(siginfo[48..].iter().all(|byte| *byte == 0));
}
