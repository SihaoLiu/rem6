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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestFileOffset(u64);

impl GuestFileOffset {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    fn checked_add(self, increment: u64) -> Option<Self> {
        self.0.checked_add(increment).map(Self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFileDescription {
    id: GuestFileDescriptionId,
    host_fd: Option<GuestHostFd>,
    status_flags: GuestFileStatusFlags,
    file_offset: GuestFileOffset,
    signal_owner: i32,
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
            file_offset: GuestFileOffset::new(0),
            signal_owner: 0,
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
            file_offset: GuestFileOffset::new(0),
            signal_owner: 0,
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

    pub const fn file_offset(&self) -> GuestFileOffset {
        self.file_offset
    }

    pub const fn set_file_offset(&mut self, file_offset: GuestFileOffset) {
        self.file_offset = file_offset;
    }

    pub const fn signal_owner(&self) -> i32 {
        self.signal_owner
    }

    pub const fn set_signal_owner(&mut self, owner: i32) {
        self.signal_owner = owner;
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
pub struct GuestFdSnapshotEntry {
    fd: GuestFd,
    entry: GuestFdEntry,
}

impl GuestFdSnapshotEntry {
    pub const fn new(fd: GuestFd, entry: GuestFdEntry) -> Self {
        Self { fd, entry }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn entry(&self) -> &GuestFdEntry {
        &self.entry
    }

    fn into_parts(self) -> (GuestFd, GuestFdEntry) {
        (self.fd, self.entry)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GuestFdTableSnapshot {
    entries: Vec<GuestFdSnapshotEntry>,
    descriptions: Vec<GuestFileDescription>,
}

impl GuestFdTableSnapshot {
    pub fn new(
        entries: Vec<GuestFdSnapshotEntry>,
        descriptions: Vec<GuestFileDescription>,
    ) -> Self {
        Self {
            entries,
            descriptions,
        }
    }

    pub fn entries(&self) -> &[GuestFdSnapshotEntry] {
        &self.entries
    }

    pub fn descriptions(&self) -> &[GuestFileDescription] {
        &self.descriptions
    }

    fn into_parts(self) -> (Vec<GuestFdSnapshotEntry>, Vec<GuestFileDescription>) {
        (self.entries, self.descriptions)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFdCloseRecord {
    fd: GuestFd,
    entry: GuestFdEntry,
    released_description: Option<GuestFileDescription>,
}

impl GuestFdCloseRecord {
    pub const fn new(fd: GuestFd, entry: GuestFdEntry) -> Self {
        Self {
            fd,
            entry,
            released_description: None,
        }
    }

    const fn with_released_description(
        fd: GuestFd,
        entry: GuestFdEntry,
        released_description: GuestFileDescription,
    ) -> Self {
        Self {
            fd,
            entry,
            released_description: Some(released_description),
        }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn entry(&self) -> &GuestFdEntry {
        &self.entry
    }

    pub fn released_description(&self) -> Option<&GuestFileDescription> {
        self.released_description.as_ref()
    }

    pub fn into_released_description(self) -> Option<GuestFileDescription> {
        self.released_description
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestFdDup2Record {
    fd: GuestFd,
    replaced: Option<GuestFdCloseRecord>,
}

impl GuestFdDup2Record {
    const fn new(fd: GuestFd, replaced: Option<GuestFdCloseRecord>) -> Self {
        Self { fd, replaced }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub fn replaced(&self) -> Option<&GuestFdCloseRecord> {
        self.replaced.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestFdError {
    NegativeFd {
        fd: i32,
    },
    NegativeHostFd {
        fd: i32,
    },
    BadFd {
        fd: GuestFd,
    },
    DuplicateFd {
        fd: GuestFd,
    },
    DuplicateFileDescription {
        description: GuestFileDescriptionId,
    },
    MissingFileDescription {
        description: GuestFileDescriptionId,
    },
    FileOffsetOverflow {
        description: GuestFileDescriptionId,
        offset: GuestFileOffset,
        increment: u64,
    },
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
            Self::FileOffsetOverflow {
                description,
                offset,
                increment,
            } => {
                write!(
                    formatter,
                    "guest file description {} offset {} overflows when advanced by {} bytes",
                    description.get(),
                    offset.get(),
                    increment
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

    pub fn snapshot(&self) -> GuestFdTableSnapshot {
        GuestFdTableSnapshot::new(
            self.entries
                .iter()
                .map(|(&fd, entry)| GuestFdSnapshotEntry::new(fd, entry.clone()))
                .collect(),
            self.descriptions.values().cloned().collect(),
        )
    }

    pub fn from_snapshot(snapshot: GuestFdTableSnapshot) -> Result<Self, GuestFdError> {
        let (entries, descriptions) = snapshot.into_parts();
        let mut table = Self::new();

        for description in descriptions {
            table.insert_description(description)?;
        }

        for snapshot_entry in entries {
            let (fd, entry) = snapshot_entry.into_parts();
            let description = entry.description();
            if !table.descriptions.contains_key(&description) {
                return Err(GuestFdError::MissingFileDescription { description });
            }
            table.insert(fd, entry)?;
        }

        Ok(table)
    }

    pub fn restore_snapshot(&mut self, snapshot: GuestFdTableSnapshot) -> Result<(), GuestFdError> {
        let restored = Self::from_snapshot(snapshot)?;
        *self = restored;
        Ok(())
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
        let (_, description) = self.description_for_fd_mut(fd)?;
        description.set_status_flags(status_flags);
        Ok(())
    }

    pub fn file_offset(&self, fd: GuestFd) -> Result<GuestFileOffset, GuestFdError> {
        Ok(self.description_for_fd(fd)?.file_offset())
    }

    pub fn set_file_offset(
        &mut self,
        fd: GuestFd,
        file_offset: GuestFileOffset,
    ) -> Result<(), GuestFdError> {
        let (_, description) = self.description_for_fd_mut(fd)?;
        description.set_file_offset(file_offset);
        Ok(())
    }

    pub fn advance_file_offset(
        &mut self,
        fd: GuestFd,
        increment: u64,
    ) -> Result<GuestFileOffset, GuestFdError> {
        let (description, description_record) = self.description_for_fd_mut(fd)?;
        let offset = description_record.file_offset();
        let advanced = offset
            .checked_add(increment)
            .ok_or(GuestFdError::FileOffsetOverflow {
                description,
                offset,
                increment,
            })?;
        description_record.set_file_offset(advanced);
        Ok(advanced)
    }

    pub fn signal_owner(&self, fd: GuestFd) -> Result<i32, GuestFdError> {
        Ok(self.description_for_fd(fd)?.signal_owner())
    }

    pub fn set_signal_owner(&mut self, fd: GuestFd, owner: i32) -> Result<(), GuestFdError> {
        let (_, description) = self.description_for_fd_mut(fd)?;
        description.set_signal_owner(owner);
        Ok(())
    }

    pub fn close_descriptor(&mut self, fd: GuestFd) -> Result<GuestFdCloseRecord, GuestFdError> {
        let entry = self.entries.remove(&fd).ok_or(GuestFdError::BadFd { fd })?;
        Ok(self.close_record_after_removal(fd, entry))
    }

    pub fn close_descriptor_range(&mut self, first: u64, last: u64) -> Vec<GuestFdCloseRecord> {
        let fds = self.fds_in_number_range(first, last);
        fds.into_iter()
            .filter_map(|fd| self.close_descriptor(fd).ok())
            .collect()
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

    pub fn set_close_on_exec_range(&mut self, first: u64, last: u64, close_on_exec: bool) {
        for fd in self.fds_in_number_range(first, last) {
            if let Some(entry) = self.entries.get_mut(&fd) {
                entry.close_on_exec = close_on_exec;
            }
        }
    }

    pub fn close_on_exec_descriptors(&mut self) -> Vec<GuestFdCloseRecord> {
        let mut retained = BTreeMap::new();
        let mut closed = Vec::new();
        let mut closed_description_counts = BTreeMap::new();

        for (fd, entry) in std::mem::take(&mut self.entries) {
            if entry.close_on_exec {
                *closed_description_counts
                    .entry(entry.description())
                    .or_insert(0_usize) += 1;
                closed.push(GuestFdCloseRecord::new(fd, entry));
            } else {
                retained.insert(fd, entry);
            }
        }

        self.entries = retained;
        for record in &mut closed {
            let description = record.entry().description();
            let closed_count = closed_description_counts
                .get_mut(&description)
                .expect("closed record description count must exist");
            *closed_count -= 1;
            if *closed_count == 0 {
                record.released_description = self.remove_description_if_unreferenced(description);
            }
        }
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

    pub fn dup_from_min(
        &mut self,
        old_fd: GuestFd,
        minimum_fd: GuestFd,
    ) -> Result<GuestFd, GuestFdError> {
        let entry = self
            .entry(old_fd)
            .ok_or(GuestFdError::BadFd { fd: old_fd })?
            .duplicated();
        let new_fd = self.next_available_fd_from(minimum_fd)?;
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

    pub fn dup2_with_replacement(
        &mut self,
        old_fd: GuestFd,
        new_fd: GuestFd,
    ) -> Result<GuestFdDup2Record, GuestFdError> {
        let entry = self
            .entry(old_fd)
            .ok_or(GuestFdError::BadFd { fd: old_fd })?;
        if old_fd == new_fd {
            return Ok(GuestFdDup2Record::new(new_fd, None));
        }

        let duplicated = entry.duplicated();
        let replaced = self
            .entries
            .insert(new_fd, duplicated)
            .map(|entry| self.close_record_after_removal(new_fd, entry));
        Ok(GuestFdDup2Record::new(new_fd, replaced))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn next_available_fd(&self) -> Result<GuestFd, GuestFdError> {
        self.next_available_fd_from(GuestFd::new(0)?)
    }

    fn next_available_fd_from(&self, minimum_fd: GuestFd) -> Result<GuestFd, GuestFdError> {
        let mut candidate = i32::try_from(minimum_fd.get()).expect("guest fd is created from i32");
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

    fn fds_in_number_range(&self, first: u64, last: u64) -> Vec<GuestFd> {
        self.entries
            .keys()
            .copied()
            .filter(|fd| {
                let value = u64::from(fd.get());
                value >= first && value <= last
            })
            .collect()
    }

    fn close_record_after_removal(
        &mut self,
        fd: GuestFd,
        entry: GuestFdEntry,
    ) -> GuestFdCloseRecord {
        match self.remove_description_if_unreferenced(entry.description()) {
            Some(released_description) => {
                GuestFdCloseRecord::with_released_description(fd, entry, released_description)
            }
            None => GuestFdCloseRecord::new(fd, entry),
        }
    }

    fn remove_description_if_unreferenced(
        &mut self,
        description: GuestFileDescriptionId,
    ) -> Option<GuestFileDescription> {
        if self
            .entries
            .values()
            .any(|entry| entry.description() == description)
        {
            return None;
        }

        self.descriptions.remove(&description)
    }

    fn description_for_fd_mut(
        &mut self,
        fd: GuestFd,
    ) -> Result<(GuestFileDescriptionId, &mut GuestFileDescription), GuestFdError> {
        let description = self
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let description_record = self
            .descriptions
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        Ok((description, description_record))
    }
}
