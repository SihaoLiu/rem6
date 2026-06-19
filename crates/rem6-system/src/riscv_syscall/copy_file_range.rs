use crate::GuestFd;

use super::{
    file_write::RiscvGuestFileWriteError, guest_fd_argument, linux_error, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_COPY_FILE_RANGE: u64 = 285;

pub(super) fn syscall_copy_file_range(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let Some(in_fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Some(out_fd) = guest_fd_argument(request.argument(2)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if let Err(errno) = validate_copy_file_range_fds(in_fd, out_fd, state) {
        return linux_error(errno);
    }

    let input_offset =
        match copy_file_range_offset(in_fd, request.argument(1), state, guest_memory_reader) {
            Ok(offset) => offset,
            Err(errno) => return linux_error(errno),
        };
    let output_offset =
        match copy_file_range_offset(out_fd, request.argument(3), state, guest_memory_reader) {
            Ok(offset) => offset,
            Err(errno) => return linux_error(errno),
        };
    if request.argument(5) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let Ok(byte_count) = usize::try_from(request.argument(4)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if byte_count == 0 {
        return 0;
    }

    let bytes = match state.guest_file_slice_at(in_fd, input_offset.value, byte_count) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return linux_error(RISCV_LINUX_EINVAL),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if bytes.is_empty() {
        return 0;
    }

    let copied = bytes.len() as u64;
    if let Err(errno) = validate_copy_file_range_extents(
        in_fd,
        out_fd,
        input_offset.value,
        output_offset.value,
        copied,
        state,
    ) {
        return linux_error(errno);
    }
    match state.write_guest_file_from_fd_at(out_fd, output_offset.value, &bytes) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Permission) => return linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }

    if let Err(errno) =
        finish_copy_file_range_offset(input_offset, in_fd, copied, state, guest_memory_writer)
            .and_then(|()| {
                finish_copy_file_range_offset(
                    output_offset,
                    out_fd,
                    copied,
                    state,
                    guest_memory_writer,
                )
            })
    {
        return linux_error(errno);
    }

    copied
}

#[derive(Clone, Copy)]
struct CopyFileRangeOffset {
    pointer: Option<u64>,
    value: u64,
}

fn copy_file_range_offset(
    fd: GuestFd,
    pointer: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<CopyFileRangeOffset, u64> {
    if pointer == 0 {
        return state
            .guest_fds
            .file_offset(fd)
            .map(|offset| CopyFileRangeOffset {
                pointer: None,
                value: offset.get(),
            })
            .map_err(|_| RISCV_LINUX_EBADF);
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
    let offset = i64::from_le_bytes(bytes.try_into().map_err(|_| RISCV_LINUX_EFAULT)?);
    if offset < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(CopyFileRangeOffset {
        pointer: Some(pointer),
        value: offset as u64,
    })
}

fn validate_copy_file_range_fds(
    in_fd: GuestFd,
    out_fd: GuestFd,
    state: &RiscvSyscallState,
) -> Result<(), u64> {
    let in_status_flags = state
        .guest_fds
        .status_flags(in_fd)
        .map_err(|_| RISCV_LINUX_EBADF)?;
    if in_status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return Err(RISCV_LINUX_EBADF);
    }
    match state.guest_file_fd_is_seekable(in_fd) {
        Ok(true) => {}
        Ok(false) => return Err(RISCV_LINUX_EINVAL),
        Err(_) => return Err(RISCV_LINUX_EBADF),
    }

    let out_status_flags = state
        .guest_fds
        .status_flags(out_fd)
        .map_err(|_| RISCV_LINUX_EBADF)?;
    if out_status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return Err(RISCV_LINUX_EBADF);
    }
    if out_status_flags.bits() & RISCV_LINUX_O_APPEND as u32 != 0 {
        return Err(RISCV_LINUX_EBADF);
    }
    match state.guest_file_fd_is_seekable(out_fd) {
        Ok(true) => Ok(()),
        Ok(false) => Err(RISCV_LINUX_EINVAL),
        Err(_) => Err(RISCV_LINUX_EBADF),
    }
}

fn validate_copy_file_range_extents(
    in_fd: GuestFd,
    out_fd: GuestFd,
    input_offset: u64,
    output_offset: u64,
    count: u64,
    state: &RiscvSyscallState,
) -> Result<(), u64> {
    if count == 0 {
        return Ok(());
    }
    let input_description = state
        .guest_fds
        .entry(in_fd)
        .ok_or(RISCV_LINUX_EBADF)?
        .description();
    let output_description = state
        .guest_fds
        .entry(out_fd)
        .ok_or(RISCV_LINUX_EBADF)?
        .description();
    let Some(input_stat) = state.guest_file_stats.get(&input_description) else {
        return Err(RISCV_LINUX_EINVAL);
    };
    let Some(output_stat) = state.guest_file_stats.get(&output_description) else {
        return Err(RISCV_LINUX_EINVAL);
    };
    if input_stat.identity == output_stat.identity
        && copy_file_range_extents_overlap(input_offset, output_offset, count)
    {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(())
}

fn copy_file_range_extents_overlap(left_start: u64, right_start: u64, count: u64) -> bool {
    let left_end = left_start.saturating_add(count);
    let right_end = right_start.saturating_add(count);
    left_start < right_end && right_start < left_end
}

fn finish_copy_file_range_offset(
    offset: CopyFileRangeOffset,
    fd: GuestFd,
    copied: u64,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Result<(), u64> {
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
            .advance_file_offset(fd, copied)
            .map(|_| ())
            .map_err(|_| RISCV_LINUX_EBADF)
    }
}
