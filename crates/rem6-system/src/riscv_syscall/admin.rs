use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvSyscallRequest, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_EPERM, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_UMOUNT2: u64 = 39;
pub(super) const RISCV_LINUX_MOUNT: u64 = 40;
pub(super) const RISCV_LINUX_PIVOT_ROOT: u64 = 41;
pub(super) const RISCV_LINUX_ACCT: u64 = 89;
pub(super) const RISCV_LINUX_REBOOT: u64 = 142;
pub(super) const RISCV_LINUX_SETHOSTNAME: u64 = 161;
pub(super) const RISCV_LINUX_SETDOMAINNAME: u64 = 162;
pub(super) const RISCV_LINUX_SWAPON: u64 = 224;
pub(super) const RISCV_LINUX_SWAPOFF: u64 = 225;

const RISCV_LINUX_MNT_FORCE: u64 = 1;
const RISCV_LINUX_MNT_DETACH: u64 = 2;
const RISCV_LINUX_MNT_EXPIRE: u64 = 4;
const RISCV_LINUX_UMOUNT_NOFOLLOW: u64 = 8;
const RISCV_LINUX_VALID_UMOUNT_FLAGS: u64 = RISCV_LINUX_MNT_FORCE
    | RISCV_LINUX_MNT_DETACH
    | RISCV_LINUX_MNT_EXPIRE
    | RISCV_LINUX_UMOUNT_NOFOLLOW;

const RISCV_LINUX_SWAP_FLAG_PRIO_MASK: u64 = 0x7fff;
const RISCV_LINUX_SWAP_FLAG_PREFER: u64 = 0x8000;
const RISCV_LINUX_SWAP_FLAG_DISCARD: u64 = 0x1_0000;
const RISCV_LINUX_SWAP_FLAG_DISCARD_ONCE: u64 = 0x2_0000;
const RISCV_LINUX_SWAP_FLAG_DISCARD_PAGES: u64 = 0x4_0000;
const RISCV_LINUX_VALID_SWAPON_FLAGS: u64 = RISCV_LINUX_SWAP_FLAG_PRIO_MASK
    | RISCV_LINUX_SWAP_FLAG_PREFER
    | RISCV_LINUX_SWAP_FLAG_DISCARD
    | RISCV_LINUX_SWAP_FLAG_DISCARD_ONCE
    | RISCV_LINUX_SWAP_FLAG_DISCARD_PAGES;

pub(super) fn syscall_umount2(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if let Err(errno) = validate_umount_flags(request.argument(1)) {
        return linux_error(errno);
    }
    deny_after_guest_path(request.argument(0), guest_memory)
}

pub(super) fn syscall_mount(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if let Err(errno) = read_guest_path(request.argument(0), guest_memory) {
        return linux_error(errno);
    }
    if let Err(errno) = read_guest_path(request.argument(1), guest_memory) {
        return linux_error(errno);
    }
    deny_after_optional_guest_c_string(request.argument(2), guest_memory)
}

pub(super) fn syscall_pivot_root(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if let Err(errno) = read_guest_path(request.argument(0), guest_memory) {
        return linux_error(errno);
    }
    deny_after_guest_path(request.argument(1), guest_memory)
}

pub(super) fn syscall_acct(
    request: RiscvSyscallRequest,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let filename = request.argument(0);
    if filename != 0 {
        let guest_memory = guest_memory?;
        if let Err(errno) = read_guest_path(filename, guest_memory) {
            return Some(linux_error(errno));
        }
    }
    Some(linux_error(RISCV_LINUX_EPERM))
}

pub(super) fn syscall_reboot() -> u64 {
    linux_error(RISCV_LINUX_EPERM)
}

pub(super) fn syscall_sethostname(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    deny_after_guest_bytes(request.argument(0), request.argument(1), guest_memory)
}

pub(super) fn syscall_setdomainname(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    deny_after_guest_bytes(request.argument(0), request.argument(1), guest_memory)
}

pub(super) fn syscall_swapon(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if request.argument(1) & !RISCV_LINUX_VALID_SWAPON_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    deny_after_guest_path(request.argument(0), guest_memory)
}

pub(super) fn syscall_swapoff(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    deny_after_guest_path(request.argument(0), guest_memory)
}

fn deny_after_guest_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> u64 {
    match read_guest_path(address, guest_memory) {
        Ok(()) => linux_error(RISCV_LINUX_EPERM),
        Err(errno) => linux_error(errno),
    }
}

fn deny_after_optional_guest_c_string(address: u64, guest_memory: &RiscvGuestMemoryReader) -> u64 {
    if address == 0 {
        return linux_error(RISCV_LINUX_EPERM);
    }
    deny_after_guest_path(address, guest_memory)
}

fn read_guest_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<(), u64> {
    match read_guest_c_string(guest_memory, address, RISCV_LINUX_PATH_MAX) {
        Ok(_) => Ok(()),
        Err(RiscvGuestCStringError::Fault) => Err(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => Err(RISCV_LINUX_ENAMETOOLONG),
    }
}

fn deny_after_guest_bytes(address: u64, len: u64, guest_memory: &RiscvGuestMemoryReader) -> u64 {
    if address == 0 {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if len == 0 {
        return linux_error(RISCV_LINUX_EPERM);
    }
    let Ok(len) = usize::try_from(len) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    match guest_memory.read(address, len) {
        Some(bytes) if bytes.len() == len => linux_error(RISCV_LINUX_EPERM),
        _ => linux_error(RISCV_LINUX_EFAULT),
    }
}

fn validate_umount_flags(flags: u64) -> Result<(), u64> {
    if flags & !RISCV_LINUX_VALID_UMOUNT_FLAGS != 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_MNT_EXPIRE != 0
        && flags & (RISCV_LINUX_MNT_FORCE | RISCV_LINUX_MNT_DETACH) != 0
    {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(())
}
