use std::error::Error;
use std::fmt;

use rem6_memory::{Address, MemoryError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficGeneratorError {
    EmptyAddressRange {
        start: Address,
        end: Address,
    },
    BlockSizeExceedsRange {
        block_size: u64,
        range_size: u64,
    },
    BlockSizeExceedsCacheLine {
        block_size: u64,
        cache_line_size: u64,
    },
    BlockSizeDoesNotDivideRange {
        block_size: u64,
        range_size: u64,
    },
    InvalidReadPercent {
        read_percent: u8,
    },
    InvertedPeriod {
        min_period: u64,
        max_period: u64,
    },
    TickOverflow {
        tick: u64,
        delta: u64,
    },
    CounterOverflow {
        counter: &'static str,
        value: u64,
        increment: u64,
    },
    SnapshotCursorOutsideRange {
        next_address: Address,
        start: Address,
        end: Address,
    },
    SnapshotCursorOutsideBlockGrid {
        next_address: Address,
        start: Address,
        block_size: u64,
    },
    Memory(MemoryError),
}

impl From<MemoryError> for TrafficGeneratorError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}

impl fmt::Display for TrafficGeneratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAddressRange { start, end } => write!(
                formatter,
                "traffic generator address range {:#x}..{:#x} is empty",
                start.get(),
                end.get()
            ),
            Self::BlockSizeExceedsRange {
                block_size,
                range_size,
            } => write!(
                formatter,
                "traffic generator block size {block_size} exceeds range size {range_size}"
            ),
            Self::BlockSizeExceedsCacheLine {
                block_size,
                cache_line_size,
            } => write!(
                formatter,
                "traffic generator block size {block_size} exceeds cache line size {cache_line_size}"
            ),
            Self::BlockSizeDoesNotDivideRange {
                block_size,
                range_size,
            } => write!(
                formatter,
                "traffic generator block size {block_size} does not divide range size {range_size}"
            ),
            Self::InvalidReadPercent { read_percent } => write!(
                formatter,
                "traffic generator read percentage {read_percent} exceeds 100"
            ),
            Self::InvertedPeriod {
                min_period,
                max_period,
            } => write!(
                formatter,
                "traffic generator period range {min_period}..={max_period} is inverted"
            ),
            Self::TickOverflow { tick, delta } => write!(
                formatter,
                "traffic generator tick {tick} cannot advance by {delta}"
            ),
            Self::CounterOverflow {
                counter,
                value,
                increment,
            } => write!(
                formatter,
                "traffic generator counter {counter} with value {value} cannot advance by {increment}"
            ),
            Self::SnapshotCursorOutsideRange {
                next_address,
                start,
                end,
            } => write!(
                formatter,
                "traffic generator snapshot cursor {:#x} is outside {:#x}..{:#x}",
                next_address.get(),
                start.get(),
                end.get()
            ),
            Self::SnapshotCursorOutsideBlockGrid {
                next_address,
                start,
                block_size,
            } => write!(
                formatter,
                "traffic generator snapshot cursor {:#x} is not on {:#x} plus {block_size}-byte block grid",
                next_address.get(),
                start.get()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TrafficGeneratorError {}
