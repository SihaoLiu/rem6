use crate::{GuestFd, GuestFileDescriptionId};

use super::stat::RISCV_LINUX_MODE_BITS;
use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestNodeKind, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_AT_FDCWD, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR, RISCV_LINUX_PATH_MAX,
};

const RISCV_LINUX_MODE_PERMISSION_BITS: u32 = 0o777;
pub(super) const RISCV_LINUX_FCHMOD: u64 = 52;
pub(super) const RISCV_LINUX_FCHMODAT: u64 = 53;
pub(super) const RISCV_LINUX_UMASK: u64 = 166;
pub(super) const RISCV_NEWLIB_LEGACY_CHMOD: u64 = 1028;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestChmodError {
    Missing,
    BadFd,
}

impl RiscvGuestChmodError {
    const fn linux_error_code(self) -> u64 {
        match self {
            Self::Missing => RISCV_LINUX_ENOENT,
            Self::BadFd => RISCV_LINUX_EBADF,
        }
    }
}

pub(super) fn syscall_chmod(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_chmod_path(request.argument(0), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let path = match resolve_chmod_path(RISCV_LINUX_AT_FDCWD, &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    chmod_path(&path, request.argument(1), state)
}

pub(super) fn syscall_fchmod(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.chmod_guest_fd(fd, chmod_permissions(request.argument(1))) {
        Ok(()) => 0,
        Err(error) => linux_error(error.linux_error_code()),
    }
}

pub(super) fn syscall_fchmodat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_chmod_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let path = match resolve_chmod_path(request.argument(0), &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    chmod_path(&path, request.argument(2), state)
}

pub(super) fn syscall_umask(mask: u64, state: &mut RiscvSyscallState) -> u64 {
    let next_mask = (mask as u32) & RISCV_LINUX_MODE_PERMISSION_BITS;
    u64::from(state.replace_file_creation_mask(next_mask))
}

pub(super) fn apply_file_creation_mask(mode: u64, state: &RiscvSyscallState) -> u32 {
    (mode as u32) & RISCV_LINUX_MODE_PERMISSION_BITS & !state.file_creation_mask()
}

fn read_chmod_path(
    path_address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<Vec<u8>, u64> {
    read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn resolve_chmod_path(dirfd: u64, path: &[u8], state: &RiscvSyscallState) -> Result<Vec<u8>, u64> {
    if path.is_empty() {
        return Err(linux_error(RISCV_LINUX_ENOENT));
    }
    if dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/") {
        return state
            .resolve_guest_path(path)
            .map_err(|error| linux_error(error.linux_error_code()));
    }
    let Some(fd) = guest_fd_argument(dirfd) else {
        return Err(linux_error(RISCV_LINUX_EBADF));
    };
    let directory = match state.guest_directory_path_for_fd(fd) {
        Ok(Some(path)) => path,
        Ok(None) => return Err(linux_error(RISCV_LINUX_ENOTDIR)),
        Err(_error) => return Err(linux_error(RISCV_LINUX_EBADF)),
    };
    state
        .resolve_guest_path_from_directory(&directory, path)
        .map_err(|error| linux_error(error.linux_error_code()))
}

fn chmod_path(path: &[u8], mode: u64, state: &mut RiscvSyscallState) -> u64 {
    match state.chmod_guest_path(path, chmod_permissions(mode)) {
        Ok(()) => 0,
        Err(error) => linux_error(error.linux_error_code()),
    }
}

fn chmod_permissions(mode: u64) -> u32 {
    (mode as u32) & RISCV_LINUX_MODE_BITS
}

impl RiscvSyscallState {
    fn chmod_guest_path(
        &mut self,
        path: &[u8],
        permissions: u32,
    ) -> Result<(), RiscvGuestChmodError> {
        let existing_path = self.existing_guest_path_key(path);
        if let Some(path) = existing_path
            .as_deref()
            .filter(|path| self.guest_path_registered(path))
        {
            let identity = self.guest_file_identity(path);
            self.update_guest_file_permissions(identity, permissions);
            return Ok(());
        }

        let directory_path = existing_path.as_deref().unwrap_or(path);
        if self.guest_directory_entries(directory_path).is_some() {
            self.update_guest_directory_permissions(directory_path, permissions);
            return Ok(());
        }
        Err(RiscvGuestChmodError::Missing)
    }

    fn chmod_guest_fd(
        &mut self,
        fd: GuestFd,
        permissions: u32,
    ) -> Result<(), RiscvGuestChmodError> {
        let description = self
            .guest_fds
            .description_for_fd(fd)
            .map_err(|_error| RiscvGuestChmodError::BadFd)?
            .id();
        let stat = self
            .guest_file_stats
            .get(&description)
            .copied()
            .ok_or(RiscvGuestChmodError::BadFd)?;
        match stat.kind {
            RiscvGuestNodeKind::Directory => {
                let path = self
                    .guest_directory_paths
                    .get(&description)
                    .cloned()
                    .ok_or(RiscvGuestChmodError::BadFd)?;
                self.update_guest_directory_permissions(&path, permissions);
            }
            RiscvGuestNodeKind::RegularFile | RiscvGuestNodeKind::Symlink => {
                self.update_guest_file_permissions(stat.identity, permissions);
            }
        }
        Ok(())
    }

    fn update_guest_file_permissions(
        &mut self,
        identity: super::RiscvGuestFileIdentity,
        permissions: u32,
    ) {
        let permissions = permissions & RISCV_LINUX_MODE_BITS;
        self.guest_file_modes.insert(identity, permissions);
        for stat in self.guest_file_stats.values_mut() {
            if stat.identity == identity && stat.kind != RiscvGuestNodeKind::Directory {
                stat.permissions = permissions;
            }
        }
    }

    fn update_guest_directory_permissions(&mut self, path: &[u8], permissions: u32) {
        let permissions = permissions & RISCV_LINUX_MODE_BITS;
        self.guest_directory_modes
            .insert(path.to_vec(), permissions);
        let descriptions = matching_directory_descriptions(&self.guest_directory_paths, path);
        for description in descriptions {
            if let Some(stat) = self.guest_file_stats.get_mut(&description) {
                stat.permissions = permissions;
            }
        }
    }
}

fn matching_directory_descriptions(
    paths: &std::collections::BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    path: &[u8],
) -> Vec<GuestFileDescriptionId> {
    paths
        .iter()
        .filter_map(|(description, candidate)| {
            (candidate.as_slice() == path).then_some(*description)
        })
        .collect()
}
