use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFdTable, GuestFileDescription,
    GuestFileDescriptionId, GuestFileStatusFlags,
};

use super::{
    guest_fd_argument, linux_error, RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_DUP: u64 = 23;
pub(super) const RISCV_LINUX_DUP3: u64 = 24;
pub(super) const RISCV_LINUX_CLOSE: u64 = 57;

pub(super) fn linux_standard_guest_fds() -> GuestFdTable {
    let mut table = GuestFdTable::new();
    for (fd, description, flags) in [
        (0, 0, RISCV_LINUX_O_RDONLY),
        (1, 1, RISCV_LINUX_O_WRONLY),
        (2, 2, RISCV_LINUX_O_WRONLY),
    ] {
        let description = GuestFileDescriptionId::new(description);
        table
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(flags as u32),
            ))
            .expect("standard RISC-V Linux file description is unique");
        table
            .insert(
                GuestFd::new(fd).expect("standard RISC-V Linux fd is non-negative"),
                GuestFdEntry::new(description),
            )
            .expect("standard RISC-V Linux fd is unique");
    }
    table
}

pub(super) fn syscall_close(fd_argument: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fds.close_descriptor(fd) {
        Ok(record) => {
            state.close_fd_sources(&record);
            0
        }
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_dup(old_fd_argument: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(old_fd) = guest_fd_argument(old_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fds.dup(old_fd) {
        Ok(new_fd) => {
            state.duplicate_fd_source(old_fd, new_fd);
            u64::from(new_fd.get())
        }
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_dup3(
    old_fd_argument: u64,
    new_fd_argument: u64,
    flags: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    if flags & !RISCV_LINUX_O_CLOEXEC != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(old_fd) = guest_fd_argument(old_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Some(new_fd) = guest_fd_argument(new_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if old_fd == new_fd {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.guest_fds.dup2_with_replacement(old_fd, new_fd) {
        Ok(record) => {
            state.duplicate_fd_source(old_fd, record.fd());
            state.release_replaced_fd_sources(&record);
            if flags & RISCV_LINUX_O_CLOEXEC != 0
                && state
                    .guest_fds
                    .set_close_on_exec(record.fd(), true)
                    .is_err()
            {
                return linux_error(RISCV_LINUX_EBADF);
            }
            u64::from(record.fd().get())
        }
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}
