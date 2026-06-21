pub(super) const RISCV_LINUX_SPLICE_F_NONBLOCK: u64 = 0x02;

const RISCV_LINUX_SPLICE_F_MOVE: u64 = 0x01;
const RISCV_LINUX_SPLICE_F_MORE: u64 = 0x04;
const RISCV_LINUX_SPLICE_F_GIFT: u64 = 0x08;
const RISCV_LINUX_SPLICE_SUPPORTED_FLAGS: u64 = RISCV_LINUX_SPLICE_F_MOVE
    | RISCV_LINUX_SPLICE_F_NONBLOCK
    | RISCV_LINUX_SPLICE_F_MORE
    | RISCV_LINUX_SPLICE_F_GIFT;

pub(super) const fn splice_flags_are_supported(flags: u64) -> bool {
    flags & !RISCV_LINUX_SPLICE_SUPPORTED_FLAGS == 0
}

pub(super) const fn splice_flags_are_nonblocking(flags: u64) -> bool {
    flags & RISCV_LINUX_SPLICE_F_NONBLOCK != 0
}
