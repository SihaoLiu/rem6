use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_GETRANDOM: u64 = 278;

const RISCV_LINUX_GRND_NONBLOCK: u64 = 0x0001;
const RISCV_LINUX_GRND_RANDOM: u64 = 0x0002;
const RISCV_LINUX_GRND_INSECURE: u64 = 0x0004;
const RISCV_LINUX_GRND_VALID_FLAGS: u64 =
    RISCV_LINUX_GRND_NONBLOCK | RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE;
const RISCV_LINUX_GETRANDOM_MAX_CHUNK_BYTES: u64 = 256;
const RISCV_LINUX_GETRANDOM_INITIAL_BYTE: u8 = 0x2b;

pub(super) fn invalid_getrandom_flags(flags: u64) -> bool {
    flags & !RISCV_LINUX_GRND_VALID_FLAGS != 0
        || flags & (RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE)
            == (RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE)
}

pub(super) fn syscall_getrandom(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let count = request
        .argument(1)
        .min(RISCV_LINUX_GETRANDOM_MAX_CHUNK_BYTES);
    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let bytes = state.getrandom_bytes(byte_count);
    if !guest_memory.write(request.argument(0), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    state.advance_getrandom_byte_counter(byte_count);
    count
}

impl RiscvSyscallState {
    fn getrandom_bytes(&self, count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(count);
        let mut counter = self.getrandom_byte_counter;
        for _ in 0..count {
            bytes.push(RISCV_LINUX_GETRANDOM_INITIAL_BYTE ^ counter);
            counter = counter.wrapping_add(1);
        }
        bytes
    }

    fn advance_getrandom_byte_counter(&mut self, count: usize) {
        self.getrandom_byte_counter = self.getrandom_byte_counter.wrapping_add(count as u8);
    }
}
