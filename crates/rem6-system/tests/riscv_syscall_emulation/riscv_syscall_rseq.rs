use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};

const RISCV_LINUX_RSEQ: u64 = 293;
const RISCV_LINUX_RSEQ_SIZE: u64 = 32;
const RISCV_LINUX_RSEQ_FLAG_UNREGISTER: u64 = 1;
const RISCV_LINUX_RSEQ_SIGNATURE: u64 = 0x5305_3053;
const RISCV_LINUX_EBUSY: u64 = 16;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_EPERM: u64 = 1;

fn linux_error(error: u64) -> u64 {
    0u64.wrapping_sub(error)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn rseq_request(address: u64, length: u64, flags: u64, signature: u64) -> RiscvSyscallRequest {
    RiscvSyscallRequest::new(
        0x8000,
        RISCV_LINUX_RSEQ,
        [address, length, flags, signature, 0, 0],
    )
}

fn handle_rseq(
    state: &mut RiscvSyscallState,
    writer: Option<&RiscvGuestMemoryWriter>,
    address: u64,
    length: u64,
    flags: u64,
    signature: u64,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        rseq_request(address, length, flags, signature),
        state,
        0,
        None,
        writer,
    )
}

#[test]
fn linux_table_rseq_registers_current_thread_area() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = handle_rseq(
        &mut state,
        Some(&writer),
        0x9000,
        RISCV_LINUX_RSEQ_SIZE,
        0,
        RISCV_LINUX_RSEQ_SIGNATURE,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert!(state.unknown_syscalls().is_empty());
    let mut bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    bytes.extend_from_slice(&guest_memory_reader(Arc::clone(&store))(0x9010, 16).unwrap());
    assert_eq!(read_u32(&bytes, 0), 0);
    assert_eq!(read_u32(&bytes, 4), 0);
    assert_eq!(read_u32(&bytes, 8), 0);
    assert_eq!(read_u32(&bytes, 12), 0);
}

#[test]
fn linux_table_rseq_rejects_duplicate_register() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBUSY),
        })
    );
}

#[test]
fn linux_table_rseq_rejects_changed_reregistration_and_signature() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 64])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9020,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL),
        })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE + 1,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM),
        })
    );
}

#[test]
fn linux_table_rseq_unregisters_matching_registration() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            RISCV_LINUX_RSEQ_FLAG_UNREGISTER,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u32(&bytes, 0), 0);
    assert_eq!(read_u32(&bytes, 4), u32::MAX);
    assert_eq!(read_u32(&bytes, 8), 0);
    assert_eq!(read_u32(&bytes, 12), 0);
    assert_eq!(
        handle_rseq(
            &mut state,
            None,
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            RISCV_LINUX_RSEQ_FLAG_UNREGISTER,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL),
        })
    );
}

#[test]
fn linux_table_rseq_unregister_rejects_signature_mismatch() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            RISCV_LINUX_RSEQ_FLAG_UNREGISTER,
            RISCV_LINUX_RSEQ_SIGNATURE + 1,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM),
        })
    );
}

#[test]
fn linux_table_rseq_unregister_fault_keeps_registration() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&faulting_writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            RISCV_LINUX_RSEQ_FLAG_UNREGISTER,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        })
    );
    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            RISCV_LINUX_RSEQ_FLAG_UNREGISTER,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_rseq_rejects_invalid_arguments() {
    let store = loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0; 64])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    for (address, length, flags) in [
        (0x9000, RISCV_LINUX_RSEQ_SIZE - 1, 0),
        (0x9001, RISCV_LINUX_RSEQ_SIZE, 0),
        (0x9000, RISCV_LINUX_RSEQ_SIZE, 2),
    ] {
        assert_eq!(
            handle_rseq(
                &mut state,
                Some(&writer),
                address,
                length,
                flags,
                RISCV_LINUX_RSEQ_SIGNATURE,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL),
            })
        );
    }
}

#[test]
fn linux_table_rseq_reports_fault_when_registration_area_is_not_writable() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_rseq(
            &mut state,
            Some(&writer),
            0x9000,
            RISCV_LINUX_RSEQ_SIZE,
            0,
            RISCV_LINUX_RSEQ_SIGNATURE,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
