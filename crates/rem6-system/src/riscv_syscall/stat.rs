use super::{linux_error, RiscvGuestMemoryWriter, RiscvSyscallIdentity, RISCV_LINUX_EFAULT};

const RISCV_LINUX_STAT_BYTES: usize = 128;
const RISCV_LINUX_STAT_BLOCK_BYTES: u64 = 512;
const RISCV_LINUX_STAT_BLOCK_SIZE: u64 = 8192;
const RISCV_LINUX_S_IFCHR: u32 = 0o020000;
const RISCV_LINUX_S_IFREG: u32 = 0o100000;
const RISCV_LINUX_REGULAR_FILE_MODE: u32 = RISCV_LINUX_S_IFREG | 0o444;
const RISCV_LINUX_CHARACTER_DEVICE_MODE: u32 = RISCV_LINUX_S_IFCHR | 0o666;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestStat {
    device: u64,
    inode: u64,
    mode: u32,
    link_count: u32,
    user_id: u32,
    group_id: u32,
    special_device: u64,
    size: u64,
    block_size: u64,
    blocks: u64,
}

impl RiscvGuestStat {
    pub(super) fn regular_file(size: u64, identity: RiscvSyscallIdentity, inode: u64) -> Self {
        Self {
            device: 0,
            inode,
            mode: RISCV_LINUX_REGULAR_FILE_MODE,
            link_count: 1,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0,
            size,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: size.div_ceil(RISCV_LINUX_STAT_BLOCK_BYTES),
        }
    }

    pub(super) fn character_device(identity: RiscvSyscallIdentity, inode: u64) -> Self {
        Self {
            device: 0x0a,
            inode,
            mode: RISCV_LINUX_CHARACTER_DEVICE_MODE,
            link_count: 1,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0x880d,
            size: 0,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: 0,
        }
    }
}

pub(super) fn guest_path_inode(path: &[u8]) -> u64 {
    path.iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
        .max(1)
}

pub(super) fn write_riscv_linux_stat(
    address: u64,
    stat: RiscvGuestStat,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let bytes = riscv_linux_stat_bytes(stat);
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

fn riscv_linux_stat_bytes(stat: RiscvGuestStat) -> [u8; RISCV_LINUX_STAT_BYTES] {
    let mut bytes = [0; RISCV_LINUX_STAT_BYTES];
    write_le_u64(&mut bytes, 0, stat.device);
    write_le_u64(&mut bytes, 8, stat.inode);
    write_le_u32(&mut bytes, 16, stat.mode);
    write_le_u32(&mut bytes, 20, stat.link_count);
    write_le_u32(&mut bytes, 24, stat.user_id);
    write_le_u32(&mut bytes, 28, stat.group_id);
    write_le_u64(&mut bytes, 32, stat.special_device);
    write_le_u64(&mut bytes, 48, stat.size);
    write_le_u64(&mut bytes, 56, stat.block_size);
    write_le_u64(&mut bytes, 64, stat.blocks);
    bytes
}

fn linux_stat_user_id(value: u64) -> u32 {
    value.min(u32::MAX as u64) as u32
}

fn write_le_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
