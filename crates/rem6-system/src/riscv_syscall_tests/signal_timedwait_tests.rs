use super::*;

const SIGKILL: u64 = 9;
const SIGUSR1: u64 = 10;
const SIGUSR2: u64 = 12;
const SIGSTOP: u64 = 19;
const SIGKILL_MASK: u64 = 1 << (SIGKILL - 1);
const SIGSTOP_MASK: u64 = 1 << (SIGSTOP - 1);
const EAGAIN: u64 = 11;
const RISCV_LINUX_SIGINFO_T_BYTES: usize = 128;

#[test]
fn linux_table_rt_sigtimedwait_returns_eagain_without_pending_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let timeout_address = 0x3fe0;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(EAGAIN)
        })
    );
}

#[test]
fn linux_table_rt_sigtimedwait_consumes_pending_blocked_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let siginfo_address = 0x3fd0;
    let timeout_address = 0x3fe0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(signal_mask.to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured_writes = writes.clone();
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        captured_writes
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    state.insert_pending_signal(SIGUSR1);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [
                    signal_set_address,
                    siginfo_address,
                    timeout_address,
                    8,
                    0,
                    0
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: SIGUSR1 })
    );
    assert_eq!(state.pending_signal_mask(), 0);
    assert!(state.unknown_syscalls().is_empty());

    let siginfo = written_signal_bytes_at(&writes.lock().unwrap(), siginfo_address);
    assert_eq!(siginfo.len(), RISCV_LINUX_SIGINFO_T_BYTES);
    assert_eq!(read_le_u32(&siginfo, 0), SIGUSR1 as u32);
}

#[test]
fn linux_table_rt_sigtimedwait_preserves_pending_signal_on_siginfo_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let siginfo_address = 0x3fd0;
    let timeout_address = 0x3fe0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(signal_mask.to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        assert_eq!(address, siginfo_address);
        assert_eq!(bytes.len(), RISCV_LINUX_SIGINFO_T_BYTES);
        false
    });

    state.insert_pending_signal(SIGUSR1);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [
                    signal_set_address,
                    siginfo_address,
                    timeout_address,
                    8,
                    0,
                    0
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.pending_signal_mask(), signal_mask);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigtimedwait_validates_timeout_before_pending_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let timeout_address = 0x3fe0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);

    state.insert_pending_signal(SIGUSR1);
    let faulting_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(signal_mask.to_le_bytes().to_vec())
        } else {
            None
        }
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&faulting_timeout_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.pending_signal_mask(), signal_mask);

    let invalid_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(signal_mask.to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(-1, 0))
        } else {
            None
        }
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&invalid_timeout_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(state.pending_signal_mask(), signal_mask);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigtimedwait_ignores_unblockable_signals() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let timeout_address = 0x3fe0;
    let unblockable_mask = SIGKILL_MASK | SIGSTOP_MASK;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(unblockable_mask.to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });

    state.insert_pending_signal(SIGKILL);
    state.insert_pending_signal(SIGSTOP);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(EAGAIN)
        })
    );
    assert_eq!(state.pending_signal_mask(), unblockable_mask);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigtimedwait_without_timeout_wait_is_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR2 - 1)).to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
}

#[test]
fn linux_table_rt_sigtimedwait_with_positive_timeout_wait_is_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4010;
    let timeout_address = 0x4018;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR2 - 1)).to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 1))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
}

#[test]
fn linux_table_rt_sigtimedwait_reports_sigset_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4020;

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 4, 0, 0],
            ),
            &mut state,
            7,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, signal_set_address);
        assert_eq!(bytes, 8);
        None
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_rt_sigtimedwait_reports_timeout_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4040;
    let timeout_address = 0x4060;
    let faulting_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&faulting_timeout_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    for (pc, seconds, nanoseconds) in [(0x8004, -1, 0), (0x8008, 0, -1), (0x800c, 0, 1_000_000_000)]
    {
        let invalid_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
            if address == signal_set_address && bytes == 8 {
                Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
            } else if address == timeout_address && bytes == 16 {
                Some(timespec64_bytes(seconds, nanoseconds))
            } else {
                None
            }
        });

        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_RT_SIGTIMEDWAIT,
                    [signal_set_address, 0, timeout_address, 8, 0, 0],
                ),
                &mut state,
                8,
                Some(&invalid_timeout_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
}

fn timespec64_bytes(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&seconds.to_le_bytes());
    bytes.extend_from_slice(&nanoseconds.to_le_bytes());
    bytes
}

fn written_signal_bytes_at(writes: &[(u64, Vec<u8>)], address: u64) -> Vec<u8> {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal info write must exist");
    bytes.clone()
}
