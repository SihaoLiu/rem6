use super::{linux_error, RiscvGuestMemoryWriter, RISCV_LINUX_EFAULT};

const RISCV_LINUX_UTS_FIELD_BYTES: usize = 65;
const RISCV_LINUX_NEW_UTS_FIELDS: usize = 6;
const RISCV_LINUX_UTS_BYTES: usize = RISCV_LINUX_UTS_FIELD_BYTES * RISCV_LINUX_NEW_UTS_FIELDS;
const RISCV_LINUX_UTS_SYSNAME: &[u8] = b"Linux";
const RISCV_LINUX_UTS_NODENAME: &[u8] = b"sim.gem5.org";
const RISCV_LINUX_UTS_RELEASE: &[u8] = b"5.1.0";
const RISCV_LINUX_UTS_VERSION: &[u8] = b"#1 Mon Aug 18 11:32:15 EDT 2003";
const RISCV_LINUX_UTS_MACHINE: &[u8] = b"riscv64";

pub(super) fn write_riscv_linux_utsname(
    address: u64,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let bytes = riscv_linux_utsname_bytes();
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

fn riscv_linux_utsname_bytes() -> [u8; RISCV_LINUX_UTS_BYTES] {
    let mut bytes = [0; RISCV_LINUX_UTS_BYTES];
    write_uts_field(&mut bytes, 0, RISCV_LINUX_UTS_SYSNAME);
    write_uts_field(&mut bytes, 1, RISCV_LINUX_UTS_NODENAME);
    write_uts_field(&mut bytes, 2, RISCV_LINUX_UTS_RELEASE);
    write_uts_field(&mut bytes, 3, RISCV_LINUX_UTS_VERSION);
    write_uts_field(&mut bytes, 4, RISCV_LINUX_UTS_MACHINE);
    bytes
}

fn write_uts_field(output: &mut [u8], index: usize, value: &[u8]) {
    debug_assert!(value.len() < RISCV_LINUX_UTS_FIELD_BYTES);
    let start = index * RISCV_LINUX_UTS_FIELD_BYTES;
    output[start..start + value.len()].copy_from_slice(value);
}
