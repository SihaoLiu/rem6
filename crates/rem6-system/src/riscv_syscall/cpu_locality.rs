use super::{linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RISCV_LINUX_EFAULT};

pub(super) const RISCV_LINUX_GETCPU: u64 = 168;

const RISCV_LINUX_GUEST_CPU_ID: u32 = 0;
const RISCV_LINUX_GUEST_NUMA_NODE: u32 = 0;

pub(super) fn syscall_getcpu(
    request: RiscvSyscallRequest,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let cpu_address = request.argument(0);
    let node_address = request.argument(1);
    if cpu_address == 0 && node_address == 0 {
        return Some(0);
    }

    let guest_memory_writer = guest_memory_writer?;
    let mut wrote_all = true;
    if cpu_address != 0 {
        wrote_all &=
            guest_memory_writer.write(cpu_address, &RISCV_LINUX_GUEST_CPU_ID.to_le_bytes());
    }
    if node_address != 0 {
        wrote_all &=
            guest_memory_writer.write(node_address, &RISCV_LINUX_GUEST_NUMA_NODE.to_le_bytes());
    }

    if wrote_all {
        Some(0)
    } else {
        Some(linux_error(RISCV_LINUX_EFAULT))
    }
}
