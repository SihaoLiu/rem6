use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_SETNS: u64 = 268;

const RISCV_LINUX_CLONE_NEWTIME: u64 = 0x0000_0080;
const RISCV_LINUX_CLONE_NEWNS: u64 = 0x0002_0000;
const RISCV_LINUX_CLONE_NEWCGROUP: u64 = 0x0200_0000;
const RISCV_LINUX_CLONE_NEWUTS: u64 = 0x0400_0000;
const RISCV_LINUX_CLONE_NEWIPC: u64 = 0x0800_0000;
const RISCV_LINUX_CLONE_NEWUSER: u64 = 0x1000_0000;
const RISCV_LINUX_CLONE_NEWPID: u64 = 0x2000_0000;
const RISCV_LINUX_CLONE_NEWNET: u64 = 0x4000_0000;
const RISCV_LINUX_SETNS_NAMESPACE_TYPES: u64 = RISCV_LINUX_CLONE_NEWTIME
    | RISCV_LINUX_CLONE_NEWNS
    | RISCV_LINUX_CLONE_NEWCGROUP
    | RISCV_LINUX_CLONE_NEWUTS
    | RISCV_LINUX_CLONE_NEWIPC
    | RISCV_LINUX_CLONE_NEWUSER
    | RISCV_LINUX_CLONE_NEWPID
    | RISCV_LINUX_CLONE_NEWNET;

pub(super) fn syscall_setns(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let namespace_type = request.argument(1);
    if namespace_type & !RISCV_LINUX_SETNS_NAMESPACE_TYPES != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    linux_error(RISCV_LINUX_EINVAL)
}
