use super::{
    fcntl::RISCV_LINUX_F_SEAL_SEAL, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestNodeKind, RiscvOpenGuestFileStat, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE,
    RISCV_LINUX_O_RDWR,
};
use crate::{GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileStatusFlags};

pub(super) const RISCV_LINUX_MEMFD_CREATE: u64 = 279;

const RISCV_LINUX_MEMFD_NAME_LIMIT: usize = 250;
const RISCV_LINUX_MFD_CLOEXEC: u32 = 0x0001;
const RISCV_LINUX_MFD_ALLOW_SEALING: u32 = 0x0002;
const RISCV_LINUX_MFD_SUPPORTED_FLAGS: u32 =
    RISCV_LINUX_MFD_CLOEXEC | RISCV_LINUX_MFD_ALLOW_SEALING;
const RISCV_LINUX_MEMFD_PERMISSIONS: u32 = 0o777;

pub(super) fn syscall_memfd_create(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let flags = request.argument(1) as u32;
    if flags & !RISCV_LINUX_MFD_SUPPORTED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match read_guest_c_string(
        guest_memory,
        request.argument(0),
        RISCV_LINUX_MEMFD_NAME_LIMIT,
    ) {
        Ok(_name) => {}
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_EINVAL),
    }

    match state.open_memfd(
        flags & RISCV_LINUX_MFD_CLOEXEC != 0,
        flags & RISCV_LINUX_MFD_ALLOW_SEALING != 0,
    ) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EINVAL),
    }
}

impl RiscvSyscallState {
    fn open_memfd(
        &mut self,
        close_on_exec: bool,
        allow_sealing: bool,
    ) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        let identity = self.allocate_guest_file_identity();
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(RISCV_LINUX_O_RDWR as u32),
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_file_descriptions.insert(description, Vec::new());
        self.guest_file_stats.insert(
            description,
            RiscvOpenGuestFileStat {
                identity,
                size: 0,
                kind: RiscvGuestNodeKind::RegularFile,
                permissions: RISCV_LINUX_MEMFD_PERMISSIONS,
            },
        );
        self.guest_file_seals.insert(
            description,
            if allow_sealing {
                0
            } else {
                RISCV_LINUX_F_SEAL_SEAL
            },
        );
        Ok(fd)
    }
}
