use std::error::Error;
use std::fmt;

use rem6_memory::{Address, MemoryError};

use crate::TrafficStateId;

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
    ZeroSuperblockSize,
    ZeroStrideSize,
    SuperblockSizeNotMultipleOfBlockSize {
        superblock_size: u64,
        block_size: u64,
    },
    OffsetNotMultipleOfSuperblock {
        offset: u64,
        superblock_size: u64,
    },
    StrideSizeNotMultipleOfSuperblock {
        stride_size: u64,
        superblock_size: u64,
    },
    StridedOffsetOutsideRange {
        next_address: Address,
        start: Address,
        end: Address,
    },
    AddressOverflow {
        label: &'static str,
        value: u64,
        increment: u64,
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
    TrafficStateGraphEmpty,
    TrafficStateDuplicate {
        state: TrafficStateId,
    },
    TrafficStateUnknownInitial {
        state: TrafficStateId,
    },
    TrafficStateUnknownTransition {
        state: TrafficStateId,
        role: &'static str,
    },
    TrafficStateDuplicateTransition {
        from: TrafficStateId,
        to: TrafficStateId,
    },
    TrafficStateTransitionRowSumMismatch {
        state: TrafficStateId,
        sum: u32,
        expected: u32,
    },
    TrafficTransitionProbabilityOutOfRange {
        probability: u32,
        scale: u32,
    },
    TrafficTransitionRatioZeroDenominator,
    TrafficStateSnapshotUnknownState {
        state: TrafficStateId,
    },
    TrafficStateSnapshotMissingCurrentState,
    TrafficConfigMissingInitial,
    TrafficConfigDuplicateInitial {
        line: usize,
    },
    TrafficConfigSparseStateIds {
        expected: u32,
        actual: TrafficStateId,
    },
    TrafficConfigUnknownKeyword {
        line: usize,
        keyword: String,
    },
    TrafficConfigUnknownStateMode {
        line: usize,
        mode: String,
    },
    TrafficConfigMissingToken {
        line: usize,
        record: &'static str,
        field: &'static str,
    },
    TrafficConfigUnexpectedToken {
        line: usize,
        record: &'static str,
        token: String,
    },
    TrafficConfigInvalidNumber {
        line: usize,
        field: &'static str,
        token: String,
    },
    TrafficConfigProbabilityTooPrecise {
        line: usize,
        token: String,
        scale: u32,
    },
    TrafficConfigReadPercentOutOfRange {
        line: usize,
        read_percent: u32,
    },
    TrafficControllerMissingStateGenerator {
        state: TrafficStateId,
    },
    TrafficControllerDuplicateStateGenerator {
        state: TrafficStateId,
    },
    TrafficControllerUnknownStateGenerator {
        state: TrafficStateId,
    },
    TraceTruncatedMagic {
        length: usize,
    },
    TraceBadMagic {
        actual: [u8; 4],
    },
    TraceMissingHeader,
    TraceTruncatedVarint {
        offset: usize,
    },
    TraceVarintTooLong {
        offset: usize,
    },
    TraceVarint32TooLong {
        offset: usize,
    },
    TraceMessageTooLarge {
        offset: usize,
        length: u64,
    },
    TraceTruncatedMessage {
        offset: usize,
        length: usize,
        remaining: usize,
    },
    TraceMissingField {
        message: &'static str,
        field: &'static str,
    },
    TraceTickFrequencyMismatch {
        expected: u64,
        actual: u64,
    },
    TraceUnsupportedCommand {
        command: u32,
    },
    TraceUnsupportedFlags {
        flags: u32,
    },
    TraceZeroSize,
    TraceInvalidFieldWireType {
        message: &'static str,
        field: &'static str,
        wire_type: u64,
    },
    TraceFieldOutOfRange {
        message: &'static str,
        field: &'static str,
        value: u64,
    },
    TraceInvalidFieldNumber,
    TraceFieldNumberTooLarge {
        number: u64,
    },
    TraceLengthDelimitedFieldTooLarge {
        offset: usize,
        length: u64,
    },
    TraceTruncatedField {
        offset: usize,
        length: usize,
        remaining: usize,
    },
    TraceUnsupportedWireType {
        wire_type: u64,
    },
    TraceInvalidWireType {
        wire_type: u64,
    },
    TraceSnapshotCursorOutsideTrace {
        cursor: usize,
        length: usize,
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
            Self::ZeroSuperblockSize => {
                write!(formatter, "traffic generator superblock size is zero")
            }
            Self::ZeroStrideSize => write!(formatter, "traffic generator stride size is zero"),
            Self::SuperblockSizeNotMultipleOfBlockSize {
                superblock_size,
                block_size,
            } => write!(
                formatter,
                "traffic generator superblock size {superblock_size} is not a multiple of block size {block_size}"
            ),
            Self::OffsetNotMultipleOfSuperblock {
                offset,
                superblock_size,
            } => write!(
                formatter,
                "traffic generator offset {offset} is not a multiple of superblock size {superblock_size}"
            ),
            Self::StrideSizeNotMultipleOfSuperblock {
                stride_size,
                superblock_size,
            } => write!(
                formatter,
                "traffic generator stride size {stride_size} is not a multiple of superblock size {superblock_size}"
            ),
            Self::StridedOffsetOutsideRange {
                next_address,
                start,
                end,
            } => write!(
                formatter,
                "traffic generator strided offset starts at {:#x}, outside {:#x}..{:#x}",
                next_address.get(),
                start.get(),
                end.get()
            ),
            Self::AddressOverflow {
                label,
                value,
                increment,
            } => write!(
                formatter,
                "traffic generator address {label} with value {value} cannot advance by {increment}"
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
            Self::TrafficStateGraphEmpty => {
                write!(formatter, "traffic state graph has no states")
            }
            Self::TrafficStateDuplicate { state } => write!(
                formatter,
                "traffic state graph defines duplicate state {}",
                state.get()
            ),
            Self::TrafficStateUnknownInitial { state } => write!(
                formatter,
                "traffic state graph initial state {} is not defined",
                state.get()
            ),
            Self::TrafficStateUnknownTransition { state, role } => write!(
                formatter,
                "traffic state graph transition {role} state {} is not defined",
                state.get()
            ),
            Self::TrafficStateDuplicateTransition { from, to } => write!(
                formatter,
                "traffic state graph defines duplicate transition {} -> {}",
                from.get(),
                to.get()
            ),
            Self::TrafficStateTransitionRowSumMismatch {
                state,
                sum,
                expected,
            } => write!(
                formatter,
                "traffic state graph transition probabilities from state {} sum to {sum}, expected {expected}",
                state.get()
            ),
            Self::TrafficTransitionProbabilityOutOfRange { probability, scale } => write!(
                formatter,
                "traffic transition probability {probability} exceeds scale {scale}"
            ),
            Self::TrafficTransitionRatioZeroDenominator => {
                write!(formatter, "traffic transition probability ratio has zero denominator")
            }
            Self::TrafficStateSnapshotUnknownState { state } => write!(
                formatter,
                "traffic state snapshot current state {} is not defined",
                state.get()
            ),
            Self::TrafficStateSnapshotMissingCurrentState => {
                write!(formatter, "traffic state snapshot is active without a current state")
            }
            Self::TrafficConfigMissingInitial => {
                write!(formatter, "traffic text config is missing INIT record")
            }
            Self::TrafficConfigDuplicateInitial { line } => {
                write!(
                    formatter,
                    "traffic text config line {line} defines duplicate INIT record"
                )
            }
            Self::TrafficConfigSparseStateIds { expected, actual } => write!(
                formatter,
                "traffic text config expected dense state id {expected}, found {}",
                actual.get()
            ),
            Self::TrafficConfigUnknownKeyword { line, keyword } => write!(
                formatter,
                "traffic text config line {line} has unknown keyword {keyword}"
            ),
            Self::TrafficConfigUnknownStateMode { line, mode } => write!(
                formatter,
                "traffic text config line {line} has unknown STATE mode {mode}"
            ),
            Self::TrafficConfigMissingToken {
                line,
                record,
                field,
            } => write!(
                formatter,
                "traffic text config line {line} {record} record is missing {field}"
            ),
            Self::TrafficConfigUnexpectedToken {
                line,
                record,
                token,
            } => write!(
                formatter,
                "traffic text config line {line} {record} record has unexpected token {token}"
            ),
            Self::TrafficConfigInvalidNumber { line, field, token } => write!(
                formatter,
                "traffic text config line {line} field {field} has invalid number {token}"
            ),
            Self::TrafficConfigProbabilityTooPrecise { line, token, scale } => write!(
                formatter,
                "traffic text config line {line} probability {token} exceeds fixed scale {scale}"
            ),
            Self::TrafficConfigReadPercentOutOfRange { line, read_percent } => write!(
                formatter,
                "traffic text config line {line} read percentage {read_percent} exceeds 100"
            ),
            Self::TrafficControllerMissingStateGenerator { state } => write!(
                formatter,
                "traffic controller has no generator for state {}",
                state.get()
            ),
            Self::TrafficControllerDuplicateStateGenerator { state } => write!(
                formatter,
                "traffic controller has duplicate generator for state {}",
                state.get()
            ),
            Self::TrafficControllerUnknownStateGenerator { state } => write!(
                formatter,
                "traffic controller has generator for unknown state {}",
                state.get()
            ),
            Self::TraceTruncatedMagic { length } => write!(
                formatter,
                "gem5 packet trace has {length} bytes before the magic header"
            ),
            Self::TraceBadMagic { actual } => write!(
                formatter,
                "gem5 packet trace magic {actual:02x?} does not match expected magic"
            ),
            Self::TraceMissingHeader => {
                write!(formatter, "gem5 packet trace is missing its PacketHeader")
            }
            Self::TraceTruncatedVarint { offset } => write!(
                formatter,
                "gem5 packet trace varint at byte offset {offset} is truncated"
            ),
            Self::TraceVarintTooLong { offset } => write!(
                formatter,
                "gem5 packet trace varint at byte offset {offset} exceeds 64 bits"
            ),
            Self::TraceVarint32TooLong { offset } => write!(
                formatter,
                "gem5 packet trace varint32 at byte offset {offset} exceeds 5 bytes"
            ),
            Self::TraceMessageTooLarge { offset, length } => write!(
                formatter,
                "gem5 packet trace message at byte offset {offset} has oversized length {length}"
            ),
            Self::TraceTruncatedMessage {
                offset,
                length,
                remaining,
            } => write!(
                formatter,
                "gem5 packet trace message at byte offset {offset} has length {length} but only {remaining} bytes remain"
            ),
            Self::TraceMissingField { message, field } => write!(
                formatter,
                "gem5 packet trace {message} message is missing required field {field}"
            ),
            Self::TraceTickFrequencyMismatch { expected, actual } => write!(
                formatter,
                "gem5 packet trace tick frequency {actual} does not match expected {expected}"
            ),
            Self::TraceUnsupportedCommand { command } => write!(
                formatter,
                "gem5 packet trace command {command} is not supported"
            ),
            Self::TraceUnsupportedFlags { flags } => write!(
                formatter,
                "gem5 packet trace flags {flags:#x} are not supported"
            ),
            Self::TraceZeroSize => write!(
                formatter,
                "gem5 packet trace packet has zero access size"
            ),
            Self::TraceInvalidFieldWireType {
                message,
                field,
                wire_type,
            } => write!(
                formatter,
                "gem5 packet trace {message}.{field} has protobuf wire type {wire_type}, expected varint"
            ),
            Self::TraceFieldOutOfRange {
                message,
                field,
                value,
            } => write!(
                formatter,
                "gem5 packet trace {message}.{field} value {value} exceeds u32 range"
            ),
            Self::TraceInvalidFieldNumber => write!(
                formatter,
                "gem5 packet trace protobuf field number zero is invalid"
            ),
            Self::TraceFieldNumberTooLarge { number } => write!(
                formatter,
                "gem5 packet trace protobuf field number {number} exceeds u32 range"
            ),
            Self::TraceLengthDelimitedFieldTooLarge { offset, length } => write!(
                formatter,
                "gem5 packet trace length-delimited field at byte offset {offset} has oversized length {length}"
            ),
            Self::TraceTruncatedField {
                offset,
                length,
                remaining,
            } => write!(
                formatter,
                "gem5 packet trace field at byte offset {offset} has length {length} but only {remaining} bytes remain"
            ),
            Self::TraceUnsupportedWireType { wire_type } => write!(
                formatter,
                "gem5 packet trace protobuf wire type {wire_type} is not supported"
            ),
            Self::TraceInvalidWireType { wire_type } => write!(
                formatter,
                "gem5 packet trace protobuf wire type {wire_type} is invalid"
            ),
            Self::TraceSnapshotCursorOutsideTrace { cursor, length } => write!(
                formatter,
                "traffic trace snapshot cursor {cursor} is outside trace length {length}"
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
