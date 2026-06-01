use std::error::Error;
use std::fmt;

use crate::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryAtomicOp, MemoryRequestId,
    MemoryTargetId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryError {
    ZeroAccessSize,
    AccessSizeTooLarge {
        size: AccessSize,
    },
    AddressOverflow {
        start: Address,
        size: AccessSize,
    },
    ZeroCacheLineSize,
    NonPowerOfTwoCacheLineSize {
        bytes: u64,
    },
    PayloadSizeMismatch {
        expected: AccessSize,
        actual: u64,
    },
    ByteMaskSizeMismatch {
        expected: AccessSize,
        actual: u64,
    },
    MissingRequestData {
        request: MemoryRequestId,
    },
    UnexpectedRequestData {
        request: MemoryRequestId,
    },
    MissingByteMask {
        request: MemoryRequestId,
    },
    UnexpectedByteMask {
        request: MemoryRequestId,
    },
    MissingAtomicOp {
        request: MemoryRequestId,
    },
    UnexpectedAtomicOp {
        request: MemoryRequestId,
    },
    UnsupportedAtomicAccessSize {
        request: MemoryRequestId,
        op: MemoryAtomicOp,
        size: AccessSize,
    },
    InvalidRequestStrictOrdering {
        request: MemoryRequestId,
    },
    InvalidRequestCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidRequestCheckpointMagic,
    UnsupportedRequestCheckpointVersion {
        version: u32,
    },
    InvalidRequestCheckpointReserved {
        value: u32,
    },
    InvalidRequestCheckpointFlags {
        flags: u32,
    },
    InvalidRequestCheckpointOperation {
        code: u32,
    },
    InvalidRequestCheckpointAtomicOp {
        code: u32,
    },
    InvalidRequestCheckpointUsize {
        value: u64,
    },
    RequestCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    InvalidRequestCheckpointDataLength {
        length: u64,
    },
    InvalidRequestCheckpointMaskLength {
        length: u64,
    },
    InvalidRequestCheckpointMaskBit {
        value: u8,
    },
    InvalidResponseCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidResponseCheckpointMagic,
    UnsupportedResponseCheckpointVersion {
        version: u32,
    },
    InvalidResponseCheckpointReserved {
        value: u32,
    },
    InvalidResponseCheckpointFlags {
        flags: u32,
    },
    InvalidResponseCheckpointStatus {
        code: u32,
    },
    InvalidResponseCheckpointUsize {
        value: u64,
    },
    ResponseCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    InvalidResponseCheckpointDataLength {
        length: u64,
    },
    UnalignedLineAddress {
        address: Address,
        line_size: u64,
    },
    LineLayoutMismatch {
        request: MemoryRequestId,
        expected: CacheLineLayout,
        actual: CacheLineLayout,
    },
    CrossLineAccess {
        request: MemoryRequestId,
        start: Address,
        size: AccessSize,
        line_size: u64,
    },
    UnmappedLine {
        line: Address,
    },
    DuplicateMemoryLine {
        line: Address,
    },
    InvalidLineCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidLineCheckpointMagic,
    UnsupportedLineCheckpointVersion {
        version: u32,
    },
    InvalidLineCheckpointReserved {
        value: u32,
    },
    InvalidLineCheckpointLineSize {
        value: u64,
    },
    LineCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    InvalidPartitionCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidPartitionCheckpointMagic,
    UnsupportedPartitionCheckpointVersion {
        version: u32,
    },
    InvalidPartitionCheckpointReserved {
        value: u32,
    },
    InvalidPartitionCheckpointInterleaveFlag {
        value: u32,
    },
    InvalidPartitionCheckpointUsize {
        value: u64,
    },
    PartitionCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    InvalidDecoderCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidDecoderCheckpointMagic,
    UnsupportedDecoderCheckpointVersion {
        version: u32,
    },
    InvalidDecoderCheckpointReserved {
        value: u32,
    },
    InvalidDecoderCheckpointInterleaveFlag {
        value: u32,
    },
    DecoderCheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    UnmappedAddress {
        address: Address,
    },
    OverlappingAddressRegion {
        existing: AddressRange,
        requested: AddressRange,
    },
    SparseHoleOutsideRange {
        base: AddressRange,
        hole: AddressRange,
    },
    OverlappingSparseHole {
        existing: AddressRange,
        requested: AddressRange,
    },
    ZeroInterleaveStripes,
    InterleaveMatchOutOfRange {
        stripes: u32,
        match_index: u32,
    },
    RequestCrossesAddressRegion {
        request: MemoryRequestId,
        range: AddressRange,
    },
    AccessCrossesAddressRegion {
        range: AddressRange,
    },
    DuplicateMemoryTarget {
        target: MemoryTargetId,
    },
    UnknownMemoryTarget {
        target: MemoryTargetId,
    },
    MissingResponseData {
        request: MemoryRequestId,
    },
    UnexpectedResponseData {
        request: MemoryRequestId,
    },
    InvalidResponseDataLength {
        request: MemoryRequestId,
        length: usize,
    },
    ResponseNotExpected {
        request: MemoryRequestId,
    },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroAccessSize => write!(formatter, "memory access size must be nonzero"),
            Self::AccessSizeTooLarge { size } => {
                write!(
                    formatter,
                    "access size {} does not fit host usize",
                    size.bytes()
                )
            }
            Self::AddressOverflow { start, size } => write!(
                formatter,
                "address {:#x} overflows for {} bytes",
                start.get(),
                size.bytes()
            ),
            Self::ZeroCacheLineSize => write!(formatter, "cache line size must be nonzero"),
            Self::NonPowerOfTwoCacheLineSize { bytes } => {
                write!(formatter, "cache line size {bytes} is not a power of two")
            }
            Self::PayloadSizeMismatch { expected, actual } => write!(
                formatter,
                "payload has {actual} bytes but request expects {}",
                expected.bytes()
            ),
            Self::ByteMaskSizeMismatch { expected, actual } => write!(
                formatter,
                "byte mask has {actual} bits but request expects {}",
                expected.bytes()
            ),
            Self::MissingRequestData { request } => write!(
                formatter,
                "request {} from agent {} requires payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedRequestData { request } => write!(
                formatter,
                "request {} from agent {} must not carry payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingByteMask { request } => write!(
                formatter,
                "request {} from agent {} requires a byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedByteMask { request } => write!(
                formatter,
                "request {} from agent {} must not carry a byte mask",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingAtomicOp { request } => write!(
                formatter,
                "request {} from agent {} requires an atomic operation",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedAtomicOp { request } => write!(
                formatter,
                "request {} from agent {} must not carry an atomic operation",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnsupportedAtomicAccessSize { request, op, size } => write!(
                formatter,
                "request {} from agent {} uses unsupported {op:?} atomic size {}",
                request.sequence(),
                request.agent().get(),
                size.bytes()
            ),
            Self::InvalidRequestStrictOrdering { request } => write!(
                formatter,
                "request {} from agent {} cannot be strict-ordered unless it is uncacheable",
                request.sequence(),
                request.agent().get()
            ),
            Self::InvalidRequestCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "memory-request checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidRequestCheckpointMagic => write!(
                formatter,
                "memory-request checkpoint payload has invalid magic"
            ),
            Self::UnsupportedRequestCheckpointVersion { version } => write!(
                formatter,
                "memory-request checkpoint payload version {version} is not supported"
            ),
            Self::InvalidRequestCheckpointReserved { value } => write!(
                formatter,
                "memory-request checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidRequestCheckpointFlags { flags } => write!(
                formatter,
                "memory-request checkpoint flags {flags:#x} are invalid"
            ),
            Self::InvalidRequestCheckpointOperation { code } => write!(
                formatter,
                "memory-request checkpoint payload has invalid operation code {code}"
            ),
            Self::InvalidRequestCheckpointAtomicOp { code } => write!(
                formatter,
                "memory-request checkpoint payload has invalid atomic operation code {code}"
            ),
            Self::InvalidRequestCheckpointUsize { value } => write!(
                formatter,
                "memory-request checkpoint usize value {value} cannot fit this target"
            ),
            Self::RequestCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "memory-request checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::InvalidRequestCheckpointDataLength { length } => write!(
                formatter,
                "memory-request checkpoint has absent data with nonzero length {length}"
            ),
            Self::InvalidRequestCheckpointMaskLength { length } => write!(
                formatter,
                "memory-request checkpoint has absent byte mask with nonzero length {length}"
            ),
            Self::InvalidRequestCheckpointMaskBit { value } => write!(
                formatter,
                "memory-request checkpoint byte mask has invalid bit value {value}"
            ),
            Self::InvalidResponseCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "memory-response checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidResponseCheckpointMagic => write!(
                formatter,
                "memory-response checkpoint payload has invalid magic"
            ),
            Self::UnsupportedResponseCheckpointVersion { version } => write!(
                formatter,
                "memory-response checkpoint payload version {version} is not supported"
            ),
            Self::InvalidResponseCheckpointReserved { value } => write!(
                formatter,
                "memory-response checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidResponseCheckpointFlags { flags } => write!(
                formatter,
                "memory-response checkpoint flags {flags:#x} are invalid"
            ),
            Self::InvalidResponseCheckpointStatus { code } => write!(
                formatter,
                "memory-response checkpoint payload has invalid status code {code}"
            ),
            Self::InvalidResponseCheckpointUsize { value } => write!(
                formatter,
                "memory-response checkpoint usize value {value} cannot fit this target"
            ),
            Self::ResponseCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "memory-response checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::InvalidResponseCheckpointDataLength { length } => write!(
                formatter,
                "memory-response checkpoint has absent data with nonzero length {length}"
            ),
            Self::UnalignedLineAddress { address, line_size } => write!(
                formatter,
                "address {:#x} is not aligned to cache line size {line_size}",
                address.get()
            ),
            Self::LineLayoutMismatch {
                request,
                expected,
                actual,
            } => write!(
                formatter,
                "request {} from agent {} uses {}-byte lines but target expects {}-byte lines",
                request.sequence(),
                request.agent().get(),
                actual.bytes(),
                expected.bytes()
            ),
            Self::CrossLineAccess {
                request,
                start,
                size,
                line_size,
            } => write!(
                formatter,
                "request {} from agent {} crosses a {line_size}-byte line at {:#x}+{}",
                request.sequence(),
                request.agent().get(),
                start.get(),
                size.bytes()
            ),
            Self::UnmappedLine { line } => {
                write!(formatter, "line {:#x} is not mapped", line.get())
            }
            Self::DuplicateMemoryLine { line } => {
                write!(formatter, "line {:#x} appears more than once", line.get())
            }
            Self::InvalidLineCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "line-memory checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidLineCheckpointMagic => {
                write!(
                    formatter,
                    "line-memory checkpoint payload has invalid magic"
                )
            }
            Self::UnsupportedLineCheckpointVersion { version } => write!(
                formatter,
                "line-memory checkpoint payload version {version} is not supported"
            ),
            Self::InvalidLineCheckpointReserved { value } => write!(
                formatter,
                "line-memory checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidLineCheckpointLineSize { value } => write!(
                formatter,
                "line-memory checkpoint line size {value} cannot fit this target"
            ),
            Self::LineCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "line-memory checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::InvalidPartitionCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "partitioned-memory checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidPartitionCheckpointMagic => write!(
                formatter,
                "partitioned-memory checkpoint payload has invalid magic"
            ),
            Self::UnsupportedPartitionCheckpointVersion { version } => write!(
                formatter,
                "partitioned-memory checkpoint payload version {version} is not supported"
            ),
            Self::InvalidPartitionCheckpointReserved { value } => write!(
                formatter,
                "partitioned-memory checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidPartitionCheckpointInterleaveFlag { value } => write!(
                formatter,
                "partitioned-memory checkpoint interleave flag {value} is invalid"
            ),
            Self::InvalidPartitionCheckpointUsize { value } => write!(
                formatter,
                "partitioned-memory checkpoint usize value {value} cannot fit this target"
            ),
            Self::PartitionCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "partitioned-memory checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::InvalidDecoderCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "address-decoder checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::InvalidDecoderCheckpointMagic => write!(
                formatter,
                "address-decoder checkpoint payload has invalid magic"
            ),
            Self::UnsupportedDecoderCheckpointVersion { version } => write!(
                formatter,
                "address-decoder checkpoint payload version {version} is not supported"
            ),
            Self::InvalidDecoderCheckpointReserved { value } => write!(
                formatter,
                "address-decoder checkpoint reserved field has nonzero value {value}"
            ),
            Self::InvalidDecoderCheckpointInterleaveFlag { value } => write!(
                formatter,
                "address-decoder checkpoint interleave flag {value} is invalid"
            ),
            Self::DecoderCheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "address-decoder checkpoint {field} value {value} exceeds maximum {maximum}"
            ),
            Self::UnmappedAddress { address } => {
                write!(formatter, "address {:#x} is not mapped", address.get())
            }
            Self::OverlappingAddressRegion {
                existing,
                requested,
            } => write!(
                formatter,
                "address region {:#x}..{:#x} overlaps existing region {:#x}..{:#x}",
                requested.start().get(),
                requested.end().get(),
                existing.start().get(),
                existing.end().get()
            ),
            Self::SparseHoleOutsideRange { base, hole } => write!(
                formatter,
                "sparse address hole {:#x}..{:#x} is outside base region {:#x}..{:#x}",
                hole.start().get(),
                hole.end().get(),
                base.start().get(),
                base.end().get()
            ),
            Self::OverlappingSparseHole {
                existing,
                requested,
            } => write!(
                formatter,
                "sparse address hole {:#x}..{:#x} overlaps existing hole {:#x}..{:#x}",
                requested.start().get(),
                requested.end().get(),
                existing.start().get(),
                existing.end().get()
            ),
            Self::ZeroInterleaveStripes => {
                write!(formatter, "address interleave stripe count must be nonzero")
            }
            Self::InterleaveMatchOutOfRange {
                stripes,
                match_index,
            } => write!(
                formatter,
                "address interleave match index {match_index} is outside {stripes} stripes"
            ),
            Self::RequestCrossesAddressRegion { request, range } => write!(
                formatter,
                "request {} from agent {} crosses address region boundary at {:#x}..{:#x}",
                request.sequence(),
                request.agent().get(),
                range.start().get(),
                range.end().get()
            ),
            Self::AccessCrossesAddressRegion { range } => write!(
                formatter,
                "access crosses address region boundary at {:#x}..{:#x}",
                range.start().get(),
                range.end().get()
            ),
            Self::DuplicateMemoryTarget { target } => {
                write!(
                    formatter,
                    "memory target {} is already declared",
                    target.get()
                )
            }
            Self::UnknownMemoryTarget { target } => {
                write!(formatter, "memory target {} is not declared", target.get())
            }
            Self::MissingResponseData { request } => write!(
                formatter,
                "response to request {} from agent {} requires payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::UnexpectedResponseData { request } => write!(
                formatter,
                "response to request {} from agent {} must not carry payload data",
                request.sequence(),
                request.agent().get()
            ),
            Self::InvalidResponseDataLength { request, length } => write!(
                formatter,
                "response to request {} from agent {} has invalid payload length {length}",
                request.sequence(),
                request.agent().get()
            ),
            Self::ResponseNotExpected { request } => write!(
                formatter,
                "request {} from agent {} does not expect a response",
                request.sequence(),
                request.agent().get()
            ),
        }
    }
}

impl Error for MemoryError {}
