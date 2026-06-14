use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_RISCV_HWPROBE: u64 = 258;

const RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES: usize = 16;
const RISCV_LINUX_RISCV_HWPROBE_MAX_PAIRS: u64 = 1024;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID: i64 = 0;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MARCHID: i64 = 1;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MIMPID: i64 = 2;
const RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR: i64 = 3;
const RISCV_LINUX_RISCV_HWPROBE_KEY_IMA_EXT_0: i64 = 4;
const RISCV_LINUX_RISCV_HWPROBE_KEY_CPUPERF_0: i64 = 5;
const RISCV_LINUX_RISCV_HWPROBE_BASE_BEHAVIOR_IMA: u64 = 1;
const RISCV_LINUX_RISCV_HWPROBE_IMA_EXT_0_FD: u64 = 1 << 0;
const RISCV_LINUX_RISCV_HWPROBE_IMA_EXT_0_C: u64 = 1 << 1;
const RISCV_LINUX_RISCV_HWPROBE_CPU_PERF_SLOW: u64 = 2;
const RISCV_LINUX_RISCV_HWPROBE_ONLINE_CPU_MASK: u8 = 1;

pub(super) fn syscall_riscv_hwprobe(
    request: RiscvSyscallRequest,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    if request.argument(4) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if let Err(error) = validate_riscv_hwprobe_cpu_mask(request, guest_memory_reader) {
        return error;
    }
    let pair_count = request.argument(1);
    if pair_count == 0 {
        return 0;
    }
    if pair_count > RISCV_LINUX_RISCV_HWPROBE_MAX_PAIRS {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(total_bytes) = usize::try_from(pair_count)
        .ok()
        .and_then(|count| count.checked_mul(RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES))
    else {
        return linux_error(RISCV_LINUX_EINVAL);
    };

    let Some(guest_memory_reader) = guest_memory_reader else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let Some(guest_memory_writer) = guest_memory_writer else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let Some(mut bytes) = guest_memory_reader
        .read(request.argument(0), total_bytes)
        .filter(|bytes| bytes.len() == total_bytes)
    else {
        return linux_error(RISCV_LINUX_EFAULT);
    };

    for pair in bytes.chunks_exact_mut(RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES) {
        let key = i64::from_le_bytes(pair[0..8].try_into().expect("hwprobe key bytes"));
        let (output_key, output_value) = riscv_hwprobe_value(key);
        pair[0..8].copy_from_slice(&output_key.to_le_bytes());
        pair[8..16].copy_from_slice(&output_value.to_le_bytes());
    }

    if guest_memory_writer.write(request.argument(0), &bytes) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

fn validate_riscv_hwprobe_cpu_mask(
    request: RiscvSyscallRequest,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<(), u64> {
    let cpusetsize = request.argument(2);
    let cpus_address = request.argument(3);
    if cpusetsize == 0 && cpus_address == 0 {
        return Ok(());
    }
    if cpusetsize == 0 {
        return Err(linux_error(RISCV_LINUX_EINVAL));
    }
    let Some(guest_memory_reader) = guest_memory_reader else {
        return Err(linux_error(RISCV_LINUX_EFAULT));
    };
    let Ok(cpusetsize) = usize::try_from(cpusetsize) else {
        return Err(linux_error(RISCV_LINUX_EINVAL));
    };
    let Some(cpu_mask) = guest_memory_reader
        .read(cpus_address, cpusetsize)
        .filter(|bytes| bytes.len() == cpusetsize)
    else {
        return Err(linux_error(RISCV_LINUX_EFAULT));
    };
    if cpu_mask
        .first()
        .is_some_and(|first| first & RISCV_LINUX_RISCV_HWPROBE_ONLINE_CPU_MASK != 0)
    {
        Ok(())
    } else {
        Err(linux_error(RISCV_LINUX_EINVAL))
    }
}

fn riscv_hwprobe_value(key: i64) -> (i64, u64) {
    match key {
        RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID
        | RISCV_LINUX_RISCV_HWPROBE_KEY_MARCHID
        | RISCV_LINUX_RISCV_HWPROBE_KEY_MIMPID => (key, 0),
        RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR => {
            (key, RISCV_LINUX_RISCV_HWPROBE_BASE_BEHAVIOR_IMA)
        }
        RISCV_LINUX_RISCV_HWPROBE_KEY_IMA_EXT_0 => (
            key,
            RISCV_LINUX_RISCV_HWPROBE_IMA_EXT_0_FD | RISCV_LINUX_RISCV_HWPROBE_IMA_EXT_0_C,
        ),
        RISCV_LINUX_RISCV_HWPROBE_KEY_CPUPERF_0 => (key, RISCV_LINUX_RISCV_HWPROBE_CPU_PERF_SLOW),
        _ => (-1, 0),
    }
}
