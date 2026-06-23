use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_TIOCGWINSZ_FOR_TEST: u64 = 0x5413;

type RecordedGuestWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

fn recording_writer(writes: RecordedGuestWrites) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn read_le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

#[test]
fn linux_table_ioctl_tiocgwinsz_writes_deterministic_stdout_window_size() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_IOCTL,
                [1, RISCV_LINUX_TIOCGWINSZ_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let winsize = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    assert_eq!(read_le_u16(&winsize, 0), 24);
    assert_eq!(read_le_u16(&winsize, 2), 80);
    assert_eq!(read_le_u16(&winsize, 4), 0);
    assert_eq!(read_le_u16(&winsize, 6), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioctl_tiocgwinsz_follows_duplicated_stdio_description() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_DUP, [1, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_IOCTL,
                [3, RISCV_LINUX_TIOCGWINSZ_FOR_TEST, 0x9100, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let winsize = collect_guest_writes(&writes.lock().unwrap(), 0x9100, 8);
    assert_eq!(read_le_u16(&winsize, 0), 24);
    assert_eq!(read_le_u16(&winsize, 2), 80);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioctl_tiocgwinsz_rejects_pipe_fds_as_not_terminal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let pipe_fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let read_fd = read_le_u32(&pipe_fds, 0) as u64;

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_IOCTL,
                [read_fd, RISCV_LINUX_TIOCGWINSZ_FOR_TEST, 0x9100, 0, 0, 0,],
            ),
            &mut state,
            8,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTTY)
        })
    );
    assert_eq!(writes.lock().unwrap().len(), 1);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioctl_tiocgwinsz_rejects_bad_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_IOCTL,
                [99, RISCV_LINUX_TIOCGWINSZ_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioctl_tiocgwinsz_faults_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writer = RiscvGuestMemoryWriter::new(|_, _| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_IOCTL,
                [1, RISCV_LINUX_TIOCGWINSZ_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
