use super::{
    eventfd::{eventfd_write_bytes_written, eventfd_write_result},
    file_write::RiscvGuestFileWriteError,
    guest_fd_argument,
    iovec::{read_iovec_bytes, read_iovec_prefix, read_iovecs, RISCV_LINUX_IOV_MAX},
    linux_error,
    pipe::RiscvGuestPipeWrite,
    positioned::riscv_linux_split_offset,
    socket::{socket_write_result, RiscvGuestSocketWrite},
    RiscvGuestMemoryReader, RiscvGuestWriteRecord, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG,
    RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM, RISCV_LINUX_ESPIPE, RISCV_LINUX_O_ACCMODE,
    RISCV_LINUX_O_RDONLY,
};
use crate::Tick;

pub(super) const RISCV_LINUX_WRITEV: u64 = 66;
pub(super) const RISCV_LINUX_PWRITEV: u64 = 70;

pub(super) fn syscall_writev(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    let iov_count = request.argument(2);
    if iov_count > RISCV_LINUX_IOV_MAX {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if iov_count == 0 {
        return Some(0);
    }

    let iov_base = request.argument(1);
    let (iovecs, total) = match read_iovecs(guest_memory, iov_base, iov_count) {
        Ok(iovecs) => iovecs,
        Err(errno) => return Some(linux_error(errno)),
    };

    match state.guest_eventfd_ready(fd) {
        Ok(Some(_ready)) => {
            if total < eventfd_write_bytes_written() {
                return Some(linux_error(RISCV_LINUX_EINVAL));
            }
            let Some(bytes) = read_iovec_prefix(
                guest_memory,
                &iovecs,
                eventfd_write_bytes_written() as usize,
            ) else {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            };
            return match state.write_guest_eventfd_from_fd(fd, &bytes) {
                Ok(Some(write)) => eventfd_write_result(write),
                Ok(None) | Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
            };
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }

    if total == 0 {
        return Some(0);
    }
    match state.guest_file_write_exceeds_dense_limit(fd, total) {
        Ok(true) => return Some(linux_error(RISCV_LINUX_EFBIG)),
        Ok(false) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    let socket_write = match usize::try_from(total)
        .ok()
        .map(|byte_count| state.guest_socket_write_plan(fd, byte_count))
    {
        Some(Ok(RiscvGuestSocketWrite::NotSocket)) | None => None,
        Some(Ok(RiscvGuestSocketWrite::Written(written))) => Some(written),
        Some(Ok(write @ RiscvGuestSocketWrite::WouldBlock))
        | Some(Ok(write @ RiscvGuestSocketWrite::Blocked))
        | Some(Ok(write @ RiscvGuestSocketWrite::BrokenPipe)) => {
            return socket_write_result(write);
        }
        Some(Err(_)) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    if let Some(written) = socket_write {
        let Some(bytes) = read_iovec_prefix(guest_memory, &iovecs, written) else {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        };
        return match state.write_guest_socket_from_fd(fd, &bytes) {
            Ok(write) => socket_write_result(write),
            Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
        };
    }
    let pipe_write = match usize::try_from(total)
        .ok()
        .map(|byte_count| state.guest_pipe_write_plan(fd, byte_count))
    {
        Some(Ok(RiscvGuestPipeWrite::NotPipe)) | None => None,
        Some(Ok(RiscvGuestPipeWrite::Written(written))) => Some(written),
        Some(Ok(RiscvGuestPipeWrite::WouldBlock)) => return Some(linux_error(RISCV_LINUX_EAGAIN)),
        Some(Ok(RiscvGuestPipeWrite::Blocked)) => return None,
        Some(Err(_)) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };

    let bytes = match pipe_write {
        Some(written) => {
            let Some(bytes) = read_iovec_prefix(guest_memory, &iovecs, written) else {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            };
            return match state.write_guest_pipe_from_fd(fd, &bytes) {
                Ok(RiscvGuestPipeWrite::Written(written)) => Some(written as u64),
                Ok(RiscvGuestPipeWrite::WouldBlock) => Some(linux_error(RISCV_LINUX_EAGAIN)),
                Ok(RiscvGuestPipeWrite::Blocked) => None,
                Ok(RiscvGuestPipeWrite::NotPipe) | Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
            };
        }
        None => {
            let Some(bytes) = read_iovec_bytes(guest_memory, &iovecs) else {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            };
            bytes
        }
    };

    match state.write_guest_file_from_fd(fd, &bytes) {
        Ok(_) => {}
        Err(RiscvGuestFileWriteError::FileTooLarge) => {
            return Some(linux_error(RISCV_LINUX_EFBIG));
        }
        Err(RiscvGuestFileWriteError::Permission) => {
            return Some(linux_error(RISCV_LINUX_EPERM));
        }
        Err(RiscvGuestFileWriteError::Fd(_)) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    if state.guest_fds.advance_file_offset(fd, total).is_err() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    state.push_guest_write(RiscvGuestWriteRecord::new(fd, iov_base, tick, bytes));
    Some(total)
}

pub(super) fn syscall_pwritev(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
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
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
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
    let iov_base = request.argument(1);
    let (iovecs, total) = match read_iovecs(guest_memory, iov_base, iov_count) {
        Ok(iovecs) => iovecs,
        Err(errno) => return linux_error(errno),
    };
    if total == 0 {
        return 0;
    }
    let offset = match state.guest_file_append_offset(fd) {
        Ok(Some(append_offset)) => append_offset,
        Ok(None) => offset,
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    match state.guest_file_write_at_exceeds_dense_limit(fd, offset, total) {
        Ok(true) => return linux_error(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let Some(bytes) = read_iovec_bytes(guest_memory, &iovecs) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    match state.write_guest_file_from_fd_at(fd, offset, &bytes) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Permission) => return linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, iov_base, tick, bytes));
    total
}
