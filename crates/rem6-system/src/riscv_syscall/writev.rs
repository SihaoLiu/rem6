use super::{
    file_write::RiscvGuestFileWriteError, guest_fd_argument, linux_error, RiscvGuestMemoryReader,
    RiscvGuestWriteRecord, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG, RISCV_LINUX_EINVAL, RISCV_LINUX_O_ACCMODE,
    RISCV_LINUX_O_RDONLY,
};
use crate::Tick;

pub(super) const RISCV_LINUX_WRITEV: u64 = 66;

const RISCV_LINUX_IOV_BYTES: usize = 16;
const RISCV_LINUX_IOV_MAX: u64 = 1024;

#[derive(Clone, Copy)]
struct RiscvIovec {
    address: u64,
    len: u64,
}

pub(super) fn syscall_writev(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let iov_count = request.argument(2);
    if iov_count > RISCV_LINUX_IOV_MAX {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if iov_count == 0 {
        return 0;
    }

    let iov_base = request.argument(1);
    let mut iovecs = Vec::with_capacity(iov_count as usize);
    let mut total = 0_u64;
    for index in 0..iov_count {
        let Some(iov_address) = iov_base.checked_add(index * RISCV_LINUX_IOV_BYTES as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let Some(iov) = read_guest_exact(guest_memory, iov_address, RISCV_LINUX_IOV_BYTES) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let data_address = le_u64(&iov, 0);
        let data_len = le_u64(&iov, 8);
        total = match total.checked_add(data_len) {
            Some(total) => total,
            None => return linux_error(RISCV_LINUX_EINVAL),
        };
        if usize::try_from(data_len).is_err() {
            return linux_error(RISCV_LINUX_EINVAL);
        }
        iovecs.push(RiscvIovec {
            address: data_address,
            len: data_len,
        });
    }
    if total == 0 {
        return 0;
    }
    match state.guest_file_write_exceeds_dense_limit(fd, total) {
        Ok(true) => return linux_error(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }

    let mut bytes = Vec::new();
    for iovec in iovecs {
        let data_len = usize::try_from(iovec.len).expect("iovec length was validated");
        if data_len == 0 {
            continue;
        }
        let Some(mut data) = read_guest_exact(guest_memory, iovec.address, data_len) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        bytes.append(&mut data);
    }

    match state.write_guest_file_from_fd(fd, &bytes) {
        Ok(_) => {}
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }
    if state.guest_fds.advance_file_offset(fd, total).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    state.push_guest_write(RiscvGuestWriteRecord::new(fd, iov_base, tick, bytes));
    total
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
