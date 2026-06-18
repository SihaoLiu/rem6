use std::collections::BTreeMap;

use super::permissions::apply_file_creation_mask;
use super::{
    guest_directory_child_name, guest_path_inode, RiscvGuestDirectoryEntry, RiscvGuestNodeKind,
    RiscvSyscallState, RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestMkdirError {
    Exists,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestRmdirError {
    Missing,
    NotDirectory,
    NotEmpty,
}

impl RiscvSyscallState {
    pub fn register_guest_directory(&mut self, path: impl AsRef<[u8]>) {
        let path = registered_guest_directory_key(path.as_ref());
        self.guest_directories.insert(path.clone());
        self.ensure_guest_directory_identity(&path);
        self.guest_directory_modes
            .entry(path)
            .or_insert(RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS);
    }

    pub(super) fn guest_directory_entries(
        &self,
        path: &[u8],
    ) -> Option<Vec<RiscvGuestDirectoryEntry>> {
        let mut children = BTreeMap::new();
        for registered_path in self.guest_paths.iter().chain(self.guest_links.keys()) {
            let Some((name, nested)) = guest_directory_child_name(path, registered_path) else {
                continue;
            };
            let kind = if nested {
                RiscvGuestNodeKind::Directory
            } else if self.guest_links.contains_key(registered_path) {
                RiscvGuestNodeKind::Symlink
            } else {
                RiscvGuestNodeKind::RegularFile
            };
            let inode = if nested {
                guest_path_inode(&name)
            } else {
                self.guest_file_identity(registered_path).inode
            };
            children
                .entry(name.clone())
                .or_insert_with(|| RiscvGuestDirectoryEntry::new(name, kind, inode));
        }
        for registered_path in &self.guest_directories {
            let Some((name, nested)) = guest_directory_child_name(path, registered_path) else {
                continue;
            };
            let inode = if nested {
                guest_path_inode(&name)
            } else {
                self.guest_directory_identity(registered_path).inode
            };
            children.entry(name.clone()).or_insert_with(|| {
                RiscvGuestDirectoryEntry::new(name, RiscvGuestNodeKind::Directory, inode)
            });
        }
        let exact_directory = path == b"."
            || path == b"/"
            || path.is_empty()
            || self.guest_directories.contains(path);
        if children.is_empty() && !exact_directory {
            return None;
        }

        let mut entries = vec![
            RiscvGuestDirectoryEntry::new(
                b".".to_vec(),
                RiscvGuestNodeKind::Directory,
                guest_path_inode(path),
            ),
            RiscvGuestDirectoryEntry::new(
                b"..".to_vec(),
                RiscvGuestNodeKind::Directory,
                guest_path_inode(b".."),
            ),
        ];
        entries.extend(children.into_values());
        Some(entries)
    }

    pub(super) fn guest_directory_permissions(&self, path: &[u8]) -> u32 {
        self.guest_directory_modes
            .get(path)
            .copied()
            .unwrap_or(RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS)
    }

    pub(super) fn mkdir_guest_directory(
        &mut self,
        path: &[u8],
        mode: u64,
    ) -> Result<(), RiscvGuestMkdirError> {
        if self.existing_guest_path_key(path).is_some()
            || self.guest_directory_entries(path).is_some()
        {
            return Err(RiscvGuestMkdirError::Exists);
        }
        self.guest_directories.insert(path.to_vec());
        self.ensure_guest_directory_identity(path);
        self.guest_directory_modes
            .insert(path.to_vec(), apply_file_creation_mask(mode, self));
        Ok(())
    }

    pub(super) fn rmdir_guest_directory(
        &mut self,
        path: &[u8],
    ) -> Result<(), RiscvGuestRmdirError> {
        let Some(entries) = self.guest_directory_entries(path) else {
            if self.existing_guest_path_key(path).is_some() {
                return Err(RiscvGuestRmdirError::NotDirectory);
            }
            return Err(RiscvGuestRmdirError::Missing);
        };
        if entries.len() > 2 {
            return Err(RiscvGuestRmdirError::NotEmpty);
        }
        if self.guest_directories.remove(path) {
            let identity = self.guest_directory_identities.remove(path);
            self.guest_directory_modes.remove(path);
            if let Some(identity) = identity {
                self.drop_guest_xattrs_if_unlinked(identity);
            }
            Ok(())
        } else {
            Err(RiscvGuestRmdirError::Missing)
        }
    }
}

fn registered_guest_directory_key(path: &[u8]) -> Vec<u8> {
    path.strip_prefix(b"/").unwrap_or(path).to_vec()
}
