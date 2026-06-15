use crate::{GuestFd, GuestFileOffset};

use super::{
    file_write::RiscvGuestFileWriteError, guest_fd_argument, linux_error, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG, RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_SENDFILE: u64 = 71;

pub(super) fn syscall_sendfile(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let offset_pointer = request.argument(2);
    let explicit_input_offset =
        match read_explicit_sendfile_offset(offset_pointer, guest_memory_reader) {
            Ok(offset) => offset,
            Err(error) => return linux_error(error),
        };
    let Some(out_fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Some(in_fd) = guest_fd_argument(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if let Err(error) = validate_sendfile_fds(out_fd, in_fd, state) {
        return linux_error(error);
    }

    let count = request.argument(3);
    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };

    let input_offset =
        match sendfile_input_offset(explicit_input_offset, offset_pointer, in_fd, state) {
            Ok(offset) => offset,
            Err(error) => return linux_error(error),
        };
    if count == 0 {
        return match finish_sendfile_input_offset(
            input_offset,
            in_fd,
            out_fd,
            0,
            state,
            guest_memory_writer,
        ) {
            Ok(()) => 0,
            Err(error) => linux_error(error),
        };
    }

    let bytes = match state.guest_file_slice_at(in_fd, input_offset.value, byte_count) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if bytes.is_empty() {
        return match finish_sendfile_input_offset(
            input_offset,
            in_fd,
            out_fd,
            0,
            state,
            guest_memory_writer,
        ) {
            Ok(()) => 0,
            Err(error) => linux_error(error),
        };
    }

    let copied = bytes.len() as u64;
    match state.guest_file_write_exceeds_dense_limit(out_fd, copied) {
        Ok(true) => return linux_error(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    match state.write_guest_file_from_fd(out_fd, &bytes) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }
    if state.guest_fds.advance_file_offset(out_fd, copied).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    if let Err(error) = finish_sendfile_input_offset(
        input_offset,
        in_fd,
        out_fd,
        copied,
        state,
        guest_memory_writer,
    ) {
        return linux_error(error);
    }

    copied
}

#[derive(Clone, Copy)]
struct SendfileInputOffset {
    explicit_pointer: Option<u64>,
    value: u64,
}

fn validate_sendfile_fds(
    out_fd: GuestFd,
    in_fd: GuestFd,
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
        Ok(false) => return Err(RISCV_LINUX_ESPIPE),
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
        return Err(RISCV_LINUX_EINVAL);
    }
    match state.guest_file_fd_is_seekable(out_fd) {
        Ok(true) => Ok(()),
        Ok(false) => Err(RISCV_LINUX_EINVAL),
        Err(_) => Err(RISCV_LINUX_EBADF),
    }
}

fn read_explicit_sendfile_offset(
    offset_pointer: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<Option<u64>, u64> {
    if offset_pointer == 0 {
        return Ok(None);
    }

    let Some(reader) = guest_memory_reader else {
        return Err(RISCV_LINUX_EFAULT);
    };
    let Some(bytes) = reader.read(offset_pointer, 8) else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != 8 {
        return Err(RISCV_LINUX_EFAULT);
    }
    let offset = i64::from_le_bytes(bytes.try_into().map_err(|_| RISCV_LINUX_EFAULT)?);
    if offset < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(Some(offset as u64))
}

fn sendfile_input_offset(
    explicit_input_offset: Option<u64>,
    offset_pointer: u64,
    in_fd: GuestFd,
    state: &RiscvSyscallState,
) -> Result<SendfileInputOffset, u64> {
    match explicit_input_offset {
        Some(value) => Ok(SendfileInputOffset {
            explicit_pointer: Some(offset_pointer),
            value,
        }),
        None => state
            .guest_fds
            .file_offset(in_fd)
            .map(|offset| SendfileInputOffset {
                explicit_pointer: None,
                value: GuestFileOffset::get(offset),
            })
            .map_err(|_| RISCV_LINUX_EBADF),
    }
}

fn finish_sendfile_input_offset(
    input_offset: SendfileInputOffset,
    in_fd: GuestFd,
    out_fd: GuestFd,
    copied: u64,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Result<(), u64> {
    if let Some(pointer) = input_offset.explicit_pointer {
        let Some(writer) = guest_memory_writer else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let updated = input_offset.value.saturating_add(copied);
        if !writer.write(pointer, &updated.to_le_bytes()) {
            return Err(RISCV_LINUX_EFAULT);
        }
        return Ok(());
    }

    if copied == 0 {
        return Ok(());
    }
    let same_description = state
        .guest_fds
        .entry(out_fd)
        .map(|entry| entry.description())
        == state
            .guest_fds
            .entry(in_fd)
            .map(|entry| entry.description());
    if same_description || state.guest_fds.advance_file_offset(in_fd, copied).is_ok() {
        Ok(())
    } else {
        Err(RISCV_LINUX_EBADF)
    }
}
