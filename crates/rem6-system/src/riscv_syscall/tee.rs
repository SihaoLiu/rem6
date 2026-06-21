use super::{
    guest_fd_argument, linux_error,
    pipe::{RiscvGuestPipeRead, RiscvGuestPipeWrite},
    splice_flags::{splice_flags_are_nonblocking, splice_flags_are_supported},
    RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_EINVAL, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_RDONLY,
    RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_TEE: u64 = 77;

pub(super) fn syscall_tee(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> RiscvSyscallOutcome {
    let flags = request.argument(3);
    if !splice_flags_are_supported(flags) {
        return tee_return(linux_error(RISCV_LINUX_EINVAL));
    }
    let Ok(byte_count) = usize::try_from(request.argument(2)) else {
        return tee_return(linux_error(RISCV_LINUX_EINVAL));
    };
    if byte_count == 0 {
        return tee_return(0);
    }

    let Some(input_fd) = guest_fd_argument(request.argument(0)) else {
        return tee_return(linux_error(RISCV_LINUX_EBADF));
    };
    let Some(output_fd) = guest_fd_argument(request.argument(1)) else {
        return tee_return(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(input_status) = state.guest_fds.status_flags(input_fd) else {
        return tee_return(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(output_status) = state.guest_fds.status_flags(output_fd) else {
        return tee_return(linux_error(RISCV_LINUX_EBADF));
    };
    if input_status.bits() & RISCV_LINUX_O_ACCMODE as u32 == RISCV_LINUX_O_WRONLY as u32
        || output_status.bits() & RISCV_LINUX_O_ACCMODE as u32 == RISCV_LINUX_O_RDONLY as u32
    {
        return tee_return(linux_error(RISCV_LINUX_EBADF));
    }

    let input_is_pipe = match state.guest_fd_is_pipe(input_fd) {
        Ok(is_pipe) => is_pipe,
        Err(_) => return tee_return(linux_error(RISCV_LINUX_EBADF)),
    };
    let output_is_pipe = match state.guest_fd_is_pipe(output_fd) {
        Ok(is_pipe) => is_pipe,
        Err(_) => return tee_return(linux_error(RISCV_LINUX_EBADF)),
    };
    if !input_is_pipe || !output_is_pipe {
        return tee_return(linux_error(RISCV_LINUX_EINVAL));
    }
    match state.guest_fds_share_pipe(input_fd, output_fd) {
        Ok(true) => return tee_return(linux_error(RISCV_LINUX_EINVAL)),
        Ok(false) => {}
        Err(_) => return tee_return(linux_error(RISCV_LINUX_EBADF)),
    }

    let nonblocking = splice_flags_are_nonblocking(flags);
    let bytes = match state.guest_pipe_read_with_nonblocking_hint(input_fd, byte_count, nonblocking)
    {
        Ok(RiscvGuestPipeRead::Bytes(bytes)) => bytes,
        Ok(RiscvGuestPipeRead::WouldBlock) => {
            return tee_return(linux_error(RISCV_LINUX_EAGAIN));
        }
        Ok(RiscvGuestPipeRead::Blocked) if nonblocking => {
            return tee_return(linux_error(RISCV_LINUX_EAGAIN));
        }
        Ok(RiscvGuestPipeRead::Blocked) => return RiscvSyscallOutcome::Blocked,
        Ok(RiscvGuestPipeRead::NotPipe) | Err(_) => {
            return tee_return(linux_error(RISCV_LINUX_EBADF));
        }
    };
    if bytes.is_empty() {
        return tee_return(0);
    }

    match state.write_guest_pipe_from_fd_with_nonblocking_hint(output_fd, &bytes, nonblocking) {
        Ok(RiscvGuestPipeWrite::Written(written)) => tee_return(written as u64),
        Ok(RiscvGuestPipeWrite::WouldBlock) => tee_return(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestPipeWrite::Blocked) if nonblocking => {
            tee_return(linux_error(RISCV_LINUX_EAGAIN))
        }
        Ok(RiscvGuestPipeWrite::Blocked) => RiscvSyscallOutcome::Blocked,
        Ok(RiscvGuestPipeWrite::NotPipe) | Err(_) => tee_return(linux_error(RISCV_LINUX_EBADF)),
    }
}

const fn tee_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}
