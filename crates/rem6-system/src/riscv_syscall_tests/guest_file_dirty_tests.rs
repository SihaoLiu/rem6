use super::*;

#[test]
fn registered_guest_file_is_clean_until_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed");
    assert!(!state.guest_file_contents_dirty(b"guest.txt"));

    let path = b"guest.txt\0".to_vec();
    let bytes = b"seed".to_vec();
    let guest_memory = RiscvGuestMemoryReader::new(move |address, count| {
        if count == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 && address < 0xa000 + bytes.len() as u64 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(count)?;
            return bytes.get(start..end).map(Vec::from);
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_WRONLY, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert!(!state.guest_file_contents_dirty(b"guest.txt"));

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [3, 0xa000, 4, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(state.guest_file_contents(b"guest.txt"), Some(&b"seed"[..]));
    assert!(state.guest_file_contents_dirty(b"guest.txt"));
}
