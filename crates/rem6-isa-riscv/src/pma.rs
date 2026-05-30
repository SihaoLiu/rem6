use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RiscvPmaAccessKind {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RiscvPmaRange {
    start: u64,
    end: u64,
}

impl RiscvPmaRange {
    pub const fn new(start: u64, end: u64) -> Result<Self, RiscvPmaError> {
        if start >= end {
            return Err(RiscvPmaError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }

    fn contains_access(self, address: u64, size: u64) -> Result<bool, RiscvPmaError> {
        let Some(end) = address.checked_add(size) else {
            return Err(RiscvPmaError::AddressOverflow { address, size });
        };
        Ok(address >= self.start && end <= self.end)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RiscvPmaTable {
    misaligned_ranges: Vec<RiscvPmaRange>,
    uncacheable_ranges: Vec<RiscvPmaRange>,
}

impl RiscvPmaTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_misaligned_range(&mut self, range: RiscvPmaRange) -> Result<(), RiscvPmaError> {
        self.misaligned_ranges.push(range);
        self.misaligned_ranges.sort_by_key(|range| range.start());
        Ok(())
    }

    pub fn add_uncacheable_range(&mut self, range: RiscvPmaRange) -> Result<(), RiscvPmaError> {
        self.uncacheable_ranges.push(range);
        self.uncacheable_ranges.sort_by_key(|range| range.start());
        Ok(())
    }

    pub fn misaligned_ranges(&self) -> &[RiscvPmaRange] {
        &self.misaligned_ranges
    }

    pub fn uncacheable_ranges(&self) -> &[RiscvPmaRange] {
        &self.uncacheable_ranges
    }

    pub fn is_uncacheable(&self, address: u64, size: u64) -> Result<bool, RiscvPmaError> {
        validate_access_extent(address, size)?;
        for range in self.uncacheable_ranges.iter().copied() {
            if range.contains_access(address, size)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn check_data_alignment(
        &self,
        address: u64,
        size: u64,
        kind: RiscvPmaAccessKind,
    ) -> Result<(), RiscvPmaError> {
        validate_access_extent(address, size)?;

        if address.is_multiple_of(size) {
            return Ok(());
        }
        for range in self.misaligned_ranges.iter().copied() {
            if range.contains_access(address, size)? {
                return Ok(());
            }
        }

        Err(RiscvPmaError::MisalignedDataAccess {
            address,
            size,
            kind,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvPmaError {
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
    MisalignedDataAccess {
        address: u64,
        size: u64,
        kind: RiscvPmaAccessKind,
    },
}

impl fmt::Display for RiscvPmaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange { start, end } => write!(
                formatter,
                "RISC-V PMA range start {start:#x} must be below end {end:#x}"
            ),
            Self::AddressOverflow { address, size } => write!(
                formatter,
                "RISC-V PMA access at {address:#x} with {size} byte(s) overflows"
            ),
            Self::ZeroAccessSize { address } => {
                write!(formatter, "RISC-V PMA access at {address:#x} has zero size")
            }
            Self::MisalignedDataAccess {
                address,
                size,
                kind,
            } => write!(
                formatter,
                "RISC-V PMA denied misaligned {kind:?} access at {address:#x} with {size} byte(s)"
            ),
        }
    }
}

impl Error for RiscvPmaError {}

fn validate_access_extent(address: u64, size: u64) -> Result<(), RiscvPmaError> {
    if size == 0 {
        return Err(RiscvPmaError::ZeroAccessSize { address });
    }
    address
        .checked_add(size)
        .ok_or(RiscvPmaError::AddressOverflow { address, size })?;
    Ok(())
}
