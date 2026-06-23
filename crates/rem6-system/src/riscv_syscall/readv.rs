use super::{
    eventfd::{eventfd_read_bytes, RiscvGuestEventFdRead},
    guest_fd_argument,
    iovec::{read_iovecs, write_iovecs, RISCV_LINUX_IOV_MAX},
    linux_error,
    positioned::riscv_linux_split_offset,
    socket::RiscvGuestSocketRead,
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_ENOTSUP, RISCV_LINUX_ESPIPE, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_READV: u64 = 65;
pub(super) const RISCV_LINUX_PREADV: u64 = 69;
pub(super) const RISCV_LINUX_PREADV2: u64 = 286;

pub(super) fn syscall_readv(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    let iov_count = request.argument(2);
    if iov_count > RISCV_LINUX_IOV_MAX {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if iov_count == 0 {
        return Some(0);
    }

    let (iovecs, total) = match read_iovecs(guest_memory_reader, request.argument(1), iov_count) {
        Ok(iovecs) => iovecs,
        Err(errno) => return Some(linux_error(errno)),
    };
    match state.guest_eventfd_read(fd, total) {
        Ok(Some(read)) => {
            let value = match read {
                RiscvGuestEventFdRead::Value(value) => value,
                RiscvGuestEventFdRead::Blocked => return None,
                RiscvGuestEventFdRead::WouldBlock => {
                    return Some(linux_error(RISCV_LINUX_EAGAIN));
                }
                RiscvGuestEventFdRead::InvalidSize => {
                    return Some(linux_error(RISCV_LINUX_EINVAL));
                }
            };
            let bytes = eventfd_read_bytes(value);
            if !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_eventfd_read(fd).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    if total == 0 {
        return Some(0);
    }
    let Ok(total_bytes) = usize::try_from(total) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };

    match state.guest_socket_read(fd, total_bytes) {
        Ok(RiscvGuestSocketRead::Bytes(bytes)) => {
            if bytes.is_empty() {
                return Some(0);
            }
            if !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_socket_read(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(RiscvGuestSocketRead::WouldBlock) => return Some(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestSocketRead::Blocked) => return None,
        Ok(RiscvGuestSocketRead::NotConnected) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        Ok(RiscvGuestSocketRead::NotSocket) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }

    match state.guest_pipe_prefix(fd, total_bytes) {
        Ok(Some(bytes)) => {
            if bytes.is_empty() {
                return Some(0);
            }
            if !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_pipe_prefix(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }

    let read_from_stdin = state.stdin_readable(fd);
    let bytes = if read_from_stdin {
        state.stdin_prefix(total_bytes)
    } else {
        match state.guest_file_prefix(fd, total_bytes) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
        }
    };
    if bytes.is_empty() {
        return Some(0);
    }

    let read_count = bytes.len() as u64;
    let Ok(offset) = state.guest_fds.file_offset(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if offset.get().checked_add(read_count).is_none() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    if !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    if state.guest_fds.advance_file_offset(fd, read_count).is_err() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    if read_from_stdin {
        state.consume_stdin_prefix(bytes.len());
    }
    Some(read_count)
}

pub(super) fn syscall_preadv(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let offset = riscv_linux_split_offset(request.argument(3), request.argument(4));
    if (offset as i64) < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    match state.guest_file_fd_is_seekable(fd) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }

    let iov_count = request.argument(2);
    if iov_count > RISCV_LINUX_IOV_MAX {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if iov_count == 0 {
        return 0;
    }
    let (iovecs, total) = match read_iovecs(guest_memory_reader, request.argument(1), iov_count) {
        Ok(iovecs) => iovecs,
        Err(errno) => return linux_error(errno),
    };
    if total == 0 {
        return 0;
    }
    let Ok(total_bytes) = usize::try_from(total) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let bytes = match state.guest_file_slice_at(fd, offset, total_bytes) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if bytes.is_empty() {
        return 0;
    }
    if !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    bytes.len() as u64
}

pub(super) fn syscall_preadv2(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    if request.argument(5) != 0 {
        return linux_error(RISCV_LINUX_ENOTSUP);
    }
    syscall_preadv(request, state, guest_memory_reader, guest_memory_writer)
}
