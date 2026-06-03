use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::indexing::CacheIndexingPolicyError;
use crate::replacement::{CacheReplacementPolicyError, CacheReplacementPolicyKind};

use super::CacheCompressedTagsConfig;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheCompressedTagsError {
    ZeroMaxCompressionRatio,
    MaxCompressionRatioNotPowerOfTwo {
        ratio: usize,
    },
    LineSizeTooSmall {
        bytes: u64,
    },
    SuperblockSpanTooLarge {
        line_bytes: u64,
        max_compression_ratio: usize,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    IndexingPolicyConfig {
        source: CacheIndexingPolicyError,
    },
    ReplacementPolicyConfig {
        source: CacheReplacementPolicyError,
    },
    UnsupportedReplacementPolicy {
        kind: CacheReplacementPolicyKind,
    },
    ReplacementPolicyState {
        source: CacheReplacementPolicyError,
    },
    UnknownSet {
        set: usize,
        sets: usize,
    },
    UnknownWay {
        way: usize,
        ways: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<CacheCompressedTagsConfig>,
        actual: Box<CacheCompressedTagsConfig>,
    },
    SnapshotSetCountMismatch {
        sets: usize,
        expected_sets: usize,
    },
    SnapshotWayCountMismatch {
        set: usize,
        ways: usize,
        expected_ways: usize,
    },
    SnapshotBlockCountMismatch {
        set: usize,
        way: usize,
        blocks: usize,
        expected_blocks: usize,
    },
    SnapshotInvalidCompressionFactor {
        set: usize,
        way: usize,
        factor: usize,
        max_factor: usize,
    },
    SnapshotSuperblockCapacityExceeded {
        set: usize,
        way: usize,
        valid_blocks: usize,
        compression_factor: usize,
    },
    SnapshotEmptySuperblock {
        set: usize,
        way: usize,
        compression_factor: usize,
        has_superblock_base: bool,
    },
    SnapshotCompressedSizeTooLarge {
        set: usize,
        way: usize,
        offset: usize,
        compressed_size_bits: usize,
        maximum_bits: usize,
    },
    SnapshotCompressedFlagMismatch {
        set: usize,
        way: usize,
        offset: usize,
        compressed: bool,
        expected_compressed: bool,
    },
    SnapshotLineWithoutSuperblock {
        set: usize,
        way: usize,
        offset: usize,
    },
    SnapshotMisalignedSuperblock {
        superblock_base: Address,
    },
    SnapshotSuperblockSetMismatch {
        superblock_base: Address,
        set: usize,
        expected_set: usize,
    },
    SnapshotMisalignedLine {
        line: Address,
    },
    SnapshotLineSuperblockMismatch {
        line: Address,
        superblock_base: Address,
        expected_superblock_base: Address,
    },
    SnapshotLineOffsetMismatch {
        line: Address,
        offset: usize,
        expected_offset: usize,
    },
    SnapshotDuplicateLine {
        line: Address,
    },
    SnapshotDuplicateSuperblock {
        superblock_base: Address,
    },
}

impl fmt::Display for CacheCompressedTagsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaxCompressionRatio => {
                write!(
                    formatter,
                    "cache compressed tags have no max compression ratio"
                )
            }
            Self::MaxCompressionRatioNotPowerOfTwo { ratio } => write!(
                formatter,
                "cache compressed tags need a power-of-two max compression ratio, got {ratio}"
            ),
            Self::LineSizeTooSmall { bytes } => write!(
                formatter,
                "cache compressed tags need a cache line size of at least 4 bytes, got {bytes}"
            ),
            Self::SuperblockSpanTooLarge {
                line_bytes,
                max_compression_ratio,
            } => write!(
                formatter,
                "cache compressed superblock span with line size {line_bytes} and max compression ratio {max_compression_ratio} is too large"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "cache compressed tags {field} length {length} exceeds vector allocation limit {maximum}"
            ),
            Self::IndexingPolicyConfig { source } => {
                write!(
                    formatter,
                    "cache compressed tag indexing config is invalid: {source}"
                )
            }
            Self::ReplacementPolicyConfig { source } => write!(
                formatter,
                "cache compressed tag replacement config is invalid: {source}"
            ),
            Self::UnsupportedReplacementPolicy { kind } => write!(
                formatter,
                "cache compressed tags do not support replacement policy {kind:?}"
            ),
            Self::ReplacementPolicyState { source } => write!(
                formatter,
                "cache compressed tag replacement state is invalid: {source}"
            ),
            Self::UnknownSet { set, sets } => write!(
                formatter,
                "cache compressed tag set {set} is outside {sets} sets"
            ),
            Self::UnknownWay { way, ways } => write!(
                formatter,
                "cache compressed tag way {way} is outside {ways} ways"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "cache compressed tag snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSetCountMismatch {
                sets,
                expected_sets,
            } => write!(
                formatter,
                "cache compressed tag snapshot has {sets} sets instead of {expected_sets}"
            ),
            Self::SnapshotWayCountMismatch {
                set,
                ways,
                expected_ways,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} has {ways} ways instead of {expected_ways}"
            ),
            Self::SnapshotBlockCountMismatch {
                set,
                way,
                blocks,
                expected_blocks,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} has {blocks} blocks instead of {expected_blocks}"
            ),
            Self::SnapshotInvalidCompressionFactor {
                set,
                way,
                factor,
                max_factor,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} has compression factor {factor} outside 1..={max_factor}"
            ),
            Self::SnapshotSuperblockCapacityExceeded {
                set,
                way,
                valid_blocks,
                compression_factor,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} has {valid_blocks} valid blocks for compression factor {compression_factor}"
            ),
            Self::SnapshotEmptySuperblock {
                set,
                way,
                compression_factor,
                has_superblock_base,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} has no valid blocks, compression factor {compression_factor}, and superblock base present {has_superblock_base}"
            ),
            Self::SnapshotCompressedSizeTooLarge {
                set,
                way,
                offset,
                compressed_size_bits,
                maximum_bits,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} offset {offset} has compressed size {compressed_size_bits} bits above {maximum_bits} bits"
            ),
            Self::SnapshotCompressedFlagMismatch {
                set,
                way,
                offset,
                compressed,
                expected_compressed,
            } => write!(
                formatter,
                "cache compressed tag snapshot set {set} way {way} offset {offset} has compressed flag {compressed} instead of {expected_compressed}"
            ),
            Self::SnapshotLineWithoutSuperblock { set, way, offset } => write!(
                formatter,
                "cache compressed tag snapshot has a line in set {set} way {way} offset {offset} without a superblock"
            ),
            Self::SnapshotMisalignedSuperblock { superblock_base } => write!(
                formatter,
                "cache compressed tag snapshot superblock {:#x} is not superblock-aligned",
                superblock_base.get()
            ),
            Self::SnapshotSuperblockSetMismatch {
                superblock_base,
                set,
                expected_set,
            } => write!(
                formatter,
                "cache compressed tag snapshot superblock {:#x} is in set {set} instead of {expected_set}",
                superblock_base.get()
            ),
            Self::SnapshotMisalignedLine { line } => write!(
                formatter,
                "cache compressed tag snapshot line {:#x} is not cache-line aligned",
                line.get()
            ),
            Self::SnapshotLineSuperblockMismatch {
                line,
                superblock_base,
                expected_superblock_base,
            } => write!(
                formatter,
                "cache compressed tag snapshot line {:#x} is in superblock {:#x} instead of {:#x}",
                line.get(),
                superblock_base.get(),
                expected_superblock_base.get()
            ),
            Self::SnapshotLineOffsetMismatch {
                line,
                offset,
                expected_offset,
            } => write!(
                formatter,
                "cache compressed tag snapshot line {:#x} is at offset {offset} instead of {expected_offset}",
                line.get()
            ),
            Self::SnapshotDuplicateLine { line } => write!(
                formatter,
                "cache compressed tag snapshot repeats line {:#x}",
                line.get()
            ),
            Self::SnapshotDuplicateSuperblock { superblock_base } => write!(
                formatter,
                "cache compressed tag snapshot repeats superblock {:#x}",
                superblock_base.get()
            ),
        }
    }
}

impl Error for CacheCompressedTagsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::IndexingPolicyConfig { source } => Some(source),
            Self::ReplacementPolicyConfig { source } | Self::ReplacementPolicyState { source } => {
                Some(source)
            }
            Self::ZeroMaxCompressionRatio
            | Self::MaxCompressionRatioNotPowerOfTwo { .. }
            | Self::LineSizeTooSmall { .. }
            | Self::SuperblockSpanTooLarge { .. }
            | Self::VectorLengthTooLarge { .. }
            | Self::UnsupportedReplacementPolicy { .. }
            | Self::UnknownSet { .. }
            | Self::UnknownWay { .. }
            | Self::SnapshotConfigMismatch { .. }
            | Self::SnapshotSetCountMismatch { .. }
            | Self::SnapshotWayCountMismatch { .. }
            | Self::SnapshotBlockCountMismatch { .. }
            | Self::SnapshotInvalidCompressionFactor { .. }
            | Self::SnapshotSuperblockCapacityExceeded { .. }
            | Self::SnapshotEmptySuperblock { .. }
            | Self::SnapshotCompressedSizeTooLarge { .. }
            | Self::SnapshotCompressedFlagMismatch { .. }
            | Self::SnapshotLineWithoutSuperblock { .. }
            | Self::SnapshotMisalignedSuperblock { .. }
            | Self::SnapshotSuperblockSetMismatch { .. }
            | Self::SnapshotMisalignedLine { .. }
            | Self::SnapshotLineSuperblockMismatch { .. }
            | Self::SnapshotLineOffsetMismatch { .. }
            | Self::SnapshotDuplicateLine { .. }
            | Self::SnapshotDuplicateSuperblock { .. } => None,
        }
    }
}
