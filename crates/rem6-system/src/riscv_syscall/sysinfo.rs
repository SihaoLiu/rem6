use rem6_kernel::Tick;

use super::{linux_error, RiscvGuestMemoryWriter, RISCV_LINUX_EFAULT};

pub(super) const RISCV_LINUX_SYSINFO: u64 = 179;

const RISCV_LINUX_SYSINFO_BYTES: usize = 112;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const RISCV_LINUX_GUEST_PROCESSES: u16 = 1;
const RISCV_LINUX_SYSINFO_MEM_UNIT: u32 = 1;

pub(super) fn syscall_sysinfo(
    address: u64,
    tick: Tick,
    memory_capacity_bytes: u64,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let bytes = riscv_linux_sysinfo_bytes(tick, memory_capacity_bytes);
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(byte_address) = address.checked_add(offset as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory.write(byte_address, std::slice::from_ref(byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
}

fn riscv_linux_sysinfo_bytes(
    tick: Tick,
    memory_capacity_bytes: u64,
) -> [u8; RISCV_LINUX_SYSINFO_BYTES] {
    let mut bytes = [0; RISCV_LINUX_SYSINFO_BYTES];
    write_le_u64(&mut bytes, 0, tick / RISCV_LINUX_NANOSECONDS_PER_SECOND);
    write_le_u64(&mut bytes, 32, memory_capacity_bytes);
    write_le_u64(&mut bytes, 40, memory_capacity_bytes);
    write_le_u16(&mut bytes, 80, RISCV_LINUX_GUEST_PROCESSES);
    write_le_u32(&mut bytes, 104, RISCV_LINUX_SYSINFO_MEM_UNIT);
    bytes
}

fn write_le_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
