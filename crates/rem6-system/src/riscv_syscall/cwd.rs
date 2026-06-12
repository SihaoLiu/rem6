use crate::{GuestFd, GuestFdError};

use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT,
    RISCV_LINUX_ENOTDIR, RISCV_LINUX_ERANGE, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_CHDIR: u64 = 49;
pub(super) const RISCV_LINUX_FCHDIR: u64 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestPathResolutionError {
    Missing,
    NotDirectory,
}

impl RiscvGuestPathResolutionError {
    pub(super) const fn linux_error_code(self) -> u64 {
        match self {
            Self::Missing => RISCV_LINUX_ENOENT,
            Self::NotDirectory => RISCV_LINUX_ENOTDIR,
        }
    }
}

pub(super) fn syscall_getcwd(
    address: u64,
    size: u64,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let cwd = state.current_directory();
    if cwd.len() as u64 >= size {
        return linux_error(RISCV_LINUX_ERANGE);
    }

    for offset in 0..size {
        let Some(byte_address) = address.checked_add(offset) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let byte = usize::try_from(offset)
            .ok()
            .and_then(|index| cwd.get(index))
            .copied()
            .unwrap_or(0);
        if !guest_memory.write(byte_address, std::slice::from_ref(&byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    cwd.len() as u64
}

pub(super) fn syscall_chdir(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_guest_c_string(guest_memory, request.argument(0), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let directory = match state.guest_directory_path(&path) {
        Ok(path) => path,
        Err(error) => return linux_error(error.linux_error_code()),
    };
    state.set_current_directory(display_guest_directory_path(&directory));
    0
}

pub(super) fn syscall_fchdir(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let directory = match state.guest_directory_path_for_fd(fd) {
        Ok(Some(path)) => path,
        Ok(None) => return linux_error(RISCV_LINUX_ENOTDIR),
        Err(_error) => return linux_error(RISCV_LINUX_EBADF),
    };
    state.set_current_directory(display_guest_directory_path(&directory));
    0
}

pub(super) fn display_guest_directory_path(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        b"/".to_vec()
    } else {
        let mut display = Vec::with_capacity(path.len() + 1);
        display.push(b'/');
        display.extend_from_slice(path);
        display
    }
}

impl RiscvSyscallState {
    pub(super) fn resolve_guest_path(
        &self,
        path: &[u8],
    ) -> Result<Vec<u8>, RiscvGuestPathResolutionError> {
        self.resolve_guest_path_from_directory(self.current_directory(), path)
    }

    pub(super) fn resolve_guest_path_from_directory(
        &self,
        directory: &[u8],
        path: &[u8],
    ) -> Result<Vec<u8>, RiscvGuestPathResolutionError> {
        let mut components = if path.starts_with(b"/") {
            Vec::new()
        } else {
            path_components(directory)
        };
        let raw_components = path.split(|byte| *byte == b'/').collect::<Vec<_>>();
        for (index, component) in raw_components.iter().enumerate() {
            match *component {
                b"" | b"." => {}
                b".." => {
                    components.pop();
                }
                _ => {
                    components.push(component.to_vec());
                    if index + 1 < raw_components.len() {
                        self.require_guest_directory(&join_components(&components))?;
                    }
                }
            }
        }
        Ok(join_components(&components))
    }

    pub(super) fn resolve_guest_path_for_create(&self, path: &[u8]) -> Vec<u8> {
        let path = canonical_guest_path_for_create(self.current_directory(), path);
        self.existing_guest_path_key(&path).unwrap_or(path)
    }

    pub(super) fn resolve_existing_guest_path(
        &self,
        path: &[u8],
    ) -> Result<Option<Vec<u8>>, RiscvGuestPathResolutionError> {
        Ok(self.existing_guest_path_key(&self.resolve_guest_path(path)?))
    }

    pub(super) fn resolve_existing_guest_regular_path(
        &self,
        path: &[u8],
    ) -> Result<Option<Vec<u8>>, RiscvGuestPathResolutionError> {
        Ok(self
            .resolve_existing_guest_path(path)?
            .filter(|path| self.guest_path_registered(path)))
    }

    pub(super) fn guest_directory_path(
        &self,
        path: &[u8],
    ) -> Result<Vec<u8>, RiscvGuestPathResolutionError> {
        let path = self.resolve_guest_path(path)?;
        if self.guest_directory_entries(&path).is_some() {
            return Ok(path);
        }
        if self.existing_guest_path_key(&path).is_some() {
            return Err(RiscvGuestPathResolutionError::NotDirectory);
        }
        Err(RiscvGuestPathResolutionError::Missing)
    }

    pub(super) fn guest_directory_path_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<Vec<u8>>, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self.guest_directory_paths.get(&description).cloned())
    }

    pub(super) fn existing_guest_path_key(&self, path: &[u8]) -> Option<Vec<u8>> {
        if self.guest_path_exists(path) {
            return Some(path.to_vec());
        }
        if path.is_empty() || path.starts_with(b"/") {
            return None;
        }
        let mut absolute = Vec::with_capacity(path.len() + 1);
        absolute.push(b'/');
        absolute.extend_from_slice(path);
        self.guest_path_exists(&absolute).then_some(absolute)
    }

    fn require_guest_directory(&self, path: &[u8]) -> Result<(), RiscvGuestPathResolutionError> {
        if self.guest_directory_entries(path).is_some() {
            return Ok(());
        }
        if self.existing_guest_path_key(path).is_some() {
            return Err(RiscvGuestPathResolutionError::NotDirectory);
        }
        Err(RiscvGuestPathResolutionError::Missing)
    }
}

fn path_components(path: &[u8]) -> Vec<Vec<u8>> {
    path.strip_prefix(b"/")
        .unwrap_or(path)
        .split(|byte| *byte == b'/')
        .filter(|component| !component.is_empty() && *component != b".")
        .map(Vec::from)
        .collect()
}

fn canonical_guest_path_for_create(current_directory: &[u8], path: &[u8]) -> Vec<u8> {
    let mut components = if path.starts_with(b"/") {
        Vec::new()
    } else {
        path_components(current_directory)
    };
    for component in path.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                components.pop();
            }
            _ => components.push(component.to_vec()),
        }
    }
    join_components(&components)
}

fn join_components(components: &[Vec<u8>]) -> Vec<u8> {
    let mut path = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if index != 0 {
            path.push(b'/');
        }
        path.extend_from_slice(component);
    }
    path
}
