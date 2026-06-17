use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBUSY,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
};

pub(super) const RISCV_LINUX_SET_TID_ADDRESS: u64 = 96;
pub(super) const RISCV_LINUX_MEMBARRIER: u64 = 283;
pub(super) const RISCV_LINUX_RSEQ: u64 = 293;

const RISCV_LINUX_MEMBARRIER_CMD_QUERY: u64 = 0;
const RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED: u64 = 1 << 3;
const RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED: u64 = 1 << 4;
const RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS: u64 = 1 << 9;
const RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS: u64 = RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED
    | RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED
    | RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS;
const RISCV_LINUX_RSEQ_SIZE: u64 = 32;
const RISCV_LINUX_RSEQ_FLAG_UNREGISTER: u64 = 1;
const RISCV_LINUX_RSEQ_VALID_FLAGS: u64 = RISCV_LINUX_RSEQ_FLAG_UNREGISTER;
const RISCV_LINUX_RSEQ_GUEST_CPU_ID: u32 = 0;
const RISCV_LINUX_RSEQ_CPU_ID_UNINITIALIZED: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvSyscallRseqRegistration {
    address: u64,
    length: u32,
    signature: u32,
}

impl RiscvSyscallRseqRegistration {
    const fn new(address: u64, length: u32, signature: u32) -> Self {
        Self {
            address,
            length,
            signature,
        }
    }

    const fn same_area(self, other: Self) -> bool {
        self.address == other.address && self.length == other.length
    }
}

impl RiscvSyscallState {
    pub(super) const fn membarrier_registrations(&self) -> u64 {
        self.membarrier_registrations
    }

    pub(super) fn register_membarrier_command(&mut self, command: u64) {
        self.membarrier_registrations |= command;
    }

    pub(super) const fn rseq_registration(&self) -> Option<RiscvSyscallRseqRegistration> {
        self.rseq_registration
    }

    pub(super) fn set_rseq_registration(
        &mut self,
        registration: Option<RiscvSyscallRseqRegistration>,
    ) {
        self.rseq_registration = registration;
    }
}

pub(super) fn syscall_thread(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    match request.number() {
        RISCV_LINUX_SET_TID_ADDRESS => Some(syscall_set_tid_address(request.argument(0), state)),
        RISCV_LINUX_MEMBARRIER => Some(syscall_membarrier(request, state)),
        RISCV_LINUX_RSEQ => syscall_rseq(request, state, guest_memory_writer),
        _ => unreachable!("RISC-V Linux thread syscall is handled by caller"),
    }
}

pub(super) fn syscall_set_tid_address(
    clear_tid_address: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    state.set_child_clear_tid(clear_tid_address);
    state.identity().thread_id()
}

fn syscall_membarrier(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let command = request.argument(0);
    let flags = request.argument(1);
    if flags != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    match command {
        RISCV_LINUX_MEMBARRIER_CMD_QUERY => RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS,
        RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED => {
            state.register_membarrier_command(command);
            0
        }
        RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED => {
            if state.membarrier_registrations()
                & RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED
                == 0
            {
                linux_error(RISCV_LINUX_EPERM)
            } else {
                0
            }
        }
        RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS => state.membarrier_registrations(),
        _ => linux_error(RISCV_LINUX_EINVAL),
    }
}

fn syscall_rseq(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let address = request.argument(0);
    let length = request.argument(1);
    let flags = request.argument(2);
    let signature = request.argument(3);

    if address == 0
        || length != RISCV_LINUX_RSEQ_SIZE
        || !address.is_multiple_of(RISCV_LINUX_RSEQ_SIZE)
        || flags & !RISCV_LINUX_RSEQ_VALID_FLAGS != 0
    {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let registration = RiscvSyscallRseqRegistration::new(address, length as u32, signature as u32);
    if flags & RISCV_LINUX_RSEQ_FLAG_UNREGISTER != 0 {
        return syscall_rseq_unregister(registration, state, guest_memory_writer);
    }

    if let Some(current) = state.rseq_registration() {
        if !current.same_area(registration) {
            return Some(linux_error(RISCV_LINUX_EINVAL));
        }
        if current.signature != registration.signature {
            return Some(linux_error(RISCV_LINUX_EPERM));
        }
        return Some(linux_error(RISCV_LINUX_EBUSY));
    }

    let guest_memory_writer = guest_memory_writer?;
    if !write_rseq_registration(address, guest_memory_writer) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    state.set_rseq_registration(Some(registration));
    Some(0)
}

fn syscall_rseq_unregister(
    registration: RiscvSyscallRseqRegistration,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(current) = state.rseq_registration() else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    if !current.same_area(registration) {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if current.signature != registration.signature {
        return Some(linux_error(RISCV_LINUX_EPERM));
    }

    let guest_memory_writer = guest_memory_writer?;
    if !write_rseq_unregistration(registration.address, guest_memory_writer) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    state.set_rseq_registration(None);
    Some(0)
}

fn write_rseq_registration(address: u64, guest_memory_writer: &RiscvGuestMemoryWriter) -> bool {
    let mut bytes = [0u8; RISCV_LINUX_RSEQ_SIZE as usize];
    bytes[0..4].copy_from_slice(&RISCV_LINUX_RSEQ_GUEST_CPU_ID.to_le_bytes());
    bytes[4..8].copy_from_slice(&RISCV_LINUX_RSEQ_GUEST_CPU_ID.to_le_bytes());
    guest_memory_writer.write(address, &bytes)
}

fn write_rseq_unregistration(address: u64, guest_memory_writer: &RiscvGuestMemoryWriter) -> bool {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&RISCV_LINUX_RSEQ_GUEST_CPU_ID.to_le_bytes());
    bytes[4..8].copy_from_slice(&RISCV_LINUX_RSEQ_CPU_ID_UNINITIALIZED.to_le_bytes());
    guest_memory_writer.write(address, &bytes)
}
