use super::*;

const RISCV_LINUX_SYSINFO_FOR_TEST: u64 = 179;

#[test]
fn linux_table_sysinfo_writes_guest_sysinfo() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
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
                RISCV_LINUX_SYSINFO_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_000_123,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 112);
    assert!(writes.iter().all(|(address, bytes)| {
        *address >= 0x9000 && address.saturating_add(bytes.len() as u64) <= 0x9000 + 112
    }));
    let sysinfo = collect_guest_writes(&writes, 0x9000, 112);
    assert_eq!(read_le_u64(&sysinfo, 0), 2);
    assert_eq!(read_le_u64(&sysinfo, 8), 0);
    assert_eq!(read_le_u64(&sysinfo, 16), 0);
    assert_eq!(read_le_u64(&sysinfo, 24), 0);
    assert_eq!(read_le_u64(&sysinfo, 32), 64 * 1024 * 1024);
    assert_eq!(read_le_u64(&sysinfo, 40), 64 * 1024 * 1024);
    assert_eq!(read_le_u64(&sysinfo, 48), 0);
    assert_eq!(read_le_u64(&sysinfo, 56), 0);
    assert_eq!(read_le_u64(&sysinfo, 64), 0);
    assert_eq!(read_le_u64(&sysinfo, 72), 0);
    assert_eq!(u16::from_le_bytes(sysinfo[80..82].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(sysinfo[82..84].try_into().unwrap()), 0);
    assert_eq!(read_le_u32(&sysinfo, 84), 0);
    assert_eq!(read_le_u64(&sysinfo, 88), 0);
    assert_eq!(read_le_u64(&sysinfo, 96), 0);
    assert_eq!(read_le_u32(&sysinfo, 104), 1);
    assert_eq!(read_le_u32(&sysinfo, 108), 0);
}

#[test]
fn linux_table_sysinfo_uses_default_se_memory_capacity() {
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
                RISCV_LINUX_SYSINFO_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let sysinfo = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 112);
    assert_eq!(read_le_u64(&sysinfo, 32), 256 * 1024 * 1024);
    assert_eq!(read_le_u64(&sysinfo, 40), 256 * 1024 * 1024);
}

#[test]
fn linux_table_sysinfo_returns_efault_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if address == 0x9008 {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SYSINFO_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_000_123,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert_eq!(writes.lock().unwrap().len(), 8);
}

#[test]
fn linux_table_sysinfo_is_unhandled_without_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SYSINFO_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_000_123,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}
