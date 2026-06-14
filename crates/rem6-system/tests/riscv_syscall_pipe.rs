#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_PIPE2: u64 = 59;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn pipe_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let fd_area = [0; 8];
    let read_area = [0; 9];
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &fd_area),
            (0x9000, b"pipe-data"),
            (0x9100, &read_area),
        ],
    )
}

fn fds_from_memory(store: &Arc<Mutex<PartitionedMemoryStore>>) -> (u64, u64) {
    let fds = guest_memory_reader(Arc::clone(store))(0x8800, 8).unwrap();
    let read_fd = i32::from_le_bytes(fds[..4].try_into().unwrap());
    let write_fd = i32::from_le_bytes(fds[4..].try_into().unwrap());
    (read_fd as u64, write_fd as u64)
}

#[test]
fn linux_table_pipe2_roundtrips_bytes_and_close_releases_endpoints() {
    let store = pipe_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (read_fd, write_fd) = fds_from_memory(&store);
    assert_ne!(read_fd, write_fd);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [write_fd, 0x9000, 9, 0, 0, 0]),
            &mut state,
            11,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(state.guest_writes().is_empty());

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [read_fd, 0x9100, 9, 0, 0, 0]),
            &mut state,
            12,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 9),
        Some(b"pipe-data".to_vec())
    );
    assert!(state.guest_writes().is_empty());

    for fd in [read_fd, write_fd] {
        assert_eq!(
            RiscvSyscallTable::new().handle(
                RiscvSyscallRequest::new(0x800c, RISCV_LINUX_CLOSE, [fd, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [read_fd, 0x9100, 1, 0, 0, 0]),
            &mut state,
            13,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_WRITE, [write_fd, 0x9000, 1, 0, 0, 0]),
            &mut state,
            14,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_pipe2_rejects_invalid_flags_and_faulting_fd_array() {
    let mut state = RiscvSyscallState::new(0);
    let invalid_flags = 1_u64 << 40;
    let panic_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid pipe2 flags should not write the fd array")
    });

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIPE2,
                [0x8800, invalid_flags, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&panic_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            1,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}
