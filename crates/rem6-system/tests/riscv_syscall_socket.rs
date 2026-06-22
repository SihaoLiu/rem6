#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_SOCKET: u64 = 198;
const RISCV_LINUX_SOCKETPAIR: u64 = 199;
const RISCV_LINUX_GETSOCKNAME: u64 = 204;
const RISCV_LINUX_GETPEERNAME: u64 = 205;
const RISCV_LINUX_SENDTO: u64 = 206;
const RISCV_LINUX_RECVFROM: u64 = 207;
const RISCV_LINUX_SETSOCKOPT: u64 = 208;
const RISCV_LINUX_GETSOCKOPT: u64 = 209;
const RISCV_LINUX_SHUTDOWN: u64 = 210;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_READV: u64 = 65;
const RISCV_LINUX_WRITEV: u64 = 66;
const RISCV_LINUX_PPOLL: u64 = 73;
const RISCV_LINUX_F_GETPIPE_SZ: u64 = 1032;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_EPIPE: u64 = 32;
const RISCV_LINUX_ENOTSOCK: u64 = 88;
const RISCV_LINUX_ENOPROTOOPT: u64 = 92;
const RISCV_LINUX_EPROTONOSUPPORT: u64 = 93;
const RISCV_LINUX_ENOTSUP: u64 = 95;
const RISCV_LINUX_EAFNOSUPPORT: u64 = 97;
const RISCV_LINUX_ENOTCONN: u64 = 107;
const RISCV_LINUX_AF_UNIX: u64 = 1;
const RISCV_LINUX_SOCK_STREAM: u64 = 1;
const RISCV_LINUX_SOL_SOCKET: u64 = 1;
const RISCV_LINUX_SO_REUSEADDR: u64 = 2;
const RISCV_LINUX_SO_TYPE: u64 = 3;
const RISCV_LINUX_SO_ERROR: u64 = 4;
const RISCV_LINUX_MSG_DONTWAIT: u64 = 0x40;
const RISCV_LINUX_MSG_NOSIGNAL: u64 = 0x4000;
const RISCV_LINUX_SHUT_RDWR: u64 = 2;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_POLLIN: i16 = 0x0001;
const RISCV_LINUX_POLLOUT: i16 = 0x0004;
const RISCV_LINUX_POLLHUP: i16 = 0x0010;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn socket_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x9000, b"left-to-right"),
            (0x9020, b"right-to-left"),
            (0x9040, b"vec-"),
            (0x9044, b"socket"),
            (0x9060, b"sendto->recv"),
            (0x9100, &[0; 16]),
            (0x9120, &[0; 16]),
            (0x9140, &[0; 16]),
            (0x9160, &[0; 16]),
            (0x9180, &[0; 16]),
            (0x91a0, &[0; 16]),
            (0x91c0, &[0; 16]),
            (0x9320, &[0; 16]),
            (0x9340, &[0; 8]),
            (0x9360, &[0; 16]),
            (0x9380, &[0; 8]),
            (0x93a0, &[0; 8]),
            (0x93c0, &[0; 8]),
            (0x9200, &iovec(0x9040, 4)),
            (0x9210, &iovec(0x9044, 6)),
            (0x9220, &iovec(0x9140, 3)),
            (0x9230, &iovec(0x9160, 7)),
            (0x9300, &[0; 8]),
        ],
    )
}

fn fds_from_memory(store: &Arc<Mutex<PartitionedMemoryStore>>) -> (u64, u64) {
    let fds = guest_memory_reader(Arc::clone(store))(0x8800, 8).unwrap();
    let left_fd = i32::from_le_bytes(fds[..4].try_into().unwrap());
    let right_fd = i32::from_le_bytes(fds[4..].try_into().unwrap());
    (left_fd as u64, right_fd as u64)
}

fn pollfd_bytes(fd: u64, events: i16) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&(fd as i32).to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes
}

fn pollfd_revents(store: &Arc<Mutex<PartitionedMemoryStore>>) -> i16 {
    let pollfd = guest_memory_reader(Arc::clone(store))(0x9300, 8).unwrap();
    i16::from_le_bytes(pollfd[6..8].try_into().unwrap())
}

fn guest_i32(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> i32 {
    i32::from_le_bytes(
        guest_memory_reader(Arc::clone(store))(address, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn guest_u32(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> u32 {
    u32::from_le_bytes(
        guest_memory_reader(Arc::clone(store))(address, 4)
            .unwrap()
            .try_into()
            .unwrap(),
    )
}

fn iovec(address: u64, len: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&address.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    bytes
}

fn handle_with_memory(
    state: &mut RiscvSyscallState,
    number: u64,
    arguments: [u64; 6],
    reader: Option<&RiscvGuestMemoryReader>,
    writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    RiscvSyscallTable::new()
        .handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, number, arguments),
            state,
            7,
            reader,
            writer,
        )
        .expect("syscall must be handled")
}

fn return_value(outcome: RiscvSyscallOutcome) -> u64 {
    match outcome {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected syscall outcome: {outcome:?}"),
    }
}

#[test]
fn linux_table_socket_creates_unconnected_unix_stream_fd() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd_value = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_SOCKET,
        [
            RISCV_LINUX_AF_UNIX,
            RISCV_LINUX_SOCK_STREAM | RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK,
            0,
            0,
            0,
            0,
        ],
        None,
        None,
    ));
    assert_eq!(fd_value, 3);
    let fd = GuestFd::new(fd_value as i32).unwrap();
    assert!(state.guest_fds().close_on_exec(fd).unwrap());
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap().bits() & RISCV_LINUX_O_NONBLOCK as u32,
        RISCV_LINUX_O_NONBLOCK as u32
    );

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93c0,
        &4_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [
                fd_value,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_TYPE,
                0x93a0,
                0x93c0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(guest_i32(&store, 0x93a0), RISCV_LINUX_SOCK_STREAM as i32);
    assert_eq!(guest_u32(&store, 0x93c0), 4);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKNAME,
            [fd_value, 0x9100, 0x9120, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 2).unwrap(),
        (RISCV_LINUX_AF_UNIX as u16).to_le_bytes()
    );
    assert_eq!(guest_u32(&store, 0x9120), 2);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETPEERNAME,
            [fd_value, 0x9100, 0x9120, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_ENOTCONN)
    );

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9300,
        &pollfd_bytes(fd_value, 0)
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9300, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(pollfd_revents(&store), RISCV_LINUX_POLLHUP);

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9300,
        &pollfd_bytes(fd_value, RISCV_LINUX_POLLOUT)
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9300, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        pollfd_revents(&store),
        RISCV_LINUX_POLLOUT | RISCV_LINUX_POLLHUP
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd_value, 0x9000, 0, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTCONN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd_value, 0x9000, 1, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTCONN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SENDTO,
            [
                fd_value,
                0x9000,
                1,
                RISCV_LINUX_MSG_DONTWAIT | RISCV_LINUX_MSG_NOSIGNAL,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTCONN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd_value, 0x9140, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RECVFROM,
            [fd_value, 0x9140, 1, RISCV_LINUX_MSG_DONTWAIT, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_CLOSE,
            [fd_value, 0, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert!(state.guest_fds().entry(fd).is_none());
}

#[test]
fn linux_table_socket_rejects_unsupported_network_domains_without_host_network() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKET,
            [2, RISCV_LINUX_SOCK_STREAM, 0, 0, 0, 0],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_EAFNOSUPPORT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKET,
            [RISCV_LINUX_AF_UNIX, 0, 0, 0, 0, 0],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_EPROTONOSUPPORT)
    );
    assert_eq!(state.guest_fds().len(), 3);
}

#[test]
fn linux_table_socket_options_roundtrip_guest_socket_state() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKETPAIR,
            [
                RISCV_LINUX_AF_UNIX,
                RISCV_LINUX_SOCK_STREAM,
                0,
                0x8800,
                0,
                0,
            ],
            None,
            Some(&writer),
        )),
        0
    );
    let (left_fd, right_fd) = fds_from_memory(&store);

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93c0,
        &4_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [
                left_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_TYPE,
                0x93a0,
                0x93c0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(guest_i32(&store, 0x93a0), RISCV_LINUX_SOCK_STREAM as i32);
    assert_eq!(guest_u32(&store, 0x93c0), 4);

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93a0,
        &(-1_i32).to_le_bytes()
    ));
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93c0,
        &4_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [
                right_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_ERROR,
                0x93a0,
                0x93c0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(guest_i32(&store, 0x93a0), 0);
    assert_eq!(guest_u32(&store, 0x93c0), 4);

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93a0,
        &1_i32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SETSOCKOPT,
            [
                left_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_REUSEADDR,
                0x93a0,
                4,
                0,
            ],
            Some(&reader),
            None,
        )),
        0
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93a0,
        &0_i32.to_le_bytes()
    ));
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93c0,
        &4_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [
                left_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_REUSEADDR,
                0x93a0,
                0x93c0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(guest_i32(&store, 0x93a0), 1);
    assert_eq!(guest_u32(&store, 0x93c0), 4);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SETSOCKOPT,
            [
                left_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_REUSEADDR,
                0x93a0,
                3,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [right_fd, RISCV_LINUX_SOL_SOCKET, 9999, 0x93a0, 0x93c0, 0],
            Some(&reader),
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_ENOPROTOOPT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [right_fd, RISCV_LINUX_SOL_SOCKET, 9999, 0x93a0, 0x1, 0],
            Some(&reader),
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x93c0,
        &0xffff_ffff_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [
                right_fd,
                RISCV_LINUX_SOL_SOCKET,
                RISCV_LINUX_SO_TYPE,
                0x93a0,
                0x93c0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SETSOCKOPT,
            [right_fd, 9999, RISCV_LINUX_SO_REUSEADDR, 0x93a0, 4, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTSUP)
    );
}

#[test]
fn linux_table_socketpair_roundtrips_bidirectional_bytes_and_poll_without_pipe_identity() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKETPAIR,
            [
                RISCV_LINUX_AF_UNIX,
                RISCV_LINUX_SOCK_STREAM,
                0,
                0x8800,
                0,
                0,
            ],
            None,
            Some(&writer),
        )),
        0
    );
    let (left_fd, right_fd) = fds_from_memory(&store);
    assert_ne!(left_fd, right_fd);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_FCNTL,
            [left_fd, RISCV_LINUX_F_GETPIPE_SZ, 0, 0, 0, 0],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_EBADF)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [left_fd, 0x9000, 13, 0, 0, 0],
            Some(&reader),
            None,
        )),
        13
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9300,
        &pollfd_bytes(right_fd, RISCV_LINUX_POLLIN),
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9300, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_ne!(pollfd_revents(&store) & RISCV_LINUX_POLLIN, 0);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [right_fd, 0x9100, 13, 0, 0, 0],
            None,
            Some(&writer),
        )),
        13
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 13),
        Some(b"left-to-right".to_vec())
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [right_fd, 0x9020, 13, 0, 0, 0],
            Some(&reader),
            None,
        )),
        13
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [left_fd, 0x9120, 13, 0, 0, 0],
            None,
            Some(&writer),
        )),
        13
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9120, 13),
        Some(b"right-to-left".to_vec())
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITEV,
            [left_fd, 0x9200, 2, 0, 0, 0],
            Some(&reader),
            None,
        )),
        10
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READV,
            [right_fd, 0x9220, 2, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        10
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9140, 3),
        Some(b"vec".to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9160, 7),
        Some(b"-socket".to_vec())
    );

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9340,
        &16_u32.to_le_bytes()
    ));
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9380,
        &16_u32.to_le_bytes()
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKNAME,
            [left_fd, 0x9320, 0x9340, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETPEERNAME,
            [right_fd, 0x9360, 0x9380, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(
        u32::from_le_bytes(
            guest_memory_reader(Arc::clone(&store))(0x9340, 4)
                .unwrap()
                .try_into()
                .unwrap()
        ),
        2
    );
    assert_eq!(
        u32::from_le_bytes(
            guest_memory_reader(Arc::clone(&store))(0x9380, 4)
                .unwrap()
                .try_into()
                .unwrap()
        ),
        2
    );
    assert_eq!(
        u16::from_le_bytes(
            guest_memory_reader(Arc::clone(&store))(0x9320, 2)
                .unwrap()
                .try_into()
                .unwrap()
        ),
        RISCV_LINUX_AF_UNIX as u16
    );
    assert_eq!(
        u16::from_le_bytes(
            guest_memory_reader(Arc::clone(&store))(0x9360, 2)
                .unwrap()
                .try_into()
                .unwrap()
        ),
        RISCV_LINUX_AF_UNIX as u16
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SENDTO,
            [left_fd, 0x9060, 12, 0, 0, 0],
            Some(&reader),
            None,
        )),
        12
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RECVFROM,
            [right_fd, 0x91c0, 12, 0, 0, 0],
            None,
            Some(&writer),
        )),
        12
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x91c0, 12),
        Some(b"sendto->recv".to_vec())
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SENDTO,
            [left_fd, 0x9060, 12, RISCV_LINUX_MSG_NOSIGNAL, 0, 0],
            Some(&reader),
            None,
        )),
        12
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RECVFROM,
            [right_fd, 0x91c0, 12, RISCV_LINUX_MSG_DONTWAIT, 0, 0],
            None,
            Some(&writer),
        )),
        12
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x91c0, 12),
        Some(b"sendto->recv".to_vec())
    );
    assert!(state.guest_writes().is_empty());

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SHUTDOWN,
            [left_fd, RISCV_LINUX_SHUT_RDWR, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [left_fd, 0x9000, 1, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EPIPE)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [left_fd, 0x9100, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [right_fd, 0x9020, 1, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EPIPE)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [right_fd, 0x9120, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );

    for fd in [left_fd, right_fd] {
        assert_eq!(
            return_value(handle_with_memory(
                &mut state,
                RISCV_LINUX_CLOSE,
                [fd, 0, 0, 0, 0, 0],
                None,
                None,
            )),
            0
        );
    }
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [left_fd, 0x9120, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
}

#[test]
fn linux_table_socketpair_poll_reports_readable_eof_after_peer_close() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKETPAIR,
            [
                RISCV_LINUX_AF_UNIX,
                RISCV_LINUX_SOCK_STREAM,
                0,
                0x8800,
                0,
                0,
            ],
            None,
            Some(&writer),
        )),
        0
    );
    let (left_fd, right_fd) = fds_from_memory(&store);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_CLOSE,
            [left_fd, 0, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9300,
        &pollfd_bytes(right_fd, RISCV_LINUX_POLLIN),
    ));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9300, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_ne!(pollfd_revents(&store) & RISCV_LINUX_POLLIN, 0);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [right_fd, 0x9100, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
}

#[test]
fn linux_table_socketpair_poll_reports_writable_error_after_peer_close() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKETPAIR,
            [
                RISCV_LINUX_AF_UNIX,
                RISCV_LINUX_SOCK_STREAM,
                0,
                0x8800,
                0,
                0,
            ],
            None,
            Some(&writer),
        )),
        0
    );
    let (left_fd, right_fd) = fds_from_memory(&store);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_CLOSE,
            [left_fd, 0, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9300,
        &pollfd_bytes(right_fd, RISCV_LINUX_POLLOUT),
    ));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9300, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_ne!(pollfd_revents(&store) & RISCV_LINUX_POLLOUT, 0);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [right_fd, 0x9000, 1, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EPIPE)
    );
}

#[test]
fn linux_table_recvfrom_msg_dontwait_reports_eagain_on_empty_socket() {
    let store = socket_store();
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SOCKETPAIR,
            [
                RISCV_LINUX_AF_UNIX,
                RISCV_LINUX_SOCK_STREAM,
                0,
                0x8800,
                0,
                0,
            ],
            None,
            Some(&writer),
        )),
        0
    );
    let (_left_fd, right_fd) = fds_from_memory(&store);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RECVFROM,
            [right_fd, 0x91c0, 1, RISCV_LINUX_MSG_DONTWAIT, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
}

#[test]
fn linux_table_sendto_recvfrom_report_non_socket_fds_as_enotsock() {
    let store = socket_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SENDTO,
            [0, 0x9060, 1, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTSOCK)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RECVFROM,
            [1, 0x91c0, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_ENOTSOCK)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SETSOCKOPT,
            [1, 9999, 9999, 0x93a0, 4, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOTSOCK)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_GETSOCKOPT,
            [1, 9999, 9999, 0x93a0, 0x93c0, 0],
            Some(&reader),
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_ENOTSOCK)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SHUTDOWN,
            [1, 99, 0, 0, 0, 0],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_ENOTSOCK)
    );
}
