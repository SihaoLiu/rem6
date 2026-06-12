use super::*;

#[test]
fn linux_table_stat_writes_registered_guest_file_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let path = b"guest.txt\0".to_vec();
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_STAT, [0x9000, 0x9100, 0, 0, 0, 0]),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 18);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
}
