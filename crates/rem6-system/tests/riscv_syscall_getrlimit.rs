#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_GETRLIMIT: u64 = 163;
const RISCV_LINUX_RLIMIT_DATA: u64 = 2;
const RISCV_LINUX_RLIMIT_STACK: u64 = 3;
const RISCV_LINUX_RLIMIT_NPROC: u64 = 6;
const RISCV_LINUX_STACK_LIMIT_BYTES: u64 = 8 * 1024 * 1024;
const RISCV_LINUX_DATA_LIMIT_BYTES: u64 = 256 * 1024 * 1024;
const RISCV_LINUX_SINGLE_PROCESS_COUNT: u64 = 1;

#[test]
fn linux_table_getrlimit_writes_stack_limit() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRLIMIT,
            [RISCV_LINUX_RLIMIT_STACK, 0x9000, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.unknown_syscalls().is_empty());
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_getrlimit_writes_data_limit() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRLIMIT,
            [RISCV_LINUX_RLIMIT_DATA, 0x9000, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.unknown_syscalls().is_empty());
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_DATA_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_DATA_LIMIT_BYTES);
}

#[test]
fn linux_table_getrlimit_writes_single_process_count_limit() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRLIMIT,
            [RISCV_LINUX_RLIMIT_NPROC, 0x9000, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.unknown_syscalls().is_empty());
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_SINGLE_PROCESS_COUNT);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_SINGLE_PROCESS_COUNT);
}

#[test]
fn linux_table_getrlimit_rejects_unsupported_resource_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("unsupported resource should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETRLIMIT, [7, 0x9000, 0, 0, 0, 0]),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(22),
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getrlimit_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_GETRLIMIT,
            [RISCV_LINUX_RLIMIT_STACK, 0x9000, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(14),
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
}
