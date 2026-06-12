use super::*;

const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;

#[test]
fn linux_table_fcntl_setfl_enables_append_guest_file_writes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let path = b"guest.txt\0".to_vec();
    let first = b"front".to_vec();
    let second = b"tail\n".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 && address < 0xa000 + first.len() as u64 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return first.get(start..end).map(Vec::from);
        }
        if address >= 0xa100 && address < 0xa100 + second.len() as u64 {
            let start = usize::try_from(address - 0xa100).ok()?;
            let end = start.checked_add(bytes)?;
            return second.get(start..end).map(Vec::from);
        }
        None
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [3, 0xa000, 5, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_SETFL, RISCV_LINUX_O_APPEND, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_WRITE, [3, 0xa100, 5, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_READ, [3, 0xb000, 32, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"fronttail\n".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"guest.txt").unwrap().size(), 10);
    assert!(state.unknown_syscalls().is_empty());
}
