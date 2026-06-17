use super::{
    eventfd::{eventfd_read_bytes, RiscvGuestEventFdRead},
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_READV: u64 = 65;

const RISCV_LINUX_IOV_BYTES: usize = 16;
const RISCV_LINUX_IOV_MAX: u64 = 1024;

#[derive(Clone, Copy)]
struct RiscvIovec {
    address: u64,
    len: u64,
}

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

fn read_iovecs(
    guest_memory: &RiscvGuestMemoryReader,
    iov_base: u64,
    iov_count: u64,
) -> Result<(Vec<RiscvIovec>, u64), u64> {
    let mut iovecs = Vec::with_capacity(iov_count as usize);
    let mut total = 0_u64;
    for index in 0..iov_count {
        let Some(iov_address) = iov_base.checked_add(index * RISCV_LINUX_IOV_BYTES as u64) else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let Some(iov) = read_guest_exact(guest_memory, iov_address, RISCV_LINUX_IOV_BYTES) else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let data_address = le_u64(&iov, 0);
        let data_len = le_u64(&iov, 8);
        total = total.checked_add(data_len).ok_or(RISCV_LINUX_EINVAL)?;
        iovecs.push(RiscvIovec {
            address: data_address,
            len: data_len,
        });
    }
    Ok((iovecs, total))
}

fn write_iovecs(
    guest_memory: &RiscvGuestMemoryWriter,
    iovecs: &[RiscvIovec],
    bytes: &[u8],
) -> bool {
    let mut offset = 0usize;
    for iovec in iovecs {
        if offset == bytes.len() {
            return true;
        }
        let Ok(iov_len) = usize::try_from(iovec.len) else {
            return false;
        };
        if iov_len == 0 {
            continue;
        }
        let chunk_len = iov_len.min(bytes.len() - offset);
        if !guest_memory.write(iovec.address, &bytes[offset..offset + chunk_len]) {
            return false;
        }
        offset += chunk_len;
    }
    offset == bytes.len()
}

fn read_guest_exact(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    len: usize,
) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    guest_memory
        .read(address, len)
        .filter(|bytes| bytes.len() == len)
}

fn le_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut raw = [0; 8];
    raw.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(raw)
}
