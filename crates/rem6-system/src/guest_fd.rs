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
    BadFd { fd: GuestFd },
    DuplicateFd { fd: GuestFd },
    FdSpaceExhausted,
}

impl fmt::Display for GuestFdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeFd { fd } => write!(formatter, "negative guest file descriptor {fd}"),
            Self::BadFd { fd } => write!(formatter, "bad guest file descriptor {}", fd.get()),
            Self::DuplicateFd { fd } => {
                write!(
                    formatter,
                    "guest file descriptor {} already exists",
                    fd.get()
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

    pub fn entry(&self, fd: GuestFd) -> Option<&GuestFdEntry> {
        self.entries.get(&fd)
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
