use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BopPrefetcherConfigOptions {
    pub line_size: u64,
    pub score_max: u32,
    pub round_max: u32,
    pub bad_score: u32,
    pub rr_entries: usize,
    pub tag_bits: u32,
    pub offset_list_size: usize,
    pub negative_offsets: bool,
    pub degree: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BopPrefetcherConfig {
    line_size: u64,
    score_max: u32,
    round_max: u32,
    bad_score: u32,
    rr_entries: usize,
    tag_bits: u32,
    offset_list_size: usize,
    negative_offsets: bool,
    degree: u32,
}

impl BopPrefetcherConfig {
    pub fn new(options: BopPrefetcherConfigOptions) -> Result<Self, BopPrefetcherError> {
        let BopPrefetcherConfigOptions {
            line_size,
            score_max,
            round_max,
            bad_score,
            rr_entries,
            tag_bits,
            offset_list_size,
            negative_offsets,
            degree,
        } = options;

        if line_size == 0 {
            return Err(BopPrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(BopPrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if score_max == 0 {
            return Err(BopPrefetcherError::ZeroScoreMax);
        }
        if round_max == 0 {
            return Err(BopPrefetcherError::ZeroRoundMax);
        }
        if rr_entries == 0 {
            return Err(BopPrefetcherError::ZeroRrEntries);
        }
        if !rr_entries.is_power_of_two() {
            return Err(BopPrefetcherError::RrEntriesNotPowerOfTwo { rr_entries });
        }
        if !(1..=63).contains(&tag_bits) {
            return Err(BopPrefetcherError::TagBitsOutOfRange { tag_bits });
        }
        if offset_list_size == 0 {
            return Err(BopPrefetcherError::ZeroOffsetListSize);
        }
        if negative_offsets && !offset_list_size.is_multiple_of(2) {
            return Err(BopPrefetcherError::OddNegativeOffsetList { offset_list_size });
        }
        if degree == 0 {
            return Err(BopPrefetcherError::ZeroDegree);
        }

        Ok(Self {
            line_size,
            score_max,
            round_max,
            bad_score,
            rr_entries,
            tag_bits,
            offset_list_size,
            negative_offsets,
            degree,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn score_max(&self) -> u32 {
        self.score_max
    }

    pub const fn round_max(&self) -> u32 {
        self.round_max
    }

    pub const fn bad_score(&self) -> u32 {
        self.bad_score
    }

    pub const fn rr_entries(&self) -> usize {
        self.rr_entries
    }

    pub const fn tag_bits(&self) -> u32 {
        self.tag_bits
    }

    pub const fn offset_list_size(&self) -> usize {
        self.offset_list_size
    }

    pub const fn negative_offsets(&self) -> bool {
        self.negative_offsets
    }

    pub const fn degree(&self) -> u32 {
        self.degree
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BopPrefetcherError {
    ZeroLineSize,
    ZeroScoreMax,
    ZeroRoundMax,
    ZeroRrEntries,
    ZeroOffsetListSize,
    ZeroDegree,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    RrEntriesNotPowerOfTwo {
        rr_entries: usize,
    },
    TagBitsOutOfRange {
        tag_bits: u32,
    },
    OddNegativeOffsetList {
        offset_list_size: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<BopPrefetcherConfig>,
        actual: Box<BopPrefetcherConfig>,
    },
    SnapshotRrShapeMismatch {
        left_entries: usize,
        right_entries: usize,
        expected: usize,
    },
    SnapshotOffsetShapeMismatch {
        offsets: usize,
        scores: usize,
        expected: usize,
    },
}

impl fmt::Display for BopPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "BOP line size is zero"),
            Self::ZeroScoreMax => write!(formatter, "BOP score max is zero"),
            Self::ZeroRoundMax => write!(formatter, "BOP round max is zero"),
            Self::ZeroRrEntries => write!(formatter, "BOP RR table has no entries"),
            Self::ZeroOffsetListSize => write!(formatter, "BOP offset list is empty"),
            Self::ZeroDegree => write!(formatter, "BOP degree is zero"),
            Self::LineSizeNotPowerOfTwo { line_size } => {
                write!(formatter, "BOP line size {line_size} is not a power of two")
            }
            Self::RrEntriesNotPowerOfTwo { rr_entries } => write!(
                formatter,
                "BOP RR entry count {rr_entries} is not a power of two"
            ),
            Self::TagBitsOutOfRange { tag_bits } => {
                write!(formatter, "BOP tag bit count {tag_bits} is outside 1..=63")
            }
            Self::OddNegativeOffsetList { offset_list_size } => write!(
                formatter,
                "BOP negative offset list size {offset_list_size} is not even"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "BOP snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotRrShapeMismatch {
                left_entries,
                right_entries,
                expected,
            } => write!(
                formatter,
                "BOP snapshot RR tables have {left_entries}/{right_entries} entries instead of {expected}"
            ),
            Self::SnapshotOffsetShapeMismatch {
                offsets,
                scores,
                expected,
            } => write!(
                formatter,
                "BOP snapshot has {offsets} offsets and {scores} scores instead of {expected}"
            ),
        }
    }
}

impl Error for BopPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BopPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl BopPrefetchAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BopPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    offset: i16,
    stride: i64,
    degree_index: u32,
}

impl BopPrefetchCandidate {
    fn new(
        address: Address,
        access: BopPrefetchAccess,
        offset: i16,
        stride: i64,
        degree_index: u32,
    ) -> Self {
        Self {
            address,
            source_address: access.address(),
            context: access.requestor(),
            pc: access.pc(),
            secure: access.secure(),
            offset,
            stride,
            degree_index,
        }
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn offset(&self) -> i16 {
        self.offset
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for BopPrefetchCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BopPrefetcherSnapshot {
    config: BopPrefetcherConfig,
    rr_left: Vec<u64>,
    rr_right: Vec<u64>,
    offsets: Vec<i16>,
    scores: Vec<u32>,
    offset_index: usize,
    issue_prefetch_requests: bool,
    best_offset: i16,
    phase_best_offset: i16,
    best_score: u32,
    round: u32,
    last_candidates: Vec<BopPrefetchCandidate>,
}

impl BopPrefetcherSnapshot {
    pub const fn config(&self) -> &BopPrefetcherConfig {
        &self.config
    }

    pub fn rr_left(&self) -> &[u64] {
        &self.rr_left
    }

    pub fn rr_right(&self) -> &[u64] {
        &self.rr_right
    }

    pub fn offsets(&self) -> &[i16] {
        &self.offsets
    }

    pub fn scores(&self) -> &[u32] {
        &self.scores
    }

    pub const fn offset_index(&self) -> usize {
        self.offset_index
    }

    pub const fn issue_prefetch_requests(&self) -> bool {
        self.issue_prefetch_requests
    }

    pub const fn best_offset(&self) -> i16 {
        self.best_offset
    }

    pub const fn phase_best_offset(&self) -> i16 {
        self.phase_best_offset
    }

    pub const fn best_score(&self) -> u32 {
        self.best_score
    }

    pub const fn round(&self) -> u32 {
        self.round
    }

    pub fn last_candidates(&self) -> &[BopPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BopPrefetcher {
    config: BopPrefetcherConfig,
    rr_left: Vec<u64>,
    rr_right: Vec<u64>,
    offsets: Vec<i16>,
    scores: Vec<u32>,
    offset_index: usize,
    issue_prefetch_requests: bool,
    best_offset: i16,
    phase_best_offset: i16,
    best_score: u32,
    round: u32,
    last_candidates: Vec<BopPrefetchCandidate>,
}

impl BopPrefetcher {
    pub fn new(config: BopPrefetcherConfig) -> Self {
        let offsets = generate_offsets(config.offset_list_size(), config.negative_offsets());
        Self {
            rr_left: vec![0; config.rr_entries()],
            rr_right: vec![0; config.rr_entries()],
            offsets,
            scores: vec![0; config.offset_list_size()],
            config,
            offset_index: 0,
            issue_prefetch_requests: false,
            best_offset: 1,
            phase_best_offset: 0,
            best_score: 0,
            round: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &BopPrefetcherConfig {
        &self.config
    }

    pub fn offsets(&self) -> &[i16] {
        &self.offsets
    }

    pub fn scores(&self) -> &[u32] {
        &self.scores
    }

    pub const fn issue_prefetch_requests(&self) -> bool {
        self.issue_prefetch_requests
    }

    pub const fn best_offset(&self) -> i16 {
        self.best_offset
    }

    pub const fn phase_best_offset(&self) -> i16 {
        self.phase_best_offset
    }

    pub const fn best_score(&self) -> u32 {
        self.best_score
    }

    pub const fn round(&self) -> u32 {
        self.round
    }

    pub fn last_candidates(&self) -> &[BopPrefetchCandidate] {
        &self.last_candidates
    }

    pub fn observe(
        &mut self,
        access: BopPrefetchAccess,
    ) -> Result<&[BopPrefetchCandidate], BopPrefetcherError> {
        self.last_candidates.clear();
        let tag = self.tag(access.address());
        self.insert_rr_left(access.address(), tag);
        self.learn_best_offset(access.address());

        if self.issue_prefetch_requests {
            self.push_candidates(access);
        }

        Ok(&self.last_candidates)
    }

    pub fn observe_prefetch_fill(&mut self, address: Address) {
        if !self.issue_prefetch_requests {
            return;
        }
        let tag = self.tag(address).wrapping_sub(self.best_offset as u64);
        self.insert_rr_right(address, tag);
    }

    pub fn snapshot(&self) -> BopPrefetcherSnapshot {
        BopPrefetcherSnapshot {
            config: self.config.clone(),
            rr_left: self.rr_left.clone(),
            rr_right: self.rr_right.clone(),
            offsets: self.offsets.clone(),
            scores: self.scores.clone(),
            offset_index: self.offset_index,
            issue_prefetch_requests: self.issue_prefetch_requests,
            best_offset: self.best_offset,
            phase_best_offset: self.phase_best_offset,
            best_score: self.best_score,
            round: self.round,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &BopPrefetcherSnapshot) -> Result<(), BopPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(BopPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.rr_left().len() != self.config.rr_entries()
            || snapshot.rr_right().len() != self.config.rr_entries()
        {
            return Err(BopPrefetcherError::SnapshotRrShapeMismatch {
                left_entries: snapshot.rr_left().len(),
                right_entries: snapshot.rr_right().len(),
                expected: self.config.rr_entries(),
            });
        }
        if snapshot.offsets().len() != self.config.offset_list_size()
            || snapshot.scores().len() != self.config.offset_list_size()
        {
            return Err(BopPrefetcherError::SnapshotOffsetShapeMismatch {
                offsets: snapshot.offsets().len(),
                scores: snapshot.scores().len(),
                expected: self.config.offset_list_size(),
            });
        }

        self.rr_left = snapshot.rr_left().to_vec();
        self.rr_right = snapshot.rr_right().to_vec();
        self.offsets = snapshot.offsets().to_vec();
        self.scores = snapshot.scores().to_vec();
        self.offset_index = snapshot.offset_index() % self.offsets.len();
        self.issue_prefetch_requests = snapshot.issue_prefetch_requests();
        self.best_offset = snapshot.best_offset();
        self.phase_best_offset = snapshot.phase_best_offset();
        self.best_score = snapshot.best_score();
        self.round = snapshot.round();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    fn learn_best_offset(&mut self, address: Address) {
        let offset = self.offsets[self.offset_index];
        let lookup_address =
            offset_address(address, -(offset as i128) * self.config.line_size() as i128);
        if let Some(lookup_address) = lookup_address {
            let lookup_tag = self.tag(lookup_address);
            if self.test_rr(lookup_tag) {
                self.scores[self.offset_index] = self.scores[self.offset_index].saturating_add(1);
                if self.scores[self.offset_index] > self.best_score {
                    self.best_score = self.scores[self.offset_index];
                    self.phase_best_offset = offset;
                }
            }
        }

        self.offset_index += 1;
        if self.offset_index == self.offsets.len() {
            self.offset_index = 0;
            self.round = self.round.saturating_add(1);
        }

        if self.best_score >= self.config.score_max() || self.round >= self.config.round_max() {
            self.round = 0;
            if self.best_score > self.config.bad_score() {
                self.best_offset = self.phase_best_offset;
                self.issue_prefetch_requests = true;
            } else {
                self.issue_prefetch_requests = false;
            }
            self.reset_scores();
            self.best_score = 0;
            self.phase_best_offset = 0;
        }
    }

    fn push_candidates(&mut self, access: BopPrefetchAccess) {
        let stride = (self.best_offset as i128) * (self.config.line_size() as i128);
        for degree_index in 1..=self.config.degree() {
            let Some(address) = offset_address(access.address(), stride * degree_index as i128)
            else {
                continue;
            };
            self.last_candidates.push(BopPrefetchCandidate::new(
                address,
                access,
                self.best_offset,
                stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
                degree_index,
            ));
        }
    }

    fn reset_scores(&mut self) {
        self.scores.fill(0);
    }

    fn test_rr(&self, tag: u64) -> bool {
        self.rr_left.contains(&tag) || self.rr_right.contains(&tag)
    }

    fn insert_rr_left(&mut self, address: Address, tag: u64) {
        let index = self.index(address, 0);
        self.rr_left[index] = tag;
    }

    fn insert_rr_right(&mut self, address: Address, tag: u64) {
        let index = self.index(address, 1);
        self.rr_right[index] = tag;
    }

    fn index(&self, address: Address, way: u32) -> usize {
        let log_rr_entries = self.config.rr_entries().ilog2();
        let line_address = address.get() >> self.config.line_size().ilog2();
        let hash = line_address ^ (line_address >> (log_rr_entries << way));
        (hash & ((1_u64 << log_rr_entries) - 1)) as usize
    }

    fn tag(&self, address: Address) -> u64 {
        let mask = (1_u64 << self.config.tag_bits()) - 1;
        (address.get() >> self.config.line_size().ilog2()) & mask
    }
}

fn generate_offsets(offset_list_size: usize, negative_offsets: bool) -> Vec<i16> {
    let mut offsets = Vec::with_capacity(offset_list_size);
    let mut candidate = 1_i64;
    while offsets.len() < offset_list_size {
        if is_smooth_offset(candidate) {
            offsets.push(candidate.min(i16::MAX as i64) as i16);
            if negative_offsets && offsets.len() < offset_list_size {
                offsets.push((-candidate).max(i16::MIN as i64) as i16);
            }
        }
        candidate += 1;
    }
    offsets
}

fn is_smooth_offset(mut offset: i64) -> bool {
    for factor in [2, 3, 5] {
        while offset % factor == 0 {
            offset /= factor;
        }
    }
    offset == 1
}

fn offset_address(address: Address, offset: i128) -> Option<Address> {
    let next = address.get() as i128 + offset;
    if !(0..=u64::MAX as i128).contains(&next) {
        return None;
    }
    Some(Address::new(next as u64))
}
