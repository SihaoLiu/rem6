use std::collections::{BTreeMap, BTreeSet};

use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EISDIR, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR, RISCV_LINUX_ENOTEMPTY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_RENAMEAT2: u64 = 276;
pub(super) const RISCV_LINUX_RENAMEAT: u64 = 38;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestRenameError {
    SourceMissing,
    DestinationIsDirectory,
    DestinationNotDirectory,
    DestinationNotEmpty,
    Invalid,
}

pub(super) fn syscall_renameat2(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_rename_operation(request, state, guest_memory, request.argument(4))
}

pub(super) fn syscall_renameat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_rename_operation(request, state, guest_memory, 0)
}

fn syscall_rename_operation(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
    flags: u64,
) -> u64 {
    if flags != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let source = match read_rename_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let destination = match read_rename_path(request.argument(3), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !dirfd_supports_path(request.argument(0), &source)
        || !dirfd_supports_path(request.argument(2), &destination)
    {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if source.is_empty() || destination.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let source = match state.resolve_existing_guest_path(&source) {
        Ok(Some(path)) => path,
        Ok(None) => return linux_error(RISCV_LINUX_ENOENT),
        Err(error) => return linux_error(error.linux_error_code()),
    };
    let destination = state.resolve_guest_path_for_create(&destination);

    match state.rename_guest_path(&source, &destination) {
        Ok(()) => 0,
        Err(RiscvGuestRenameError::SourceMissing) => linux_error(RISCV_LINUX_ENOENT),
        Err(RiscvGuestRenameError::DestinationIsDirectory) => linux_error(RISCV_LINUX_EISDIR),
        Err(RiscvGuestRenameError::DestinationNotDirectory) => linux_error(RISCV_LINUX_ENOTDIR),
        Err(RiscvGuestRenameError::DestinationNotEmpty) => linux_error(RISCV_LINUX_ENOTEMPTY),
        Err(RiscvGuestRenameError::Invalid) => linux_error(RISCV_LINUX_EINVAL),
    }
}

fn read_rename_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
    read_guest_c_string(guest_memory, address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn dirfd_supports_path(dirfd: u64, path: &[u8]) -> bool {
    dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/")
}

impl RiscvSyscallState {
    pub(super) fn rename_guest_path(
        &mut self,
        source: &[u8],
        destination: &[u8],
    ) -> Result<(), RiscvGuestRenameError> {
        if !self.guest_path_exists(source) {
            return Err(RiscvGuestRenameError::SourceMissing);
        }
        if source == destination {
            return Ok(());
        }
        let source_is_directory = self.guest_directories.contains(source);
        let destination_is_directory = self.guest_directory_entries(destination).is_some();
        let destination_exists = self.guest_path_exists(destination) || destination_is_directory;

        if source_is_directory {
            if guest_path_is_descendant(destination, source) {
                return Err(RiscvGuestRenameError::Invalid);
            }
            if destination_exists && !destination_is_directory {
                return Err(RiscvGuestRenameError::DestinationNotDirectory);
            }
            if destination_is_directory
                && self
                    .guest_directory_entries(destination)
                    .is_some_and(|entries| entries.len() > 2)
            {
                return Err(RiscvGuestRenameError::DestinationNotEmpty);
            }
            self.remove_guest_node_exact(destination);
            move_rebased_set(&mut self.guest_paths, source, destination);
            move_rebased_set(&mut self.guest_directories, source, destination);
            move_rebased_map(&mut self.guest_directory_identities, source, destination);
            move_rebased_map(&mut self.guest_directory_modes, source, destination);
            move_rebased_map(&mut self.guest_files, source, destination);
            move_rebased_map(&mut self.guest_links, source, destination);
            move_rebased_map(&mut self.guest_file_identities, source, destination);
            move_rebased_values(&mut self.guest_directory_paths, source, destination);
            if let Some(path) =
                rebase_display_guest_path(&self.current_directory, source, destination)
            {
                self.current_directory = path;
            }
            return Ok(());
        }

        if destination_is_directory {
            return Err(RiscvGuestRenameError::DestinationIsDirectory);
        }
        if self.guest_path_exists(destination)
            && self.guest_file_identity(source) == self.guest_file_identity(destination)
        {
            return Ok(());
        }

        let source_is_path = self.guest_paths.remove(source);
        let source_file = self.guest_files.remove(source);
        let source_link = self.guest_links.remove(source);
        let source_identity = self.guest_file_identities.remove(source);

        self.remove_guest_node_exact(destination);

        let destination = destination.to_vec();
        if source_is_path {
            self.guest_paths.insert(destination.clone());
        }
        if let Some(contents) = source_file {
            self.guest_files.insert(destination.clone(), contents);
        }
        if let Some(target) = source_link {
            self.guest_links.insert(destination.clone(), target);
        }
        if let Some(identity) = source_identity {
            self.guest_file_identities.insert(destination, identity);
        }
        Ok(())
    }

    fn remove_guest_node_exact(&mut self, path: &[u8]) {
        self.guest_paths.remove(path);
        let removed_directory = self.guest_directories.remove(path);
        let removed_directory_identity = self.guest_directory_identities.remove(path);
        self.guest_directory_modes.remove(path);
        self.guest_files.remove(path);
        self.guest_links.remove(path);
        if let Some(identity) = self.guest_file_identities.remove(path) {
            self.drop_guest_file_mode_if_unlinked(identity);
            self.drop_guest_xattrs_if_unlinked(identity);
        }
        if removed_directory {
            if let Some(identity) = removed_directory_identity {
                self.drop_guest_xattrs_if_unlinked(identity);
            }
        }
    }
}

fn guest_path_is_descendant(path: &[u8], parent: &[u8]) -> bool {
    path.strip_prefix(parent)
        .is_some_and(|suffix| suffix.first() == Some(&b'/'))
}

fn rebase_guest_path(path: &[u8], source: &[u8], destination: &[u8]) -> Option<Vec<u8>> {
    if path == source {
        return Some(destination.to_vec());
    }
    let suffix = path.strip_prefix(source)?;
    let child_suffix = suffix.strip_prefix(b"/")?;
    let mut rebased = destination.to_vec();
    if !rebased.is_empty() {
        rebased.push(b'/');
    }
    rebased.extend_from_slice(child_suffix);
    Some(rebased)
}

fn rebase_display_guest_path(path: &[u8], source: &[u8], destination: &[u8]) -> Option<Vec<u8>> {
    let canonical = path.strip_prefix(b"/").unwrap_or(path);
    let rebased = rebase_guest_path(canonical, source, destination)?;
    if rebased.is_empty() {
        return Some(b"/".to_vec());
    }

    let mut display = Vec::with_capacity(rebased.len() + 1);
    display.push(b'/');
    display.extend_from_slice(&rebased);
    Some(display)
}

fn move_rebased_set(set: &mut BTreeSet<Vec<u8>>, source: &[u8], destination: &[u8]) {
    let moves = set
        .iter()
        .filter_map(|path| {
            rebase_guest_path(path, source, destination).map(|rebased| (path.clone(), rebased))
        })
        .collect::<Vec<_>>();
    for (old, new) in moves {
        set.remove(&old);
        set.insert(new);
    }
}

fn move_rebased_map<T>(map: &mut BTreeMap<Vec<u8>, T>, source: &[u8], destination: &[u8]) {
    let moves = map
        .keys()
        .filter_map(|path| {
            rebase_guest_path(path, source, destination).map(|rebased| (path.clone(), rebased))
        })
        .collect::<Vec<_>>();
    for (old, new) in moves {
        if let Some(value) = map.remove(&old) {
            map.insert(new, value);
        }
    }
}

fn move_rebased_values<K: Ord>(map: &mut BTreeMap<K, Vec<u8>>, source: &[u8], destination: &[u8]) {
    for path in map.values_mut() {
        if let Some(rebased) = rebase_guest_path(path, source, destination) {
            *path = rebased;
        }
    }
}
