use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFd(u32);

impl GuestFd {
    pub fn new(value: i32) -> Result<Self, GuestFdError> {
        if value < 0 {
            return Err(GuestFdError::NegativeFd { fd: value });
        }

        Ok(Self(value as u32))
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFileDescriptionId(u64);

impl GuestFileDescriptionId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestHostFd(i32);

impl GuestHostFd {
    pub fn new(value: i32) -> Result<Self, GuestFdError> {
        if value < 0 {
            return Err(GuestFdError::NegativeHostFd { fd: value });
        }

        Ok(Self(value))
    }

    pub const fn get(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFileStatusFlags(u32);

impl GuestFileStatusFlags {
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFileDescription {
    id: GuestFileDescriptionId,
    host_fd: Option<GuestHostFd>,
    status_flags: GuestFileStatusFlags,
}

impl GuestFileDescription {
    pub const fn guest_backed(
        id: GuestFileDescriptionId,
        status_flags: GuestFileStatusFlags,
    ) -> Self {
        Self {
            id,
            host_fd: None,
            status_flags,
        }
    }

    pub const fn host_backed(
        id: GuestFileDescriptionId,
        host_fd: GuestHostFd,
        status_flags: GuestFileStatusFlags,
    ) -> Self {
        Self {
            id,
            host_fd: Some(host_fd),
            status_flags,
        }
    }

    pub const fn id(&self) -> GuestFileDescriptionId {
        self.id
    }

    pub const fn host_fd(&self) -> Option<GuestHostFd> {
        self.host_fd
    }

    pub const fn status_flags(&self) -> GuestFileStatusFlags {
        self.status_flags
    }

    pub const fn set_status_flags(&mut self, status_flags: GuestFileStatusFlags) {
        self.status_flags = status_flags;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFdEntry {
    description: GuestFileDescriptionId,
    close_on_exec: bool,
}

impl GuestFdEntry {
    pub const fn new(description: GuestFileDescriptionId) -> Self {
        Self {
            description,
            close_on_exec: false,
        }
    }

    pub const fn with_close_on_exec(mut self, close_on_exec: bool) -> Self {
        self.close_on_exec = close_on_exec;
        self
    }

    pub const fn description(&self) -> GuestFileDescriptionId {
        self.description
    }

    pub const fn close_on_exec(&self) -> bool {
        self.close_on_exec
    }

    fn duplicated(&self) -> Self {
        Self {
            description: self.description,
            close_on_exec: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFdCloseRecord {
    fd: GuestFd,
    entry: GuestFdEntry,
}

impl GuestFdCloseRecord {
    pub const fn new(fd: GuestFd, entry: GuestFdEntry) -> Self {
        Self { fd, entry }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn entry(&self) -> &GuestFdEntry {
        &self.entry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestFdError {
    NegativeFd { fd: i32 },
    NegativeHostFd { fd: i32 },
    BadFd { fd: GuestFd },
    DuplicateFd { fd: GuestFd },
    DuplicateFileDescription { description: GuestFileDescriptionId },
    MissingFileDescription { description: GuestFileDescriptionId },
    FdSpaceExhausted,
}

impl fmt::Display for GuestFdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeFd { fd } => write!(formatter, "negative guest file descriptor {fd}"),
            Self::NegativeHostFd { fd } => write!(formatter, "negative host file descriptor {fd}"),
            Self::BadFd { fd } => write!(formatter, "bad guest file descriptor {}", fd.get()),
            Self::DuplicateFd { fd } => {
                write!(
                    formatter,
                    "guest file descriptor {} already exists",
                    fd.get()
                )
            }
            Self::DuplicateFileDescription { description } => {
                write!(
                    formatter,
                    "guest file description {} already exists",
                    description.get()
                )
            }
            Self::MissingFileDescription { description } => {
                write!(
                    formatter,
                    "guest file description {} is missing",
                    description.get()
                )
            }
            Self::FdSpaceExhausted => write!(formatter, "guest file descriptor space exhausted"),
        }
    }
}

impl Error for GuestFdError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GuestFdTable {
    entries: BTreeMap<GuestFd, GuestFdEntry>,
    descriptions: BTreeMap<GuestFileDescriptionId, GuestFileDescription>,
}

impl GuestFdTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, fd: GuestFd, entry: GuestFdEntry) -> Result<(), GuestFdError> {
        if self.entries.contains_key(&fd) {
            return Err(GuestFdError::DuplicateFd { fd });
        }

        self.entries.insert(fd, entry);
        Ok(())
    }

    pub fn insert_description(
        &mut self,
        description: GuestFileDescription,
    ) -> Result<(), GuestFdError> {
        let id = description.id();
        if self.descriptions.contains_key(&id) {
            return Err(GuestFdError::DuplicateFileDescription { description: id });
        }

        self.descriptions.insert(id, description);
        Ok(())
    }

    pub fn entry(&self, fd: GuestFd) -> Option<&GuestFdEntry> {
        self.entries.get(&fd)
    }

    pub fn description(
        &self,
        description: GuestFileDescriptionId,
    ) -> Option<&GuestFileDescription> {
        self.descriptions.get(&description)
    }

    pub fn description_for_fd(&self, fd: GuestFd) -> Result<&GuestFileDescription, GuestFdError> {
        let description = self
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        self.description(description)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    pub fn status_flags(&self, fd: GuestFd) -> Result<GuestFileStatusFlags, GuestFdError> {
        Ok(self.description_for_fd(fd)?.status_flags())
    }

    pub fn set_status_flags(
        &mut self,
        fd: GuestFd,
        status_flags: GuestFileStatusFlags,
    ) -> Result<(), GuestFdError> {
        let description = self
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let description = self
            .descriptions
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        description.set_status_flags(status_flags);
        Ok(())
    }

    pub fn close(&mut self, fd: GuestFd) -> Result<GuestFdEntry, GuestFdError> {
        self.entries.remove(&fd).ok_or(GuestFdError::BadFd { fd })
    }

    pub fn close_on_exec(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let entry = self.entry(fd).ok_or(GuestFdError::BadFd { fd })?;
        Ok(entry.close_on_exec())
    }

    pub fn set_close_on_exec(
        &mut self,
        fd: GuestFd,
        close_on_exec: bool,
    ) -> Result<(), GuestFdError> {
        let entry = self
            .entries
            .get_mut(&fd)
            .ok_or(GuestFdError::BadFd { fd })?;
        entry.close_on_exec = close_on_exec;
        Ok(())
    }

    pub fn close_on_exec_descriptors(&mut self) -> Vec<GuestFdCloseRecord> {
        let mut retained = BTreeMap::new();
        let mut closed = Vec::new();

        for (fd, entry) in std::mem::take(&mut self.entries) {
            if entry.close_on_exec {
                closed.push(GuestFdCloseRecord::new(fd, entry));
            } else {
                retained.insert(fd, entry);
            }
        }

        self.entries = retained;
        closed
    }

    pub fn dup(&mut self, old_fd: GuestFd) -> Result<GuestFd, GuestFdError> {
        let entry = self
            .entry(old_fd)
            .ok_or(GuestFdError::BadFd { fd: old_fd })?
            .duplicated();
        let new_fd = self.next_available_fd()?;
        self.entries.insert(new_fd, entry);
        Ok(new_fd)
    }

    pub fn dup2(&mut self, old_fd: GuestFd, new_fd: GuestFd) -> Result<GuestFd, GuestFdError> {
        let entry = self
            .entry(old_fd)
            .ok_or(GuestFdError::BadFd { fd: old_fd })?;
        if old_fd == new_fd {
            return Ok(new_fd);
        }

        self.entries.insert(new_fd, entry.duplicated());
        Ok(new_fd)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn next_available_fd(&self) -> Result<GuestFd, GuestFdError> {
        let mut candidate = 0_i32;
        loop {
            let fd = GuestFd::new(candidate)?;
            if !self.entries.contains_key(&fd) {
                return Ok(fd);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }
}
