use crate::{GuestFd, GuestFileOffset};

use super::{
    file_write::RiscvGuestFileWriteError,
    guest_fd_argument, linux_error,
    pipe::{RiscvGuestPipeRead, RiscvGuestPipeWrite},
    splice_flags::{splice_flags_are_nonblocking, splice_flags_are_supported},
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EFBIG, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM, RISCV_LINUX_ESPIPE,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_SPLICE: u64 = 76;

pub(super) fn syscall_splice(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    if !splice_flags_are_supported(request.argument(5)) {
        return splice_return(linux_error(RISCV_LINUX_EINVAL));
    }
    let Some(in_fd) = guest_fd_argument(request.argument(0)) else {
        return splice_return(linux_error(RISCV_LINUX_EBADF));
    };
    let Some(out_fd) = guest_fd_argument(request.argument(2)) else {
        return splice_return(linux_error(RISCV_LINUX_EBADF));
    };
    if !match (
        state.guest_fd_is_pipe(in_fd),
        state.guest_fd_is_pipe(out_fd),
    ) {
        (Ok(input_pipe), Ok(output_pipe)) => input_pipe || output_pipe,
        _ => return splice_return(linux_error(RISCV_LINUX_EBADF)),
    } {
        return splice_return(linux_error(RISCV_LINUX_EINVAL));
    }
    match state.guest_fds_share_pipe(in_fd, out_fd) {
        Ok(true) => return splice_return(linux_error(RISCV_LINUX_EINVAL)),
        Ok(false) => {}
        Err(_) => return splice_return(linux_error(RISCV_LINUX_EBADF)),
    }
    let count = request.argument(4);
    let Ok(byte_count) = usize::try_from(count) else {
        return splice_return(linux_error(RISCV_LINUX_EINVAL));
    };
    if byte_count == 0 {
        return splice_return(0);
    }

    let input_offset = match splice_offset(in_fd, request.argument(1), state, guest_memory_reader) {
        Ok(offset) => offset,
        Err(errno) => return splice_return(linux_error(errno)),
    };
    let output_offset = match splice_offset(out_fd, request.argument(3), state, guest_memory_reader)
    {
        Ok(offset) => offset,
        Err(errno) => return splice_return(linux_error(errno)),
    };
    let nonblocking = splice_flags_are_nonblocking(request.argument(5));
    match splice_bytes(
        in_fd,
        out_fd,
        input_offset.value,
        byte_count,
        nonblocking,
        state,
    ) {
        SpliceBytes::WouldBlock => splice_return(linux_error(RISCV_LINUX_EAGAIN)),
        SpliceBytes::Blocked if nonblocking => splice_return(linux_error(RISCV_LINUX_EAGAIN)),
        SpliceBytes::Blocked => RiscvSyscallOutcome::Blocked,
        SpliceBytes::Errno(errno) => splice_return(linux_error(errno)),
        SpliceBytes::Bytes(bytes) => {
            if bytes.is_empty() {
                return splice_return(0);
            }
            match write_splice_bytes(out_fd, output_offset.value, &bytes, nonblocking, state) {
                SpliceWrite::Blocked if nonblocking => {
                    splice_return(linux_error(RISCV_LINUX_EAGAIN))
                }
                SpliceWrite::Blocked => RiscvSyscallOutcome::Blocked,
                SpliceWrite::Errno(errno) => splice_return(linux_error(errno)),
                SpliceWrite::Written(written) => {
                    let copied = written as u64;
                    if let Err(errno) = finish_splice_offset(
                        input_offset,
                        in_fd,
                        copied,
                        state,
                        guest_memory_writer,
                    )
                    .and_then(|()| {
                        finish_splice_offset(
                            output_offset,
                            out_fd,
                            copied,
                            state,
                            guest_memory_writer,
                        )
                    })
                    .and_then(|()| consume_splice_input(in_fd, copied, state))
                    {
                        return splice_return(linux_error(errno));
                    }
                    splice_return(copied)
                }
            }
        }
    }
}

const fn splice_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}

#[derive(Clone, Copy)]
struct SpliceOffset {
    pointer: Option<u64>,
    value: u64,
}

enum SpliceBytes {
    Bytes(Vec<u8>),
    WouldBlock,
    Blocked,
    Errno(u64),
}

enum SpliceWrite {
    Written(usize),
    Blocked,
    Errno(u64),
}

fn splice_offset(
    fd: GuestFd,
    pointer: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<SpliceOffset, u64> {
    if pointer == 0 {
        return state
            .guest_fds
            .file_offset(fd)
            .map(|offset| SpliceOffset {
                pointer: None,
                value: offset.get(),
            })
            .map_err(|_| RISCV_LINUX_EBADF);
    }

    if !state
        .guest_file_fd_is_seekable(fd)
        .map_err(|_| RISCV_LINUX_EBADF)?
    {
        return Err(RISCV_LINUX_ESPIPE);
    }
    let Some(reader) = guest_memory_reader else {
        return Err(RISCV_LINUX_EFAULT);
    };
    let Some(bytes) = reader.read(pointer, 8) else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != 8 {
        return Err(RISCV_LINUX_EFAULT);
    }
    let value = i64::from_le_bytes(bytes.try_into().map_err(|_| RISCV_LINUX_EFAULT)?);
    if value < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(SpliceOffset {
        pointer: Some(pointer),
        value: value as u64,
    })
}

fn splice_bytes(
    in_fd: GuestFd,
    out_fd: GuestFd,
    input_offset: u64,
    byte_count: usize,
    nonblocking: bool,
    state: &RiscvSyscallState,
) -> SpliceBytes {
    let Ok(status_flags) = state.guest_fds.status_flags(in_fd) else {
        return SpliceBytes::Errno(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return SpliceBytes::Errno(RISCV_LINUX_EBADF);
    }

    match state.guest_pipe_read_with_nonblocking_hint(in_fd, byte_count, nonblocking) {
        Ok(RiscvGuestPipeRead::Bytes(bytes)) => return SpliceBytes::Bytes(bytes),
        Ok(RiscvGuestPipeRead::WouldBlock) => return SpliceBytes::WouldBlock,
        Ok(RiscvGuestPipeRead::Blocked) => return SpliceBytes::Blocked,
        Ok(RiscvGuestPipeRead::NotPipe) => {}
        Err(_) => return SpliceBytes::Errno(RISCV_LINUX_EBADF),
    }
    if !state.guest_file_fd_is_seekable(in_fd).unwrap_or(false) {
        return SpliceBytes::Errno(RISCV_LINUX_EINVAL);
    }
    if same_file_description(in_fd, out_fd, state) {
        return SpliceBytes::Errno(RISCV_LINUX_EINVAL);
    }
    match state.guest_file_slice_at(in_fd, input_offset, byte_count) {
        Ok(Some(bytes)) => SpliceBytes::Bytes(bytes),
        Ok(None) => SpliceBytes::Errno(RISCV_LINUX_EINVAL),
        Err(_) => SpliceBytes::Errno(RISCV_LINUX_EBADF),
    }
}

fn write_splice_bytes(
    out_fd: GuestFd,
    output_offset: u64,
    bytes: &[u8],
    nonblocking: bool,
    state: &mut RiscvSyscallState,
) -> SpliceWrite {
    let Ok(status_flags) = state.guest_fds.status_flags(out_fd) else {
        return SpliceWrite::Errno(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return SpliceWrite::Errno(RISCV_LINUX_EBADF);
    }

    match state.write_guest_pipe_from_fd_with_nonblocking_hint(out_fd, bytes, nonblocking) {
        Ok(RiscvGuestPipeWrite::Written(written)) => return SpliceWrite::Written(written),
        Ok(RiscvGuestPipeWrite::WouldBlock) => return SpliceWrite::Errno(RISCV_LINUX_EAGAIN),
        Ok(RiscvGuestPipeWrite::Blocked) => return SpliceWrite::Blocked,
        Ok(RiscvGuestPipeWrite::NotPipe) => {}
        Err(_) => return SpliceWrite::Errno(RISCV_LINUX_EBADF),
    }

    if status_flags.bits() & RISCV_LINUX_O_APPEND as u32 != 0 {
        return SpliceWrite::Errno(RISCV_LINUX_EINVAL);
    }
    match state.guest_file_write_at_exceeds_dense_limit(out_fd, output_offset, bytes.len() as u64) {
        Ok(true) => return SpliceWrite::Errno(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return SpliceWrite::Errno(RISCV_LINUX_EBADF),
    }
    match state.write_guest_file_from_fd_at(out_fd, output_offset, bytes) {
        Ok(true) => SpliceWrite::Written(bytes.len()),
        Ok(false) => SpliceWrite::Errno(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileWriteError::FileTooLarge) => SpliceWrite::Errno(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Permission) => SpliceWrite::Errno(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileWriteError::Fd(_)) => SpliceWrite::Errno(RISCV_LINUX_EBADF),
    }
}

fn finish_splice_offset(
    offset: SpliceOffset,
    fd: GuestFd,
    copied: u64,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Result<(), u64> {
    if !state
        .guest_file_fd_is_seekable(fd)
        .map_err(|_| RISCV_LINUX_EBADF)?
    {
        return Ok(());
    }
    let updated = offset.value.saturating_add(copied);
    if let Some(pointer) = offset.pointer {
        let Some(writer) = guest_memory_writer else {
            return Err(RISCV_LINUX_EFAULT);
        };
        if writer.write(pointer, &updated.to_le_bytes()) {
            Ok(())
        } else {
            Err(RISCV_LINUX_EFAULT)
        }
    } else {
        state
            .guest_fds
            .set_file_offset(fd, GuestFileOffset::new(updated))
            .map_err(|_| RISCV_LINUX_EBADF)
    }
}

fn consume_splice_input(
    in_fd: GuestFd,
    copied: u64,
    state: &mut RiscvSyscallState,
) -> Result<(), u64> {
    if state
        .guest_pipe_prefix(in_fd, usize::try_from(copied).unwrap_or(usize::MAX))
        .map_err(|_| RISCV_LINUX_EBADF)?
        .is_some()
    {
        return state
            .consume_guest_pipe_prefix(
                in_fd,
                usize::try_from(copied).map_err(|_| RISCV_LINUX_EBADF)?,
            )
            .map_err(|_| RISCV_LINUX_EBADF);
    }
    Ok(())
}

fn same_file_description(left: GuestFd, right: GuestFd, state: &RiscvSyscallState) -> bool {
    state.guest_fds.entry(left).map(|entry| entry.description())
        == state
            .guest_fds
            .entry(right)
            .map(|entry| entry.description())
}
