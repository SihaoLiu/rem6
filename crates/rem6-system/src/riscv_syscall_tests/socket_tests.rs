use super::*;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

type GuestMemory = Arc<Mutex<BTreeMap<u64, u8>>>;

const RISCV_LINUX_SOCKETPAIR_FOR_TEST: u64 = 199;
const RISCV_LINUX_RECVMMSG_FOR_TEST: u64 = 243;
const RISCV_LINUX_SENDMMSG_FOR_TEST: u64 = 269;
const RISCV_LINUX_AF_UNIX_FOR_TEST: u64 = 1;
const RISCV_LINUX_SOCK_STREAM_FOR_TEST: u64 = 1;
const RISCV_LINUX_MSG_DONTWAIT_FOR_TEST: u64 = 0x40;
const RISCV_LINUX_MSG_NOSIGNAL_FOR_TEST: u64 = 0x4000;

#[test]
fn linux_table_sendmmsg_sends_single_message_and_writes_message_length() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let memory = Arc::new(Mutex::new(BTreeMap::new()));
    let guest_memory_reader = guest_memory_reader(Arc::clone(&memory));
    let guest_memory_writer = guest_memory_writer(Arc::clone(&memory));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SOCKETPAIR_FOR_TEST,
                [
                    RISCV_LINUX_AF_UNIX_FOR_TEST,
                    RISCV_LINUX_SOCK_STREAM_FOR_TEST,
                    0,
                    0x9000,
                    0,
                    0,
                ],
            ),
            &mut state,
            4,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = read_bytes(&memory, 0x9000, 8);
    let left_fd = read_le_i32(&fds, 0) as u64;
    let right_fd = read_le_i32(&fds, 4) as u64;

    write_msghdr(&memory, 0x9100, 0x9200, 2);
    write_iovec(&memory, 0x9200, 0x9300, 4);
    write_iovec(&memory, 0x9210, 0x9400, 3);
    write_bytes(&memory, 0x9300, b"batc");
    write_bytes(&memory, 0x9400, b"hed");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SENDMMSG_FOR_TEST,
                [left_fd, 0x9100, 1, RISCV_LINUX_MSG_NOSIGNAL_FOR_TEST, 0, 0,],
            ),
            &mut state,
            5,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    let msg_len = read_bytes(&memory, 0x9138, 4);
    assert_eq!(read_le_u32(&msg_len, 0), 7);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [right_fd, 0x9500, 7, 0, 0, 0]),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(read_bytes(&memory, 0x9500, 7), b"batched");
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_recvmmsg_receives_single_message_and_writes_message_length() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let memory = Arc::new(Mutex::new(BTreeMap::new()));
    let guest_memory_reader = guest_memory_reader(Arc::clone(&memory));
    let guest_memory_writer = guest_memory_writer(Arc::clone(&memory));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SOCKETPAIR_FOR_TEST,
                [
                    RISCV_LINUX_AF_UNIX_FOR_TEST,
                    RISCV_LINUX_SOCK_STREAM_FOR_TEST,
                    0,
                    0xa000,
                    0,
                    0,
                ],
            ),
            &mut state,
            4,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = read_bytes(&memory, 0xa000, 8);
    let left_fd = read_le_i32(&fds, 0) as u64;
    let right_fd = read_le_i32(&fds, 4) as u64;

    write_bytes(&memory, 0xa100, b"packet");
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_WRITE, [left_fd, 0xa100, 6, 0, 0, 0]),
            &mut state,
            5,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 6 })
    );

    write_msghdr(&memory, 0xa200, 0xa300, 2);
    write_iovec(&memory, 0xa300, 0xa400, 3);
    write_iovec(&memory, 0xa310, 0xa500, 3);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_RECVMMSG_FOR_TEST,
                [right_fd, 0xa200, 1, RISCV_LINUX_MSG_DONTWAIT_FOR_TEST, 0, 0,],
            ),
            &mut state,
            6,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(read_bytes(&memory, 0xa400, 3), b"pac");
    assert_eq!(read_bytes(&memory, 0xa500, 3), b"ket");
    let msg_len = read_bytes(&memory, 0xa238, 4);
    assert_eq!(read_le_u32(&msg_len, 0), 6);
    let msg_flags = read_bytes(&memory, 0xa230, 4);
    assert_eq!(read_le_i32(&msg_flags, 0), 0);
    assert!(state.unknown_syscalls().is_empty());
}

fn guest_memory_reader(memory: GuestMemory) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        let memory = memory.lock().unwrap();
        let mut result = Vec::with_capacity(bytes);
        for offset in 0..bytes {
            let address = address.checked_add(offset as u64)?;
            result.push(*memory.get(&address)?);
        }
        Some(result)
    })
}

fn guest_memory_writer(memory: GuestMemory) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        let mut memory = memory.lock().unwrap();
        for (offset, byte) in bytes.iter().enumerate() {
            let Some(address) = address.checked_add(offset as u64) else {
                return false;
            };
            memory.insert(address, *byte);
        }
        true
    })
}

fn write_msghdr(memory: &GuestMemory, address: u64, iov: u64, iovlen: u64) {
    let mut bytes = [0_u8; 56];
    bytes[16..24].copy_from_slice(&iov.to_le_bytes());
    bytes[24..32].copy_from_slice(&iovlen.to_le_bytes());
    write_bytes(memory, address, &bytes);
}

fn write_iovec(memory: &GuestMemory, address: u64, base: u64, len: u64) {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    write_bytes(memory, address, &bytes);
}

fn write_bytes(memory: &GuestMemory, address: u64, bytes: &[u8]) {
    let mut memory = memory.lock().unwrap();
    for (offset, byte) in bytes.iter().enumerate() {
        memory.insert(address + offset as u64, *byte);
    }
}

fn read_bytes(memory: &GuestMemory, address: u64, len: usize) -> Vec<u8> {
    let memory = memory.lock().unwrap();
    (0..len)
        .map(|offset| memory[&(address + offset as u64)])
        .collect()
}

fn read_le_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
