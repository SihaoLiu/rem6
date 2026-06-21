use super::*;

const RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST: u64 = 120;
const RISCV_LINUX_SCHED_GETPARAM_FOR_TEST: u64 = 121;
const RISCV_LINUX_SCHED_SETPARAM_FOR_TEST: u64 = 118;
const RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST: u64 = 119;
const RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST: u64 = 122;
const RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST: u64 = 123;
const RISCV_LINUX_SCHED_GET_PRIORITY_MAX_FOR_TEST: u64 = 125;
const RISCV_LINUX_SCHED_GET_PRIORITY_MIN_FOR_TEST: u64 = 126;
const RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST: u64 = 127;
const RISCV_LINUX_IOPRIO_SET_FOR_TEST: u64 = 30;
const RISCV_LINUX_IOPRIO_GET_FOR_TEST: u64 = 31;
const RISCV_LINUX_SETPRIORITY_FOR_TEST: u64 = 140;
const RISCV_LINUX_GETPRIORITY_FOR_TEST: u64 = 141;
const RISCV_LINUX_EPERM_FOR_TEST: u64 = 1;
const RISCV_LINUX_EACCES_FOR_TEST: u64 = 13;
const RISCV_LINUX_ESRCH_FOR_TEST: u64 = 3;
const RISCV_LINUX_SCHED_OTHER_FOR_TEST: u64 = 0;
const RISCV_LINUX_SCHED_FIFO_FOR_TEST: u64 = 1;
const RISCV_LINUX_SCHED_RR_FOR_TEST: u64 = 2;
const RISCV_LINUX_SCHED_BATCH_FOR_TEST: u64 = 3;
const RISCV_LINUX_SCHED_IDLE_FOR_TEST: u64 = 5;
const RISCV_LINUX_SCHED_DEADLINE_FOR_TEST: u64 = 6;
const RISCV_LINUX_SCHED_PRIORITY_BYTES_FOR_TEST: usize = 4;
const RISCV_LINUX_SCHED_RR_INTERVAL_NANOSECONDS_FOR_TEST: u64 = 2_000_000;
const RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST: u64 = 8;
const RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST: u64 = 1;
const RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST: u64 = 1;
const RISCV_LINUX_IOPRIO_CLASS_NONE_FOR_TEST: u64 = 0;
const RISCV_LINUX_IOPRIO_CLASS_RT_FOR_TEST: u64 = 1;
const RISCV_LINUX_IOPRIO_CLASS_BE_FOR_TEST: u64 = 2;
const RISCV_LINUX_IOPRIO_CLASS_IDLE_FOR_TEST: u64 = 3;
const RISCV_LINUX_IOPRIO_CLASS_SHIFT_FOR_TEST: u64 = 13;

#[test]
fn linux_table_sched_getscheduler_returns_other_for_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, pid) in [(0x8000, 0), (0x8004, 41), (0x8008, 42)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST,
                    [pid, 0, 0, 0, 0, 0],
                ),
                &mut state,
                10,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_SCHED_OTHER_FOR_TEST
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioprio_get_set_tracks_current_process_priority() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let idle = RISCV_LINUX_IOPRIO_CLASS_IDLE_FOR_TEST << RISCV_LINUX_IOPRIO_CLASS_SHIFT_FOR_TEST;
    let best_effort_5 =
        (RISCV_LINUX_IOPRIO_CLASS_BE_FOR_TEST << RISCV_LINUX_IOPRIO_CLASS_SHIFT_FOR_TEST) | 5;
    let high_bits_best_effort_5 = (1_u64 << 40) | (1_u64 << 16) | best_effort_5;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_IOPRIO_GET_FOR_TEST,
                [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_IOPRIO_SET_FOR_TEST,
                [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 0, idle, 0, 0, 0,],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_IOPRIO_GET_FOR_TEST,
                [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 41, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: idle })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_IOPRIO_SET_FOR_TEST,
                [
                    RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                    42,
                    best_effort_5,
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
                0x8010,
                RISCV_LINUX_IOPRIO_GET_FOR_TEST,
                [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: best_effort_5
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_IOPRIO_SET_FOR_TEST,
                [
                    RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                    0,
                    high_bits_best_effort_5,
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
                RISCV_LINUX_IOPRIO_GET_FOR_TEST,
                [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: best_effort_5
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioprio_rejects_invalid_targets_and_class_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let invalid_class = 4 << RISCV_LINUX_IOPRIO_CLASS_SHIFT_FOR_TEST;
    let realtime_class =
        RISCV_LINUX_IOPRIO_CLASS_RT_FOR_TEST << RISCV_LINUX_IOPRIO_CLASS_SHIFT_FOR_TEST;

    for (pc, number, arguments, errno) in [
        (
            0x8000,
            RISCV_LINUX_IOPRIO_GET_FOR_TEST,
            [99, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8004,
            RISCV_LINUX_IOPRIO_SET_FOR_TEST,
            [99, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            RISCV_LINUX_IOPRIO_GET_FOR_TEST,
            [RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST, 999, 0, 0, 0, 0],
            RISCV_LINUX_ESRCH_FOR_TEST,
        ),
        (
            0x800c,
            RISCV_LINUX_IOPRIO_SET_FOR_TEST,
            [
                RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                0,
                invalid_class,
                0,
                0,
                0,
            ],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8010,
            RISCV_LINUX_IOPRIO_SET_FOR_TEST,
            [
                RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                999,
                invalid_class,
                0,
                0,
                0,
            ],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8014,
            RISCV_LINUX_IOPRIO_SET_FOR_TEST,
            [
                RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                0,
                RISCV_LINUX_IOPRIO_CLASS_NONE_FOR_TEST | 5,
                0,
                0,
                0,
            ],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8018,
            RISCV_LINUX_IOPRIO_SET_FOR_TEST,
            [
                RISCV_LINUX_IOPRIO_WHO_PROCESS_FOR_TEST,
                0,
                realtime_class,
                0,
                0,
                0,
            ],
            RISCV_LINUX_EPERM_FOR_TEST,
        ),
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(pc, number, arguments), &mut state),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getscheduler_rejects_unknown_pid() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST,
                [999, 0, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getscheduler_rejects_32_bit_negative_pid() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST,
                [u64::from(u32::MAX), 0, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setscheduler_updates_current_process_policy() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let parameter_address = 0x9000;
    let guest_memory_reader = sched_param_reader(parameter_address, 0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_SCHED_BATCH_FOR_TEST,
                    parameter_address,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SCHED_GETSCHEDULER_FOR_TEST,
                [41, 0, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_SCHED_BATCH_FOR_TEST
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setparam_accepts_current_process_zero_priority() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let parameter_address = 0x9010;
    let guest_memory_reader = sched_param_reader(parameter_address, 0);

    for (pc, pid) in [(0x8000, 0), (0x8004, 41), (0x8008, 42)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETPARAM_FOR_TEST,
                    [pid, parameter_address, 0, 0, 0, 0],
                ),
                &mut state,
                12,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setparam_and_setscheduler_reject_invalid_requests() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, number, arguments, errno) in [
        (
            0x8000,
            RISCV_LINUX_SCHED_SETPARAM_FOR_TEST,
            [999, 0x9000, 0, 0, 0, 0],
            RISCV_LINUX_ESRCH_FOR_TEST,
        ),
        (
            0x8004,
            RISCV_LINUX_SCHED_SETPARAM_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [0, 4, 0x9000, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x800c,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [0, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0x9008, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8010,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [999, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0x9000, 0, 0, 0],
            RISCV_LINUX_ESRCH_FOR_TEST,
        ),
    ] {
        let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
            if address == 0x9000 && bytes == RISCV_LINUX_SCHED_PRIORITY_BYTES_FOR_TEST {
                Some(0_i32.to_le_bytes().to_vec())
            } else if address == 0x9008 && bytes == RISCV_LINUX_SCHED_PRIORITY_BYTES_FOR_TEST {
                Some(1_i32.to_le_bytes().to_vec())
            } else {
                None
            }
        });
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                13,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_param_rejects_null_pointer_before_target_checks() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("null sched_getparam must not write"));

    for (pc, number, arguments) in [
        (
            0x8000,
            RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
            [999, 0, 0, 0, 0, 0],
        ),
        (
            0x8004,
            RISCV_LINUX_SCHED_SETPARAM_FOR_TEST,
            [999, 0, 0, 0, 0, 0],
        ),
        (
            0x8008,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [999, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0, 0, 0, 0],
        ),
        (
            0x800c,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [0, 4, 0, 0, 0, 0],
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                13,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setters_report_parameter_fault_before_target_and_policy_checks() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let read_addresses = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let read_addresses_for_reader = std::sync::Arc::clone(&read_addresses);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        read_addresses_for_reader
            .lock()
            .unwrap()
            .push((address, bytes));
        None
    });

    for (pc, number, arguments) in [
        (
            0x8000,
            RISCV_LINUX_SCHED_SETPARAM_FOR_TEST,
            [999, 0x9000, 0, 0, 0, 0],
        ),
        (
            0x8004,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [999, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0x9004, 0, 0, 0],
        ),
        (
            0x8008,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [0, 4, 0x9008, 0, 0, 0],
        ),
        (
            0x800c,
            RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
            [999, 4, 0x900c, 0, 0, 0],
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                13,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EFAULT)
            })
        );
    }
    assert_eq!(
        *read_addresses.lock().unwrap(),
        vec![(0x9000, 4), (0x9004, 4), (0x9008, 4), (0x900c, 4)]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setscheduler_without_guest_reader_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETSCHEDULER_FOR_TEST,
                [0, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_get_priority_max_reports_supported_policy_limits() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, policy, expected_priority) in [
        (0x8000, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0),
        (0x8004, RISCV_LINUX_SCHED_FIFO_FOR_TEST, 99),
        (0x8008, RISCV_LINUX_SCHED_RR_FOR_TEST, 99),
        (0x800c, RISCV_LINUX_SCHED_BATCH_FOR_TEST, 0),
        (0x8010, RISCV_LINUX_SCHED_IDLE_FOR_TEST, 0),
        (0x8014, RISCV_LINUX_SCHED_DEADLINE_FOR_TEST, 0),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GET_PRIORITY_MAX_FOR_TEST,
                    [policy, 0, 0, 0, 0, 0],
                ),
                &mut state,
                10,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: expected_priority
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_get_priority_min_reports_supported_policy_limits() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, policy, expected_priority) in [
        (0x8000, RISCV_LINUX_SCHED_OTHER_FOR_TEST, 0),
        (0x8004, RISCV_LINUX_SCHED_FIFO_FOR_TEST, 1),
        (0x8008, RISCV_LINUX_SCHED_RR_FOR_TEST, 1),
        (0x800c, RISCV_LINUX_SCHED_BATCH_FOR_TEST, 0),
        (0x8010, RISCV_LINUX_SCHED_IDLE_FOR_TEST, 0),
        (0x8014, RISCV_LINUX_SCHED_DEADLINE_FOR_TEST, 0),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GET_PRIORITY_MIN_FOR_TEST,
                    [policy, 0, 0, 0, 0, 0],
                ),
                &mut state,
                10,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: expected_priority
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_get_priority_limits_reject_invalid_policy() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, syscall_number, policy) in [
        (0x8000, RISCV_LINUX_SCHED_GET_PRIORITY_MAX_FOR_TEST, 4),
        (
            0x8004,
            RISCV_LINUX_SCHED_GET_PRIORITY_MIN_FOR_TEST,
            u64::from(u32::MAX),
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, syscall_number, [policy, 0, 0, 0, 0, 0]),
                &mut state,
                10,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_priority_uses_raw_kernel_encoding() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 20 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETPRIORITY_FOR_TEST,
                [0, 41, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 20 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SETPRIORITY_FOR_TEST, [0, 0, 7, 0, 0, 0],),
            &mut state,
            12,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
            13,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 13 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SETPRIORITY_FOR_TEST,
                [0, 0, 40, 0, 0, 0],
            ),
            &mut state,
            14,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_GETPRIORITY_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
            15,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_priority_reports_linux_errors() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, number, arguments, errno) in [
        (
            0x8000,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [3, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8004,
            RISCV_LINUX_SETPRIORITY_FOR_TEST,
            [3, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [0, 999, 0, 0, 0, 0],
            RISCV_LINUX_ESRCH_FOR_TEST,
        ),
        (
            0x800c,
            RISCV_LINUX_SETPRIORITY_FOR_TEST,
            [0, 999, 0, 0, 0, 0],
            RISCV_LINUX_ESRCH_FOR_TEST,
        ),
        (
            0x8010,
            RISCV_LINUX_SETPRIORITY_FOR_TEST,
            [0, 0, u64::MAX, 0, 0, 0],
            RISCV_LINUX_EACCES_FOR_TEST,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                20,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_rr_get_interval_writes_current_process_interval() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, pid, interval_address) in [
        (0x8000, 0, 0x9000),
        (0x8004, 41, 0x9020),
        (0x8008, 42, 0x9040),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                    [pid, interval_address, 0, 0, 0, 0],
                ),
                &mut state,
                10,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        let interval = collect_guest_writes(&writes.lock().unwrap(), interval_address, 16);
        assert_eq!(read_le_u64(&interval, 0), 0);
        assert_eq!(
            read_le_u64(&interval, 8),
            RISCV_LINUX_SCHED_RR_INTERVAL_NANOSECONDS_FOR_TEST
        );
        writes.lock().unwrap().clear();
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_rr_get_interval_rejects_unknown_pid_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("unknown pid must not write RR interval"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [999, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_rr_get_interval_prioritizes_unknown_pid_over_null_interval() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("unknown pid must not write RR interval"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [999, 0, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_rr_get_interval_rejects_32_bit_negative_pid_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("negative pid must not write RR interval"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [u64::from(u32::MAX), 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_rr_get_interval_rejects_null_interval_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("null interval must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            10,
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
fn linux_table_sched_rr_get_interval_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, _bytes| address != 0x9008);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
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
fn linux_table_sched_rr_get_interval_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_RR_GET_INTERVAL_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_writes_zero_priority_for_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, pid, parameter_address) in [
        (0x8000, 0, 0x9000),
        (0x8004, 41, 0x9010),
        (0x8008, 42, 0x9020),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                    [pid, parameter_address, 0, 0, 0, 0],
                ),
                &mut state,
                10,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            written_priority_at(&writes.lock().unwrap(), parameter_address),
            0
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_rejects_unknown_pid_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("unknown pid must not write sched parameter"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [999, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_rejects_negative_pid_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("negative pid must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [u64::MAX, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_rejects_32_bit_negative_pid_without_writing() {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [u64::from(u32::MAX), 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_rejects_null_parameter_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("null parameter must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getparam_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, &[0; RISCV_LINUX_SCHED_PRIORITY_BYTES_FOR_TEST]);
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
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
fn linux_table_sched_getparam_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETPARAM_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_accepts_single_cpu_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mask_address = 0x9000;
    let guest_memory_reader =
        affinity_reader_at(mask_address, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    mask_address,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_accepts_current_process_and_thread_ids() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, pid, mask_address) in [(0x8000, 41, 0x9000), (0x8004, 42, 0x9010)] {
        let guest_memory_reader =
            affinity_reader_at(mask_address, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                    [
                        pid,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        mask_address,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                10,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_masks_without_guest_cpu_zero() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, mask) in [(0x8000, 0), (0x8004, 2)] {
        let guest_memory_reader = affinity_reader_at(0x9000, mask);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                    [
                        0,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        0x9000,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                10,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_short_sizes_after_reading_guest_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, 4);
        Some(RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()[..4].to_vec())
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 4, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_zero_size_without_guest_read() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 0, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_truncates_oversized_guest_mask_to_single_word() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        Some(
            RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST
                .to_le_bytes()
                .to_vec(),
        )
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 16, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_unknown_pid_after_reading_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader =
        affinity_reader_at(0x9000, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    999,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_reports_guest_read_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_without_guest_reader_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_writes_current_thread_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mask_address = 0x9000;
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
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    mask_address,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        written_mask_at(&writes.lock().unwrap(), mask_address),
        RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_accepts_current_process_and_thread_ids() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, pid, mask_address) in [(0x8000, 41, 0x9000), (0x8004, 42, 0x9010)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                    [
                        pid,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        mask_address,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                11,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST
            })
        );
        assert_eq!(
            written_mask_at(&writes.lock().unwrap(), mask_address),
            RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_rejects_short_and_unaligned_sizes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("invalid size must not write"));

    for (pc, size) in [(0x8000, 0), (0x8004, 4), (0x8008, 12)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                    [0, size, 0x9000, 0, 0, 0],
                ),
                &mut state,
                12,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_rejects_unknown_pid() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("unknown pid must not write affinity"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    999,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            13,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(
            bytes.len(),
            RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize
        );
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            14,
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
fn linux_table_sched_getaffinity_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            15,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn written_priority_at(writes: &[(u64, Vec<u8>)], address: u64) -> i32 {
    writes
        .iter()
        .find_map(|(written_address, bytes)| {
            (*written_address == address).then(|| {
                i32::from_le_bytes(
                    bytes
                        .as_slice()
                        .try_into()
                        .expect("sched priority write is one int"),
                )
            })
        })
        .unwrap_or_else(|| panic!("missing sched priority write at {address:#x}"))
}

fn written_mask_at(writes: &[(u64, Vec<u8>)], address: u64) -> [u8; 8] {
    writes
        .iter()
        .find_map(|(written_address, bytes)| {
            (*written_address == address).then(|| bytes.as_slice().try_into().unwrap())
        })
        .unwrap_or_else(|| panic!("missing affinity write at {address:#x}"))
}

fn affinity_reader_at(address: u64, mask: u64) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, bytes| {
        assert_eq!(read_address, address);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        Some(mask.to_le_bytes().to_vec())
    })
}

fn sched_param_reader_at(
    address: u64,
    priority: i32,
    expected_reads: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, bytes| {
        assert_eq!(read_address, address);
        expected_reads.lock().unwrap().push(bytes);
        (bytes == RISCV_LINUX_SCHED_PRIORITY_BYTES_FOR_TEST)
            .then(|| priority.to_le_bytes().to_vec())
    })
}

fn sched_param_reader(address: u64, priority: i32) -> RiscvGuestMemoryReader {
    let reads = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    sched_param_reader_at(address, priority, reads)
}
