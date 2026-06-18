use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_CAPGET: u64 = 90;
pub(super) const RISCV_LINUX_CAPSET: u64 = 91;

const RISCV_LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const RISCV_LINUX_CAP_HEADER_BYTES: usize = 8;
const RISCV_LINUX_CAP_DATA_BYTES: usize = 24;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvLinuxCapabilityHeader {
    version: u32,
    pid: i32,
}

pub(super) fn syscall_capget(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let header_address = request.argument(0);
    let data_address = request.argument(1);
    let header = match read_capability_header(header_address, guest_memory_reader) {
        Ok(header) => header,
        Err(error) => return linux_error(error),
    };
    if let Err(error) =
        validate_capability_version(header.version, header_address, guest_memory_writer)
    {
        return linux_error(error);
    }
    if !capget_pid_matches_current_process(header.pid, state) {
        return linux_error(RISCV_LINUX_ESRCH);
    }
    if data_address == 0 {
        return 0;
    }
    let Some(guest_memory_writer) = guest_memory_writer else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if guest_memory_writer.write(data_address, &[0; RISCV_LINUX_CAP_DATA_BYTES]) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

pub(super) fn syscall_capset(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let header_address = request.argument(0);
    let data_address = request.argument(1);
    let header = match read_capability_header(header_address, guest_memory_reader) {
        Ok(header) => header,
        Err(error) => return linux_error(error),
    };
    if let Err(error) =
        validate_capability_version(header.version, header_address, guest_memory_writer)
    {
        return linux_error(error);
    }
    if !capset_pid_matches_current_process(header.pid, state) {
        return linux_error(RISCV_LINUX_EPERM);
    }
    if data_address == 0 {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    let Some(guest_memory_reader) = guest_memory_reader else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let data = match read_guest_exact(
        guest_memory_reader,
        data_address,
        RISCV_LINUX_CAP_DATA_BYTES,
    ) {
        Some(data) => data,
        None => return linux_error(RISCV_LINUX_EFAULT),
    };
    if data.iter().any(|byte| *byte != 0) {
        linux_error(RISCV_LINUX_EPERM)
    } else {
        0
    }
}

fn read_capability_header(
    address: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<RiscvLinuxCapabilityHeader, u64> {
    if address == 0 {
        return Err(RISCV_LINUX_EFAULT);
    }
    let Some(guest_memory_reader) = guest_memory_reader else {
        return Err(RISCV_LINUX_EFAULT);
    };
    let bytes = read_guest_exact(guest_memory_reader, address, RISCV_LINUX_CAP_HEADER_BYTES)
        .ok_or(RISCV_LINUX_EFAULT)?;
    Ok(RiscvLinuxCapabilityHeader {
        version: u32::from_le_bytes(bytes[0..4].try_into().expect("capability version bytes")),
        pid: i32::from_le_bytes(bytes[4..8].try_into().expect("capability pid bytes")),
    })
}

fn validate_capability_version(
    version: u32,
    header_address: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Result<(), u64> {
    if version == RISCV_LINUX_CAPABILITY_VERSION_3 {
        return Ok(());
    }
    let Some(guest_memory_writer) = guest_memory_writer else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if !guest_memory_writer.write(
        header_address,
        &RISCV_LINUX_CAPABILITY_VERSION_3.to_le_bytes(),
    ) {
        return Err(RISCV_LINUX_EFAULT);
    }
    Err(RISCV_LINUX_EINVAL)
}

fn capget_pid_matches_current_process(pid: i32, state: &RiscvSyscallState) -> bool {
    pid == 0 || u64::try_from(pid).ok() == Some(state.identity().thread_group_id())
}

fn capset_pid_matches_current_process(pid: i32, state: &RiscvSyscallState) -> bool {
    pid == 0 || u64::try_from(pid).ok() == Some(state.identity().thread_group_id())
}

fn read_guest_exact(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
    bytes: usize,
) -> Option<Vec<u8>> {
    guest_memory_reader
        .read(address, bytes)
        .filter(|read| read.len() == bytes)
}
