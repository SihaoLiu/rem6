use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};

const RISCV_LINUX_MKNODAT: u64 = 33;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_S_IFREG: u64 = 0o100000;
const RISCV_LINUX_ENOENT: u64 = 2;
const RISCV_LINUX_EEXIST: u64 = 17;
const RISCV_LINUX_O_RDWR: u64 = 2;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn memory_with_path(path: &'static [u8]) -> Arc<Mutex<rem6_memory::PartitionedMemoryStore>> {
    loaded_program_store_with_data(&[(0x8000, 0)], &[(0x9000, path)])
}

fn memory_with_mknodat_path() -> Arc<Mutex<rem6_memory::PartitionedMemoryStore>> {
    memory_with_path(b"/node.reg\0")
}

fn handle_mknodat_with_mode(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    mode: u64,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_MKNODAT,
            [RISCV_LINUX_AT_FDCWD, 0x9000, mode, 0, 0, 0],
        ),
        state,
        0,
        Some(reader),
        None,
    )
}

fn handle_mknodat(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
) -> Option<RiscvSyscallOutcome> {
    handle_mknodat_with_mode(state, reader, RISCV_LINUX_S_IFREG | 0o600)
}

fn handle_open_created_node(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_OPENAT,
            [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDWR, 0, 0, 0],
        ),
        state,
        0,
        Some(reader),
        None,
    )
}

#[test]
fn linux_table_mknodat_creates_regular_guest_file_for_openat() {
    let store = memory_with_mknodat_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_mknodat(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_open_created_node(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
}

#[test]
fn linux_table_mknodat_reports_existing_regular_guest_file() {
    let store = memory_with_mknodat_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_mknodat(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_mknodat(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST)
        })
    );
}

#[test]
fn linux_table_mknodat_treats_mode_without_file_type_as_regular_file() {
    let store = memory_with_mknodat_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_mknodat_with_mode(&mut state, &reader, 0o600),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_open_created_node(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
}

#[test]
fn linux_table_mknodat_rejects_missing_path_with_trailing_slash() {
    let store = memory_with_path(b"/node.reg/\0");
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_mknodat(&mut state, &reader),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
}
