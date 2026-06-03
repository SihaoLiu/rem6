use rem6_memory::Address;

use crate::replacement::{ReplacementSetSnapshot, RANDOM_REPLACEMENT_INITIAL_STATE};

use super::{
    CacheCompressedTagLine, CacheCompressedTagReplacementState, CacheCompressedTagsConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagsSnapshot {
    pub(super) config: CacheCompressedTagsConfig,
    pub(super) tick: u64,
    pub(super) random_state: u64,
    pub(super) sets: Vec<CacheCompressedTagSetSnapshot>,
}

impl CacheCompressedTagsSnapshot {
    pub fn new(
        config: CacheCompressedTagsConfig,
        sets: Vec<CacheCompressedTagSetSnapshot>,
    ) -> Self {
        Self {
            config,
            tick: 0,
            random_state: RANDOM_REPLACEMENT_INITIAL_STATE,
            sets,
        }
    }

    pub const fn config(&self) -> &CacheCompressedTagsConfig {
        &self.config
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn sets(&self) -> &[CacheCompressedTagSetSnapshot] {
        &self.sets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagSetSnapshot {
    pub(super) entries: Vec<CacheCompressedTagEntrySnapshot>,
    pub(super) replacement: ReplacementSetSnapshot,
}

impl CacheCompressedTagSetSnapshot {
    pub fn new(
        entries: Vec<CacheCompressedTagEntrySnapshot>,
        replacement: ReplacementSetSnapshot,
    ) -> Self {
        Self {
            entries,
            replacement,
        }
    }

    pub fn entries(&self) -> &[CacheCompressedTagEntrySnapshot] {
        &self.entries
    }

    pub const fn replacement(&self) -> &ReplacementSetSnapshot {
        &self.replacement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheCompressedTagEntrySnapshot {
    pub(super) superblock_base: Option<Address>,
    pub(super) blocks: Vec<Option<CacheCompressedTagLine>>,
    pub(super) compression_factor: usize,
    pub(super) replacement_state: CacheCompressedTagReplacementState,
}

impl CacheCompressedTagEntrySnapshot {
    pub fn new(
        superblock_base: Option<Address>,
        blocks: Vec<Option<CacheCompressedTagLine>>,
        compression_factor: usize,
    ) -> Self {
        let replacement_state = CacheCompressedTagReplacementState::from_blocks(&blocks);
        Self {
            superblock_base,
            blocks,
            compression_factor,
            replacement_state,
        }
    }

    pub const fn superblock_base(&self) -> Option<Address> {
        self.superblock_base
    }

    pub fn blocks(&self) -> &[Option<CacheCompressedTagLine>] {
        &self.blocks
    }

    pub const fn compression_factor(&self) -> usize {
        self.compression_factor
    }
}
