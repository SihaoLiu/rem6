use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENOTTY,
};
use crate::{GuestFd, GuestFdError};

pub(super) const RISCV_LINUX_IOCTL: u64 = 29;
const RISCV_LINUX_TIOCGWINSZ: u64 = 0x5413;
const RISCV_LINUX_FIONREAD: u64 = 0x541b;
const RISCV_LINUX_WINSIZE_ROWS: u16 = 24;
const RISCV_LINUX_WINSIZE_COLUMNS: u16 = 80;

impl RiscvSyscallState {
    fn guest_fd_is_terminal(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self.guest_terminal_descriptions.contains(&description))
    }
}

pub(super) fn syscall_ioctl(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    match request.argument(1) {
        RISCV_LINUX_TIOCGWINSZ => {
            match state.guest_fd_is_terminal(fd) {
                Ok(true) => {}
                Ok(false) => return linux_error(RISCV_LINUX_ENOTTY),
                Err(_) => return linux_error(RISCV_LINUX_EBADF),
            }
            let Some(guest_memory_writer) = guest_memory_writer else {
                return linux_error(RISCV_LINUX_EFAULT);
            };
            if guest_memory_writer.write(request.argument(2), &linux_winsize_bytes()) {
                0
            } else {
                linux_error(RISCV_LINUX_EFAULT)
            }
        }
        RISCV_LINUX_FIONREAD => {
            let count = match state.guest_pipe_unread_byte_count(fd) {
                Ok(Some(count)) => count,
                Ok(None) => return linux_error(RISCV_LINUX_ENOTTY),
                Err(_) => return linux_error(RISCV_LINUX_EBADF),
            };
            let Ok(count) = i32::try_from(count) else {
                return linux_error(RISCV_LINUX_EINVAL);
            };
            let Some(guest_memory_writer) = guest_memory_writer else {
                return linux_error(RISCV_LINUX_EFAULT);
            };
            if guest_memory_writer.write(request.argument(2), &count.to_le_bytes()) {
                0
            } else {
                linux_error(RISCV_LINUX_EFAULT)
            }
        }
        _ => linux_error(RISCV_LINUX_ENOTTY),
    }
}

fn linux_winsize_bytes() -> [u8; 8] {
    let mut bytes = [0; 8];
    bytes[0..2].copy_from_slice(&RISCV_LINUX_WINSIZE_ROWS.to_le_bytes());
    bytes[2..4].copy_from_slice(&RISCV_LINUX_WINSIZE_COLUMNS.to_le_bytes());
    bytes
}
