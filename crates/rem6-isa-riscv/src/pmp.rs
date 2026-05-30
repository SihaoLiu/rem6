use std::error::Error;
use std::fmt;

const PMP_READ_BIT: u8 = 1 << 0;
const PMP_WRITE_BIT: u8 = 1 << 1;
const PMP_EXECUTE_BIT: u8 = 1 << 2;
const PMP_ADDRESS_MODE_SHIFT: u8 = 3;
const PMP_ADDRESS_MODE_MASK: u8 = 0b11 << PMP_ADDRESS_MODE_SHIFT;
const PMP_LOCKED_BIT: u8 = 1 << 7;
const DEFAULT_MAX_PMP_ENTRIES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvPrivilegeMode {
    User,
    Supervisor,
    Machine,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvPmpAccessKind {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvPmpAddressMode {
    Off,
    Tor,
    Na4,
    Napot,
}

impl RiscvPmpAddressMode {
    pub const fn from_bits(bits: u8) -> Result<Self, RiscvPmpError> {
        match bits & 0b11 {
            0 => Ok(Self::Off),
            1 => Ok(Self::Tor),
            2 => Ok(Self::Na4),
            3 => Ok(Self::Napot),
            _ => unreachable!(),
        }
    }

    pub const fn bits(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Tor => 1,
            Self::Na4 => 2,
            Self::Napot => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RiscvPmpConfig {
    bits: u8,
}

impl RiscvPmpConfig {
    pub const fn new(address_mode: RiscvPmpAddressMode) -> Self {
        Self {
            bits: address_mode.bits() << PMP_ADDRESS_MODE_SHIFT,
        }
    }

    pub const fn from_bits(bits: u8) -> Result<Self, RiscvPmpError> {
        let address_mode = (bits & PMP_ADDRESS_MODE_MASK) >> PMP_ADDRESS_MODE_SHIFT;
        match RiscvPmpAddressMode::from_bits(address_mode) {
            Ok(_) => Ok(Self { bits }),
            Err(error) => Err(error),
        }
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn address_mode(self) -> RiscvPmpAddressMode {
        match RiscvPmpAddressMode::from_bits(
            (self.bits & PMP_ADDRESS_MODE_MASK) >> PMP_ADDRESS_MODE_SHIFT,
        ) {
            Ok(mode) => mode,
            Err(_) => unreachable!(),
        }
    }

    pub const fn read(self) -> bool {
        self.bits & PMP_READ_BIT != 0
    }

    pub const fn write(self) -> bool {
        self.bits & PMP_WRITE_BIT != 0
    }

    pub const fn execute(self) -> bool {
        self.bits & PMP_EXECUTE_BIT != 0
    }

    pub const fn locked(self) -> bool {
        self.bits & PMP_LOCKED_BIT != 0
    }

    pub const fn with_read(mut self, read: bool) -> Self {
        self.bits = set_bit(self.bits, PMP_READ_BIT, read);
        self
    }

    pub const fn with_write(mut self, write: bool) -> Self {
        self.bits = set_bit(self.bits, PMP_WRITE_BIT, write);
        self
    }

    pub const fn with_execute(mut self, execute: bool) -> Self {
        self.bits = set_bit(self.bits, PMP_EXECUTE_BIT, execute);
        self
    }

    pub const fn with_locked(mut self, locked: bool) -> Self {
        self.bits = set_bit(self.bits, PMP_LOCKED_BIT, locked);
        self
    }

    const fn reset_address_mode_and_lock(self) -> Self {
        Self {
            bits: self.bits & !(PMP_ADDRESS_MODE_MASK | PMP_LOCKED_BIT),
        }
    }

    const fn permits(self, kind: RiscvPmpAccessKind) -> bool {
        match kind {
            RiscvPmpAccessKind::Read => self.read(),
            RiscvPmpAccessKind::Write => self.write(),
            RiscvPmpAccessKind::Execute => self.execute(),
        }
    }
}

impl Default for RiscvPmpConfig {
    fn default() -> Self {
        Self::new(RiscvPmpAddressMode::Off)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RiscvPmpRange {
    start: u64,
    end: u64,
}

impl RiscvPmpRange {
    pub const fn new(start: u64, end: u64) -> Result<Self, RiscvPmpError> {
        if start >= end {
            return Err(RiscvPmpError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }

    fn contains_access(self, address: u64, size: u64) -> Result<bool, RiscvPmpError> {
        let Some(last) = address.checked_add(size - 1) else {
            return Err(RiscvPmpError::AddressOverflow { address, size });
        };
        Ok(address >= self.start && last < self.end)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RiscvPmpEntry {
    raw_addr: u64,
    config: RiscvPmpConfig,
    range: Option<RiscvPmpRange>,
}

impl RiscvPmpEntry {
    pub const fn raw_addr(&self) -> u64 {
        self.raw_addr
    }

    pub const fn config(&self) -> RiscvPmpConfig {
        self.config
    }

    pub const fn range(&self) -> Option<RiscvPmpRange> {
        self.range
    }

    const fn snapshot(&self) -> RiscvPmpSnapshotEntry {
        RiscvPmpSnapshotEntry::new(self.raw_addr, self.config)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvPmpSnapshotEntry {
    raw_addr: u64,
    config: RiscvPmpConfig,
}

impl RiscvPmpSnapshotEntry {
    pub const fn new(raw_addr: u64, config: RiscvPmpConfig) -> Self {
        Self { raw_addr, config }
    }

    pub const fn raw_addr(&self) -> u64 {
        self.raw_addr
    }

    pub const fn config(&self) -> RiscvPmpConfig {
        self.config
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvPmpSnapshot {
    entries: Vec<RiscvPmpSnapshotEntry>,
}

impl RiscvPmpSnapshot {
    pub fn new(entries: Vec<RiscvPmpSnapshotEntry>) -> Result<Self, RiscvPmpError> {
        validate_entry_count(entries.len())?;
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[RiscvPmpSnapshotEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvPmpTable {
    entries: Vec<RiscvPmpEntry>,
    active_rule_count: usize,
}

impl RiscvPmpTable {
    pub fn new(entry_count: usize) -> Result<Self, RiscvPmpError> {
        validate_entry_count(entry_count)?;
        let mut table = Self {
            entries: vec![RiscvPmpEntry::default(); entry_count],
            active_rule_count: 0,
        };
        table.rebuild_ranges()?;
        Ok(table)
    }

    pub fn active_rule_count(&self) -> usize {
        self.active_rule_count
    }

    pub fn entries(&self) -> &[RiscvPmpEntry] {
        &self.entries
    }

    pub fn entry(&self, index: usize) -> Result<&RiscvPmpEntry, RiscvPmpError> {
        self.entries
            .get(index)
            .ok_or(RiscvPmpError::EntryIndexOutOfRange {
                index,
                entries: self.entries.len(),
            })
    }

    pub fn write_config(
        &mut self,
        index: usize,
        config: RiscvPmpConfig,
    ) -> Result<(), RiscvPmpError> {
        self.ensure_entry_mutable(index)?;
        self.entries[index].config = config;
        self.rebuild_ranges()
    }

    pub fn write_config_bits(&mut self, index: usize, bits: u8) -> Result<(), RiscvPmpError> {
        self.write_config(index, RiscvPmpConfig::from_bits(bits)?)
    }

    pub fn write_addr(&mut self, index: usize, raw_addr: u64) -> Result<(), RiscvPmpError> {
        self.ensure_entry_mutable(index)?;
        if let Some(next) = self.entries.get(index + 1) {
            if next.config.locked() && next.config.address_mode() == RiscvPmpAddressMode::Tor {
                return Err(RiscvPmpError::NextTorEntryLocked {
                    index,
                    next: index + 1,
                });
            }
        }
        self.entries[index].raw_addr = raw_addr;
        self.rebuild_ranges()
    }

    pub fn reset(&mut self) -> Result<(), RiscvPmpError> {
        for entry in &mut self.entries {
            entry.config = entry.config.reset_address_mode_and_lock();
        }
        self.rebuild_ranges()
    }

    pub fn check_access(
        &self,
        address: u64,
        size: u64,
        kind: RiscvPmpAccessKind,
        privilege: RiscvPrivilegeMode,
    ) -> Result<(), RiscvPmpError> {
        if size == 0 {
            return Err(RiscvPmpError::ZeroAccessSize { address });
        }

        if self.active_rule_count == 0 {
            return self.default_access(address, size, kind, privilege);
        }

        for (index, entry) in self.entries.iter().enumerate() {
            if entry
                .range
                .map(|range| range.contains_access(address, size))
                .transpose()?
                .unwrap_or(false)
            {
                if privilege == RiscvPrivilegeMode::Machine && !entry.config.locked() {
                    return Ok(());
                }
                if entry.config.permits(kind) {
                    return Ok(());
                }
                return Err(RiscvPmpError::AccessDenied {
                    address,
                    size,
                    kind,
                    privilege,
                    matched_entry: Some(index),
                });
            }
        }

        self.default_access(address, size, kind, privilege)
    }

    pub fn snapshot(&self) -> RiscvPmpSnapshot {
        RiscvPmpSnapshot {
            entries: self.entries.iter().map(RiscvPmpEntry::snapshot).collect(),
        }
    }

    pub fn restore(&mut self, snapshot: &RiscvPmpSnapshot) -> Result<(), RiscvPmpError> {
        if snapshot.entries.len() != self.entries.len() {
            return Err(RiscvPmpError::SnapshotEntryCountMismatch {
                expected: self.entries.len(),
                actual: snapshot.entries.len(),
            });
        }

        let mut restored = Vec::with_capacity(snapshot.entries.len());
        for entry in &snapshot.entries {
            restored.push(RiscvPmpEntry {
                raw_addr: entry.raw_addr,
                config: entry.config,
                range: None,
            });
        }

        let active_rule_count = rebuild_ranges(&mut restored)?;
        self.entries = restored;
        self.active_rule_count = active_rule_count;
        Ok(())
    }

    fn default_access(
        &self,
        address: u64,
        size: u64,
        kind: RiscvPmpAccessKind,
        privilege: RiscvPrivilegeMode,
    ) -> Result<(), RiscvPmpError> {
        if privilege == RiscvPrivilegeMode::Machine || self.entries.is_empty() {
            Ok(())
        } else {
            Err(RiscvPmpError::AccessDenied {
                address,
                size,
                kind,
                privilege,
                matched_entry: None,
            })
        }
    }

    fn ensure_entry_mutable(&self, index: usize) -> Result<(), RiscvPmpError> {
        let entry = self.entry(index)?;
        if entry.config.locked() {
            Err(RiscvPmpError::EntryLocked { index })
        } else {
            Ok(())
        }
    }

    fn rebuild_ranges(&mut self) -> Result<(), RiscvPmpError> {
        self.active_rule_count = rebuild_ranges(&mut self.entries)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvPmpError {
    EntryCountTooLarge {
        entries: usize,
        max: usize,
    },
    EntryIndexOutOfRange {
        index: usize,
        entries: usize,
    },
    EntryLocked {
        index: usize,
    },
    NextTorEntryLocked {
        index: usize,
        next: usize,
    },
    InvalidRange {
        start: u64,
        end: u64,
    },
    AddressOverflow {
        address: u64,
        size: u64,
    },
    ZeroAccessSize {
        address: u64,
    },
    SnapshotEntryCountMismatch {
        expected: usize,
        actual: usize,
    },
    AccessDenied {
        address: u64,
        size: u64,
        kind: RiscvPmpAccessKind,
        privilege: RiscvPrivilegeMode,
        matched_entry: Option<usize>,
    },
}

impl fmt::Display for RiscvPmpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EntryCountTooLarge { entries, max } => {
                write!(formatter, "RISC-V PMP entry count {entries} exceeds {max}")
            }
            Self::EntryIndexOutOfRange { index, entries } => write!(
                formatter,
                "RISC-V PMP entry index {index} is outside {entries} entries"
            ),
            Self::EntryLocked { index } => {
                write!(formatter, "RISC-V PMP entry {index} is locked")
            }
            Self::NextTorEntryLocked { index, next } => write!(
                formatter,
                "RISC-V PMP entry {index} cannot update address because locked TOR entry {next} uses it as a lower bound"
            ),
            Self::InvalidRange { start, end } => write!(
                formatter,
                "RISC-V PMP range start {start:#x} must be below end {end:#x}"
            ),
            Self::AddressOverflow { address, size } => write!(
                formatter,
                "RISC-V PMP access at {address:#x} with {size} byte(s) overflows"
            ),
            Self::ZeroAccessSize { address } => {
                write!(formatter, "RISC-V PMP access at {address:#x} has zero size")
            }
            Self::SnapshotEntryCountMismatch { expected, actual } => write!(
                formatter,
                "RISC-V PMP snapshot has {actual} entries, expected {expected}"
            ),
            Self::AccessDenied {
                address,
                size,
                kind,
                privilege,
                matched_entry,
            } => write!(
                formatter,
                "RISC-V PMP denied {kind:?} access at {address:#x} with {size} byte(s) for {privilege:?} mode at entry {matched_entry:?}"
            ),
        }
    }
}

impl Error for RiscvPmpError {}

fn validate_entry_count(entry_count: usize) -> Result<(), RiscvPmpError> {
    if entry_count > DEFAULT_MAX_PMP_ENTRIES {
        return Err(RiscvPmpError::EntryCountTooLarge {
            entries: entry_count,
            max: DEFAULT_MAX_PMP_ENTRIES,
        });
    }
    Ok(())
}

fn rebuild_ranges(entries: &mut [RiscvPmpEntry]) -> Result<usize, RiscvPmpError> {
    let raw_addrs: Vec<u64> = entries.iter().map(|entry| entry.raw_addr).collect();
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.range = decode_range(index, entry.config.address_mode(), &raw_addrs)?;
    }
    Ok(entries
        .iter()
        .filter(|entry| entry.config.address_mode() != RiscvPmpAddressMode::Off)
        .count())
}

fn decode_range(
    index: usize,
    address_mode: RiscvPmpAddressMode,
    raw_addrs: &[u64],
) -> Result<Option<RiscvPmpRange>, RiscvPmpError> {
    let raw_addr = raw_addrs[index];
    let range = match address_mode {
        RiscvPmpAddressMode::Off => return Ok(None),
        RiscvPmpAddressMode::Tor => {
            let start = index
                .checked_sub(1)
                .and_then(|previous| raw_addrs.get(previous))
                .copied()
                .unwrap_or(0)
                << 2;
            let end = raw_addr << 2;
            if start >= end {
                return Ok(None);
            }
            RiscvPmpRange::new(start, end)?
        }
        RiscvPmpAddressMode::Na4 => {
            let start = raw_addr << 2;
            let Some(end) = start.checked_add(4) else {
                return Err(RiscvPmpError::AddressOverflow {
                    address: start,
                    size: 4,
                });
            };
            RiscvPmpRange::new(start, end)?
        }
        RiscvPmpAddressMode::Napot => decode_napot_range(raw_addr)?,
    };
    Ok(Some(range))
}

fn decode_napot_range(raw_addr: u64) -> Result<RiscvPmpRange, RiscvPmpError> {
    if raw_addr == u64::MAX {
        return RiscvPmpRange::new(0, u64::MAX);
    }
    let trailing_ones = (!raw_addr).trailing_zeros();
    let size = 1_u64 << (trailing_ones + 3);
    let base = (raw_addr & !((1_u64 << trailing_ones) - 1)) << 2;
    RiscvPmpRange::new(base, base + size)
}

const fn set_bit(bits: u8, mask: u8, enabled: bool) -> u8 {
    if enabled {
        bits | mask
    } else {
        bits & !mask
    }
}
