use super::{
    guest_fd_argument,
    iovec::{read_iovec_prefix, read_iovecs, RISCV_LINUX_IOV_MAX},
    linux_error,
    pipe::RiscvGuestPipeWrite,
    RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_VMSPLICE: u64 = 75;

const RISCV_LINUX_SPLICE_F_MOVE: u64 = 0x01;
const RISCV_LINUX_SPLICE_F_NONBLOCK: u64 = 0x02;
const RISCV_LINUX_SPLICE_F_MORE: u64 = 0x04;
const RISCV_LINUX_SPLICE_F_GIFT: u64 = 0x08;
const RISCV_LINUX_VMSPLICE_SUPPORTED_FLAGS: u64 = RISCV_LINUX_SPLICE_F_MOVE
    | RISCV_LINUX_SPLICE_F_NONBLOCK
    | RISCV_LINUX_SPLICE_F_MORE
    | RISCV_LINUX_SPLICE_F_GIFT;

pub(super) fn syscall_vmsplice(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> RiscvSyscallOutcome {
    if request.argument(3) & !RISCV_LINUX_VMSPLICE_SUPPORTED_FLAGS != 0 {
        return vmsplice_return(linux_error(RISCV_LINUX_EINVAL));
    }
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return vmsplice_return(linux_error(RISCV_LINUX_EBADF));
    };
    let is_pipe = match state.guest_fd_is_pipe(fd) {
        Ok(is_pipe) => is_pipe,
        Err(_) => return vmsplice_return(linux_error(RISCV_LINUX_EBADF)),
    };
    if !is_pipe {
        return vmsplice_return(linux_error(RISCV_LINUX_EBADF));
    }

    let iov_count = request.argument(2);
    if iov_count > RISCV_LINUX_IOV_MAX {
        return vmsplice_return(linux_error(RISCV_LINUX_EINVAL));
    }
    if iov_count == 0 {
        return vmsplice_return(0);
    }
    let (iovecs, total) = match read_iovecs(guest_memory, request.argument(1), iov_count) {
        Ok(iovecs) => iovecs,
        Err(errno) => return vmsplice_return(linux_error(errno)),
    };
    if total == 0 {
        return vmsplice_return(0);
    }
    let Ok(total) = usize::try_from(total) else {
        return vmsplice_return(linux_error(RISCV_LINUX_EINVAL));
    };

    let nonblocking = request.argument(3) & RISCV_LINUX_SPLICE_F_NONBLOCK != 0;
    let planned = match state.guest_pipe_write_plan_with_nonblocking_hint(fd, total, nonblocking) {
        Ok(RiscvGuestPipeWrite::Written(written)) => written,
        Ok(RiscvGuestPipeWrite::WouldBlock) => {
            return vmsplice_return(linux_error(RISCV_LINUX_EAGAIN));
        }
        Ok(RiscvGuestPipeWrite::Blocked) => return RiscvSyscallOutcome::Blocked,
        Ok(RiscvGuestPipeWrite::NotPipe) => return vmsplice_return(linux_error(RISCV_LINUX_EBADF)),
        Err(_) => return vmsplice_return(linux_error(RISCV_LINUX_EBADF)),
    };
    let Some(bytes) = read_iovec_prefix(guest_memory, &iovecs, planned) else {
        return vmsplice_return(linux_error(RISCV_LINUX_EFAULT));
    };
    match state.write_guest_pipe_from_fd_with_nonblocking_hint(fd, &bytes, nonblocking) {
        Ok(RiscvGuestPipeWrite::Written(written)) => vmsplice_return(written as u64),
        Ok(RiscvGuestPipeWrite::WouldBlock) => vmsplice_return(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestPipeWrite::Blocked) => RiscvSyscallOutcome::Blocked,
        Ok(RiscvGuestPipeWrite::NotPipe) | Err(_) => {
            vmsplice_return(linux_error(RISCV_LINUX_EBADF))
        }
    }
}

const fn vmsplice_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}
