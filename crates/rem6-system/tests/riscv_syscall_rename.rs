#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_RENAMEAT2: u64 = 276;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_EEXIST: u64 = 17;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_RENAME_NOREPLACE: u64 = 1;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn memory_with_rename_paths() -> Arc<Mutex<rem6_memory::PartitionedMemoryStore>> {
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[(0x9000, b"old.txt\0"), (0x9100, b"new.txt\0")],
    )
}

fn handle_renameat2(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    flags: u64,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_RENAMEAT2,
            [
                RISCV_LINUX_AT_FDCWD,
                0x9000,
                RISCV_LINUX_AT_FDCWD,
                0x9100,
                flags,
                0,
            ],
        ),
        state,
        0,
        Some(reader),
        None,
    )
}

#[test]
fn linux_table_renameat2_noreplace_moves_when_destination_is_missing() {
    let store = memory_with_rename_paths();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"old.txt", b"old");

    assert_eq!(
        handle_renameat2(&mut state, &reader, RISCV_LINUX_RENAME_NOREPLACE),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.guest_file_contents(b"old.txt"), None);
    assert_eq!(state.guest_file_contents(b"new.txt"), Some(&b"old"[..]));
}

#[test]
fn linux_table_renameat2_noreplace_preserves_existing_destination() {
    let store = memory_with_rename_paths();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"old.txt", b"old");
    state.register_guest_file(b"new.txt", b"new");

    assert_eq!(
        handle_renameat2(&mut state, &reader, RISCV_LINUX_RENAME_NOREPLACE),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST)
        })
    );
    assert_eq!(state.guest_file_contents(b"old.txt"), Some(&b"old"[..]));
    assert_eq!(state.guest_file_contents(b"new.txt"), Some(&b"new"[..]));
}

#[test]
fn linux_table_renameat2_rejects_unsupported_flags_without_mutation() {
    let store = memory_with_rename_paths();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"old.txt", b"old");

    assert_eq!(
        handle_renameat2(&mut state, &reader, 2),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(state.guest_file_contents(b"old.txt"), Some(&b"old"[..]));
    assert_eq!(state.guest_file_contents(b"new.txt"), None);
}
