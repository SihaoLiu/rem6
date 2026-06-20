use crate::{
    GuestFd, GuestFdEntry, GuestFdTable, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

use super::{
    guest_fd_argument, linux_error, RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_DUP: u64 = 23;
pub(super) const RISCV_LINUX_DUP3: u64 = 24;
pub(super) const RISCV_LINUX_CLOSE: u64 = 57;
pub(super) const RISCV_LINUX_CLOSE_RANGE: u64 = 436;
const RISCV_LINUX_CLOSE_RANGE_UNSHARE: u64 = 1 << 1;
const RISCV_LINUX_CLOSE_RANGE_CLOEXEC: u64 = 1 << 2;
const RISCV_LINUX_CLOSE_RANGE_SUPPORTED_FLAGS: u64 =
    RISCV_LINUX_CLOSE_RANGE_UNSHARE | RISCV_LINUX_CLOSE_RANGE_CLOEXEC;

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

pub(super) fn syscall_close_range(
    first: u64,
    last: u64,
    flags: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    if first > last || first > u64::from(u32::MAX) || last > u64::from(u32::MAX) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & !RISCV_LINUX_CLOSE_RANGE_SUPPORTED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_CLOSE_RANGE_CLOEXEC != 0 {
        state.guest_fds.set_close_on_exec_range(first, last, true);
        return 0;
    }

    for record in state.guest_fds.close_descriptor_range(first, last) {
        state.close_fd_sources(&record);
    }
    0
}

pub(super) fn syscall_dup(old_fd_argument: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(old_fd) = guest_fd_argument(old_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds.entry(old_fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    let new_fd = match state.next_guest_fd_excluding(&[]) {
        Ok(new_fd) => new_fd,
        Err(_) => return linux_error(RISCV_LINUX_EMFILE),
    };
    match state.guest_fds.dup2(old_fd, new_fd) {
        Ok(new_fd) => {
            state.duplicate_fd_source(old_fd, new_fd);
            u64::from(new_fd.get())
        }
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
    if state.guest_fds.entry(old_fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if !state.guest_fd_is_below_open_file_limit(new_fd) {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if state.guest_fds.entry(new_fd).is_none() && !state.has_open_file_capacity(1) {
        return linux_error(RISCV_LINUX_EMFILE);
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
