use super::{
    stat::guest_path_inode, RiscvGuestFileIdentity, RiscvGuestNodeKind, RiscvSyscallState,
    RISCV_LINUX_O_APPEND,
};
use crate::{GuestFd, GuestFdError, GuestFileOffset};

const RISCV_GUEST_FILE_DENSE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestFileWriteError {
    Fd(GuestFdError),
    FileTooLarge,
}

impl From<GuestFdError> for RiscvGuestFileWriteError {
    fn from(error: GuestFdError) -> Self {
        Self::Fd(error)
    }
}

impl RiscvSyscallState {
    pub(super) fn replace_guest_file_contents(&mut self, path: &[u8], contents: Vec<u8>) {
        let path = path.to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_file_identities
            .entry(path.clone())
            .or_insert_with(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(&path),
            });
        let identity = self.guest_file_identity(&path);
        self.synchronize_guest_file_contents(identity, contents);
    }

    fn synchronize_guest_file_contents(
        &mut self,
        identity: RiscvGuestFileIdentity,
        contents: Vec<u8>,
    ) {
        let paths = self
            .guest_paths
            .iter()
            .filter(|path| self.guest_file_identity(path) == identity)
            .cloned()
            .collect::<Vec<_>>();
        for path in paths {
            self.guest_files.insert(path, contents.clone());
        }

        let description_ids = self
            .guest_file_stats
            .iter()
            .filter_map(|(description, stat)| {
                (stat.identity == identity && stat.kind == RiscvGuestNodeKind::RegularFile)
                    .then_some(*description)
            })
            .collect::<Vec<_>>();
        for description in description_ids {
            if let Some(file_contents) = self.guest_file_descriptions.get_mut(&description) {
                *file_contents = contents.clone();
            }
            if let Some(stat) = self.guest_file_stats.get_mut(&description) {
                stat.size = contents.len() as u64;
            }
        }
    }

    pub(super) fn write_guest_file_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<bool, RiscvGuestFileWriteError> {
        let byte_count =
            u64::try_from(bytes.len()).map_err(|_| RiscvGuestFileWriteError::FileTooLarge)?;
        if self.guest_file_write_exceeds_dense_limit(fd, byte_count)? {
            return Err(RiscvGuestFileWriteError::FileTooLarge);
        }
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let append = self.guest_fd_appends_to_file(fd)?;
        let offset = if append {
            let Some(contents) = self.guest_file_descriptions.get(&description) else {
                return Ok(false);
            };
            GuestFileOffset::new(contents.len() as u64)
        } else {
            self.guest_fds.file_offset(fd)?
        };
        if append {
            self.guest_fds.set_file_offset(fd, offset)?;
        }
        let Some(contents) = self.guest_file_descriptions.get_mut(&description) else {
            return Ok(false);
        };
        let start = usize::try_from(offset.get()).map_err(|_| GuestFdError::BadFd { fd })?;
        let end = start
            .checked_add(bytes.len())
            .ok_or(GuestFdError::BadFd { fd })?;
        if end > contents.len() {
            contents.resize(end, 0);
        }
        contents[start..end].copy_from_slice(bytes);
        let contents = contents.clone();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            self.synchronize_guest_file_contents(stat.identity, contents);
        } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned() {
            self.guest_files.insert(path, contents);
        }
        Ok(true)
    }

    pub(super) fn guest_file_write_exceeds_dense_limit(
        &self,
        fd: GuestFd,
        byte_count: u64,
    ) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        if !self.guest_file_descriptions.contains_key(&description) {
            return Ok(false);
        }
        let offset = if self.guest_fd_appends_to_file(fd)? {
            let Some(contents) = self.guest_file_descriptions.get(&description) else {
                return Ok(false);
            };
            contents.len() as u64
        } else {
            self.guest_fds.file_offset(fd)?.get()
        };
        let Some(end) = offset.checked_add(byte_count) else {
            return Ok(true);
        };
        Ok(end > RISCV_GUEST_FILE_DENSE_LIMIT_BYTES)
    }

    fn guest_fd_appends_to_file(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_APPEND as u32 != 0)
    }
}
