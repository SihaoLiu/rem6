use super::{linux_error, RiscvSyscallRequest, RISCV_LINUX_EINVAL};

pub(super) const RISCV_LINUX_RISCV_FLUSH_ICACHE: u64 = 259;

const SYS_RISCV_FLUSH_ICACHE_LOCAL: u64 = 1;
const SYS_RISCV_FLUSH_ICACHE_ALL: u64 = SYS_RISCV_FLUSH_ICACHE_LOCAL;

pub(super) fn syscall_riscv_flush_icache(request: RiscvSyscallRequest) -> u64 {
    if request.argument(2) & !SYS_RISCV_FLUSH_ICACHE_ALL != 0 {
        linux_error(RISCV_LINUX_EINVAL)
    } else {
        0
    }
}
