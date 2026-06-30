use super::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

const RISCV_LINUX_PROCESS_VM_READV_FOR_TEST: u64 = 270;
const RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST: u64 = 271;

#[test]
fn linux_table_process_vm_iovecs_copy_same_process_guest_memory() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let (memory, reader, writer) = guest_memory();

    write_iovec(&memory, 0x9000, 0xb000, 3);
    write_iovec(&memory, 0x9010, 0xb100, 3);
    write_iovec(&memory, 0x9100, 0xa000, 2);
    write_iovec(&memory, 0x9110, 0xa100, 4);
    write_bytes(&memory, 0xa000, b"ab");
    write_bytes(&memory, 0xa100, b"cdef");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [41, 0x9000, 2, 0x9100, 2, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 6 })
    );
    assert_eq!(read_bytes(&memory, 0xb000, 3), b"abc");
    assert_eq!(read_bytes(&memory, 0xb100, 3), b"def");

    write_iovec(&memory, 0x9200, 0xc000, 2);
    write_iovec(&memory, 0x9210, 0xc100, 4);
    write_iovec(&memory, 0x9300, 0xd000, 4);
    write_iovec(&memory, 0x9310, 0xd100, 2);
    write_bytes(&memory, 0xc000, b"XY");
    write_bytes(&memory, 0xc100, b"Z123");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST,
                [41, 0x9200, 2, 0x9300, 2, 0],
            ),
            &mut state,
            16,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 6 })
    );
    assert_eq!(read_bytes(&memory, 0xd000, 4), b"XYZ1");
    assert_eq!(read_bytes(&memory, 0xd100, 2), b"23");
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_vm_iovecs_reject_invalid_requests_without_touching_memory() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let reader = RiscvGuestMemoryReader::new(|_, _| panic!("invalid process_vm must not read"));
    let writer = RiscvGuestMemoryWriter::new(|_, _| panic!("invalid process_vm must not write"));

    for (pc, number, args, errno) in [
        (
            0x8000,
            RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
            [41, 0x9000, 2, 0x9100, 2, 1],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
            [41, 0x9000, 1025, 0x9100, 2, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x800c,
            RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST,
            [41, 0x9000, 2, 0x9100, 1025, 0],
            RISCV_LINUX_EINVAL,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, args),
                &mut state,
                15,
                Some(&reader),
                Some(&writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }

    let (memory, reader, writer) = guest_memory();
    write_iovec(&memory, 0x9000, 0xa000, 1);
    write_iovec(&memory, 0x9100, 0xb000, 1);
    write_bytes(&memory, 0xa000, b"Q");
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST,
                [99, 0x9000, 1, 0x9100, 1, 0],
            ),
            &mut state,
            16,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_vm_iovecs_report_guest_memory_faults() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let reader = RiscvGuestMemoryReader::new(|_, _| None);
    let writer = RiscvGuestMemoryWriter::new(|_, _| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [41, 0x9000, 1, 0x9100, 1, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_process_vm_iovecs_return_short_count_after_later_source_fault() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let (memory, reader, writer) = guest_memory();
    write_iovec(&memory, 0x9000, 0xb000, 3);
    write_iovec(&memory, 0x9010, 0xb100, 3);
    write_iovec(&memory, 0x9100, 0xa000, 3);
    write_iovec(&memory, 0x9110, 0xa100, 3);
    write_bytes(&memory, 0xa000, b"abc");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [41, 0x9000, 2, 0x9100, 2, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(read_bytes(&memory, 0xb000, 3), b"abc");
}

#[test]
fn linux_table_process_vm_iovecs_return_short_count_after_later_destination_fault() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let memory = Arc::new(Mutex::new(BTreeMap::new()));
    write_iovec(&memory, 0x9000, 0xc000, 3);
    write_iovec(&memory, 0x9010, 0xc100, 3);
    write_iovec(&memory, 0x9100, 0xd000, 3);
    write_iovec(&memory, 0x9110, 0xd100, 3);
    write_bytes(&memory, 0xc000, b"abc");
    write_bytes(&memory, 0xc100, b"def");
    let reader_memory = Arc::clone(&memory);
    let writer_memory = Arc::clone(&memory);
    let reader = guest_memory_reader(reader_memory);
    let writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if !address_range_is_within(address, bytes.len(), 0xd000, 3) {
            return false;
        }
        let mut memory = writer_memory.lock().unwrap();
        for (offset, byte) in bytes.iter().copied().enumerate() {
            memory.insert(address + offset as u64, byte);
        }
        true
    })
    .with_write_probe(|address, bytes| address_range_is_within(address, bytes, 0xd000, 3));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST,
                [41, 0x9000, 2, 0x9100, 2, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(read_bytes(&memory, 0xd000, 3), b"abc");
}

#[test]
fn linux_table_process_vm_iovecs_return_short_count_after_later_local_readv_fault() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let memory = Arc::new(Mutex::new(BTreeMap::new()));
    write_iovec(&memory, 0x9000, 0xb000, 3);
    write_iovec(&memory, 0x9010, 0xb100, 3);
    write_iovec(&memory, 0x9100, 0xa000, 3);
    write_iovec(&memory, 0x9110, 0xa100, 3);
    write_bytes(&memory, 0xa000, b"abc");
    write_bytes(&memory, 0xa100, b"def");
    let reader = guest_memory_reader(Arc::clone(&memory));
    let writer_memory = Arc::clone(&memory);
    let writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if !address_range_is_within(address, bytes.len(), 0xb000, 3) {
            return false;
        }
        let mut memory = writer_memory.lock().unwrap();
        for (offset, byte) in bytes.iter().copied().enumerate() {
            memory.insert(address + offset as u64, byte);
        }
        true
    })
    .with_write_probe(|address, bytes| address_range_is_within(address, bytes, 0xb000, 3));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [41, 0x9000, 2, 0x9100, 2, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(read_bytes(&memory, 0xb000, 3), b"abc");
}

#[test]
fn linux_table_process_vm_iovecs_return_short_count_after_later_local_writev_fault() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let (memory, reader, writer) = guest_memory();
    write_iovec(&memory, 0x9000, 0xc000, 3);
    write_iovec(&memory, 0x9010, 0xc100, 3);
    write_iovec(&memory, 0x9100, 0xd000, 3);
    write_iovec(&memory, 0x9110, 0xd100, 3);
    write_bytes(&memory, 0xc000, b"abc");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_WRITEV_FOR_TEST,
                [41, 0x9000, 2, 0x9100, 2, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(read_bytes(&memory, 0xd000, 3), b"abc");
}

#[test]
fn linux_table_process_vm_iovecs_validate_zero_count_and_iovecs_before_pid() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let no_reader =
        RiscvGuestMemoryReader::new(|_, _| panic!("zero-count process_vm must not read"));
    let no_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("zero-count process_vm must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [99, 0x9000, 0, 0x9100, 0, 0],
            ),
            &mut state,
            15,
            Some(&no_reader),
            Some(&no_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(|_, _| None);
    let writer = RiscvGuestMemoryWriter::new(|_, _| true);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [99, 0x9000, 1, 0x9100, 1, 0],
            ),
            &mut state,
            16,
            Some(&faulting_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [41, 0x9000, 1, 0x9100, 0, 0],
            ),
            &mut state,
            17,
            Some(&faulting_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    let (memory, reader, writer) = guest_memory();
    write_iovec(&memory, 0x9000, 0xa000, 0);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PROCESS_VM_READV_FOR_TEST,
                [99, 0x9000, 1, 0xdead, 1, 0],
            ),
            &mut state,
            18,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

fn guest_memory() -> (
    Arc<Mutex<BTreeMap<u64, u8>>>,
    RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter,
) {
    let memory = Arc::new(Mutex::new(BTreeMap::new()));
    let writer_memory = Arc::clone(&memory);
    let reader = guest_memory_reader(Arc::clone(&memory));
    let writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        let mut memory = writer_memory.lock().unwrap();
        for (offset, byte) in bytes.iter().copied().enumerate() {
            let Some(address) = u64::try_from(offset)
                .ok()
                .and_then(|offset| address.checked_add(offset))
            else {
                return false;
            };
            memory.insert(address, byte);
        }
        true
    });
    (memory, reader, writer)
}

fn guest_memory_reader(memory: Arc<Mutex<BTreeMap<u64, u8>>>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        let memory = memory.lock().unwrap();
        let mut data = Vec::with_capacity(bytes);
        for offset in 0..u64::try_from(bytes).ok()? {
            data.push(*memory.get(&address.checked_add(offset)?)?);
        }
        Some(data)
    })
}

fn address_range_is_within(address: u64, bytes: usize, base: u64, len: usize) -> bool {
    let Some(end) = u64::try_from(bytes)
        .ok()
        .and_then(|bytes| address.checked_add(bytes))
    else {
        return false;
    };
    let Some(limit) = u64::try_from(len)
        .ok()
        .and_then(|len| base.checked_add(len))
    else {
        return false;
    };
    address >= base && end <= limit
}

fn write_iovec(memory: &Arc<Mutex<BTreeMap<u64, u8>>>, address: u64, base: u64, len: u64) {
    let mut bytes = [0; 16];
    bytes[0..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..16].copy_from_slice(&len.to_le_bytes());
    write_bytes(memory, address, &bytes);
}

fn write_bytes(memory: &Arc<Mutex<BTreeMap<u64, u8>>>, address: u64, bytes: &[u8]) {
    let mut memory = memory.lock().unwrap();
    for (offset, byte) in bytes.iter().copied().enumerate() {
        memory.insert(address + offset as u64, byte);
    }
}

fn read_bytes(memory: &Arc<Mutex<BTreeMap<u64, u8>>>, address: u64, len: usize) -> Vec<u8> {
    let memory = memory.lock().unwrap();
    (0..len)
        .map(|offset| memory[&(address + offset as u64)])
        .collect()
}
