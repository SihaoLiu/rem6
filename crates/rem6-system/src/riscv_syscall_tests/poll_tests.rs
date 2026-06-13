use super::*;

const RISCV_LINUX_PPOLL_FOR_TEST: u64 = 73;
const RISCV_LINUX_POLLIN_FOR_TEST: i16 = 0x0001;
const RISCV_LINUX_POLLOUT_FOR_TEST: i16 = 0x0004;
const RISCV_LINUX_POLLNVAL_FOR_TEST: i16 = 0x0020;

#[test]
fn linux_table_ppoll_marks_ready_stdio_and_invalid_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"x");
    let pollfds = [
        pollfd_bytes(0, RISCV_LINUX_POLLIN_FOR_TEST),
        pollfd_bytes(
            1,
            RISCV_LINUX_POLLIN_FOR_TEST | RISCV_LINUX_POLLOUT_FOR_TEST,
        ),
        pollfd_bytes(
            99,
            RISCV_LINUX_POLLIN_FOR_TEST | RISCV_LINUX_POLLOUT_FOR_TEST,
        ),
    ]
    .concat();
    let base = 0x9000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address < base {
            return None;
        }
        let offset = usize::try_from(address - base).ok()?;
        pollfds
            .get(offset..offset + bytes)
            .map(|chunk| chunk.to_vec())
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PPOLL_FOR_TEST, [base, 3, 0, 0, 0, 0],),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[
            (base + 6, RISCV_LINUX_POLLIN_FOR_TEST.to_le_bytes().to_vec()),
            (
                base + 14,
                RISCV_LINUX_POLLOUT_FOR_TEST.to_le_bytes().to_vec()
            ),
            (
                base + 22,
                RISCV_LINUX_POLLNVAL_FOR_TEST.to_le_bytes().to_vec()
            ),
        ]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ppoll_zero_nfds_returns_without_guest_memory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PPOLL_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            12,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ppoll_reports_efault_when_pollfd_read_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_address, _bytes| panic!("faulting ppoll must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PPOLL_FOR_TEST, [0x9000, 1, 0, 0, 0, 0],),
            &mut state,
            13,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ppoll_later_read_fault_does_not_partially_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"x");
    let first_pollfd = pollfd_bytes(0, RISCV_LINUX_POLLIN_FOR_TEST);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0x9000 && bytes == first_pollfd.len() {
            Some(first_pollfd.to_vec())
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PPOLL_FOR_TEST, [0x9000, 2, 0, 0, 0, 0],),
            &mut state,
            14,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

fn pollfd_bytes(fd: i32, events: i16) -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes
}
