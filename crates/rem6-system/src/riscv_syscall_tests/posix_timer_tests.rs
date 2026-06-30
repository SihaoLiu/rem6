use super::*;

const RISCV_LINUX_TIMER_CREATE_FOR_TEST: u64 = 107;
const RISCV_LINUX_TIMER_GETTIME_FOR_TEST: u64 = 108;
const RISCV_LINUX_TIMER_GETOVERRUN_FOR_TEST: u64 = 109;
const RISCV_LINUX_TIMER_SETTIME_FOR_TEST: u64 = 110;
const RISCV_LINUX_TIMER_DELETE_FOR_TEST: u64 = 111;
const RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST: u64 = 1;
const RISCV_LINUX_SIGEV_NONE_FOR_TEST: u32 = 1;
const RISCV_LINUX_TIMER_ABSTIME_FOR_TEST: u64 = 1;
const RISCV_LINUX_ITIMERSPEC_BYTES_FOR_TEST: usize = 32;
const RISCV_LINUX_SIGEVENT_BYTES_FOR_TEST: usize = 64;

#[test]
fn linux_table_posix_timer_lifecycle_uses_deterministic_tick() {
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
                RISCV_LINUX_TIMER_CREATE_FOR_TEST,
                [RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST, 0, 0x9000, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let timer_id = u64::from(read_le_u32(
        &collect_guest_writes(&writes.lock().unwrap(), 0x9000, 4),
        0,
    ));
    assert_eq!(timer_id, 0);
    writes.lock().unwrap().clear();

    let new_value = itimerspec_bytes(0, 100, 2, 0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xa000 && bytes == new_value.len() {
            Some(new_value.clone())
        } else {
            None
        }
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_TIMER_SETTIME_FOR_TEST,
                [timer_id, 0, 0xa000, 0xb000, 0, 0],
            ),
            &mut state,
            1_000_000_000,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(
            &writes.lock().unwrap(),
            0xb000,
            RISCV_LINUX_ITIMERSPEC_BYTES_FOR_TEST,
        ),
        itimerspec_bytes(0, 0, 0, 0)
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_TIMER_GETTIME_FOR_TEST,
                [timer_id, 0xc000, 0, 0, 0, 0],
            ),
            &mut state,
            1_500_000_000,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let current = collect_guest_writes(
        &writes.lock().unwrap(),
        0xc000,
        RISCV_LINUX_ITIMERSPEC_BYTES_FOR_TEST,
    );
    assert_eq!(read_le_u64(&current, 0), 0);
    assert_eq!(read_le_u64(&current, 8), 100);
    assert_eq!(read_le_u64(&current, 16), 1);
    assert_eq!(read_le_u64(&current, 24), 500_000_000);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_TIMER_GETOVERRUN_FOR_TEST,
                [timer_id, 0, 0, 0, 0, 0],
            ),
            &mut state,
            1_500_000_001,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_TIMER_DELETE_FOR_TEST,
                [timer_id, 0, 0, 0, 0, 0],
            ),
            &mut state,
            1_500_000_002,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_TIMER_GETTIME_FOR_TEST,
                [timer_id, 0xc000, 0, 0, 0, 0],
            ),
            &mut state,
            1_500_000_003,
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
fn linux_table_posix_timer_validates_args_and_sigevent() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_event_address = 0x9000;
    let none_event_address = 0x9040;
    let valid_spec_address = 0x9080;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_event_address && bytes == RISCV_LINUX_SIGEVENT_BYTES_FOR_TEST {
            Some(sigevent_bytes(0))
        } else if address == none_event_address && bytes == RISCV_LINUX_SIGEVENT_BYTES_FOR_TEST {
            Some(sigevent_bytes(RISCV_LINUX_SIGEV_NONE_FOR_TEST))
        } else if address == valid_spec_address && bytes == RISCV_LINUX_ITIMERSPEC_BYTES_FOR_TEST {
            Some(itimerspec_bytes(0, 0, 1, 0))
        } else {
            None
        }
    });
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    for (pc, arguments, errno) in [
        (0x8018, [99, 0, 0x9100, 0, 0, 0], RISCV_LINUX_EINVAL),
        (
            0x801c,
            [RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8020,
            [
                RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST,
                signal_event_address,
                0x9100,
                0,
                0,
                0,
            ],
            RISCV_LINUX_ENOTSUP,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_TIMER_CREATE_FOR_TEST, arguments),
                &mut state,
                0,
                Some(&guest_memory_reader),
                Some(&writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_TIMER_CREATE_FOR_TEST,
                [
                    RISCV_LINUX_CLOCK_MONOTONIC_FOR_TEST,
                    none_event_address,
                    0x9100,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_TIMER_SETTIME_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_TIMER_ABSTIME_FOR_TEST << 1,
                    valid_spec_address,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x802c,
                RISCV_LINUX_TIMER_SETTIME_FOR_TEST,
                [0, 0, 0xdead, 0, 0, 0],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8030,
                RISCV_LINUX_TIMER_GETTIME_FOR_TEST,
                [99, 0x9140, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8034,
                RISCV_LINUX_TIMER_GETTIME_FOR_TEST,
                [0, 0x9140, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn itimerspec_bytes(
    interval_seconds: u64,
    interval_nanoseconds: u64,
    value_seconds: u64,
    value_nanoseconds: u64,
) -> Vec<u8> {
    [
        interval_seconds.to_le_bytes(),
        interval_nanoseconds.to_le_bytes(),
        value_seconds.to_le_bytes(),
        value_nanoseconds.to_le_bytes(),
    ]
    .concat()
}

fn sigevent_bytes(notify: u32) -> Vec<u8> {
    let mut bytes = vec![0; RISCV_LINUX_SIGEVENT_BYTES_FOR_TEST];
    bytes[12..16].copy_from_slice(&notify.to_le_bytes());
    bytes
}
