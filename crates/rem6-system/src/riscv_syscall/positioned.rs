pub(super) fn riscv_linux_split_offset(pos_l: u64, pos_h: u64) -> u64 {
    ((pos_h & u32::MAX as u64) << 32) | (pos_l & u32::MAX as u64)
}
