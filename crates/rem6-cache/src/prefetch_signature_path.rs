use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

const CONFIDENCE_PPM: u32 = 1_000_000;
const LOOKAHEAD_CONFIDENCE_CAP_PPM: u32 = 950_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathRatio {
    numerator: u32,
    denominator: u32,
}

impl SignaturePathRatio {
    pub const fn new(
        numerator: u32,
        denominator: u32,
    ) -> Result<Self, SignaturePathPrefetcherError> {
        if denominator == 0 {
            return Err(SignaturePathPrefetcherError::ZeroRatioDenominator);
        }
        if numerator > denominator {
            return Err(SignaturePathPrefetcherError::RatioOutOfRange {
                numerator,
                denominator,
            });
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    pub const fn numerator(&self) -> u32 {
        self.numerator
    }

    pub const fn denominator(&self) -> u32 {
        self.denominator
    }

    fn counter_at_least(&self, counter: u32, max_counter: u32) -> bool {
        (counter as u128) * (self.denominator as u128)
            >= (self.numerator as u128) * (max_counter as u128)
    }

    fn confidence_exceeds_ppm(&self, confidence_ppm: u32) -> bool {
        (confidence_ppm as u128) * (self.denominator as u128)
            > (self.numerator as u128) * (CONFIDENCE_PPM as u128)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathPrefetcherConfig {
    line_size: u64,
    page_bytes: u64,
    signature_shift: u32,
    signature_bits: u32,
    signature_table_entries: usize,
    pattern_table_entries: usize,
    strides_per_pattern_entry: usize,
    counter_bits: u32,
    max_counter: u32,
    prefetch_confidence_threshold: SignaturePathRatio,
    lookahead_confidence_threshold: SignaturePathRatio,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathPrefetcherConfigOptions {
    pub line_size: u64,
    pub page_bytes: u64,
    pub signature_shift: u32,
    pub signature_bits: u32,
    pub signature_table_entries: usize,
    pub pattern_table_entries: usize,
    pub strides_per_pattern_entry: usize,
    pub counter_bits: u32,
    pub prefetch_confidence_threshold: SignaturePathRatio,
    pub lookahead_confidence_threshold: SignaturePathRatio,
}

impl SignaturePathPrefetcherConfig {
    pub fn new(
        options: SignaturePathPrefetcherConfigOptions,
    ) -> Result<Self, SignaturePathPrefetcherError> {
        let SignaturePathPrefetcherConfigOptions {
            line_size,
            page_bytes,
            signature_shift,
            signature_bits,
            signature_table_entries,
            pattern_table_entries,
            strides_per_pattern_entry,
            counter_bits,
            prefetch_confidence_threshold,
            lookahead_confidence_threshold,
        } = options;

        if line_size == 0 {
            return Err(SignaturePathPrefetcherError::ZeroLineSize);
        }
        if !line_size.is_power_of_two() {
            return Err(SignaturePathPrefetcherError::LineSizeNotPowerOfTwo { line_size });
        }
        if page_bytes == 0 {
            return Err(SignaturePathPrefetcherError::ZeroPageBytes);
        }
        if !page_bytes.is_multiple_of(line_size) {
            return Err(SignaturePathPrefetcherError::PageLineMismatch {
                page_bytes,
                line_size,
            });
        }
        let page_lines = page_bytes / line_size;
        if page_lines == 0 || page_lines > i16::MAX as u64 {
            return Err(SignaturePathPrefetcherError::PageLineCountOutOfRange {
                page_bytes,
                line_size,
            });
        }
        if signature_shift > 15 {
            return Err(SignaturePathPrefetcherError::SignatureShiftOutOfRange { signature_shift });
        }
        if !(1..=16).contains(&signature_bits) {
            return Err(SignaturePathPrefetcherError::SignatureBitsOutOfRange { signature_bits });
        }
        if signature_table_entries == 0 {
            return Err(SignaturePathPrefetcherError::ZeroSignatureTableEntries);
        }
        if pattern_table_entries == 0 {
            return Err(SignaturePathPrefetcherError::ZeroPatternTableEntries);
        }
        if strides_per_pattern_entry == 0 {
            return Err(SignaturePathPrefetcherError::ZeroStridesPerPatternEntry);
        }
        if !(1..=31).contains(&counter_bits) {
            return Err(SignaturePathPrefetcherError::CounterBitsOutOfRange { counter_bits });
        }
        let max_counter = (1_u32 << counter_bits) - 1;

        Ok(Self {
            line_size,
            page_bytes,
            signature_shift,
            signature_bits,
            signature_table_entries,
            pattern_table_entries,
            strides_per_pattern_entry,
            counter_bits,
            max_counter,
            prefetch_confidence_threshold,
            lookahead_confidence_threshold,
        })
    }

    pub const fn line_size(&self) -> u64 {
        self.line_size
    }

    pub const fn page_bytes(&self) -> u64 {
        self.page_bytes
    }

    pub const fn signature_shift(&self) -> u32 {
        self.signature_shift
    }

    pub const fn signature_bits(&self) -> u32 {
        self.signature_bits
    }

    pub const fn signature_table_entries(&self) -> usize {
        self.signature_table_entries
    }

    pub const fn pattern_table_entries(&self) -> usize {
        self.pattern_table_entries
    }

    pub const fn strides_per_pattern_entry(&self) -> usize {
        self.strides_per_pattern_entry
    }

    pub const fn counter_bits(&self) -> u32 {
        self.counter_bits
    }

    pub const fn max_counter(&self) -> u32 {
        self.max_counter
    }

    pub const fn prefetch_confidence_threshold(&self) -> SignaturePathRatio {
        self.prefetch_confidence_threshold
    }

    pub const fn lookahead_confidence_threshold(&self) -> SignaturePathRatio {
        self.lookahead_confidence_threshold
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignaturePathPrefetcherError {
    ZeroLineSize,
    ZeroPageBytes,
    ZeroSignatureTableEntries,
    ZeroPatternTableEntries,
    ZeroStridesPerPatternEntry,
    ZeroRatioDenominator,
    LineSizeNotPowerOfTwo {
        line_size: u64,
    },
    PageLineMismatch {
        page_bytes: u64,
        line_size: u64,
    },
    PageLineCountOutOfRange {
        page_bytes: u64,
        line_size: u64,
    },
    SignatureShiftOutOfRange {
        signature_shift: u32,
    },
    SignatureBitsOutOfRange {
        signature_bits: u32,
    },
    CounterBitsOutOfRange {
        counter_bits: u32,
    },
    RatioOutOfRange {
        numerator: u32,
        denominator: u32,
    },
    SnapshotConfigMismatch {
        expected: Box<SignaturePathPrefetcherConfig>,
        actual: Box<SignaturePathPrefetcherConfig>,
    },
    SnapshotSignatureEntryCountOutOfRange {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPatternEntryCountOutOfRange {
        entries: usize,
        table_entries: usize,
    },
    SnapshotPatternEntryShapeMismatch {
        signature: u16,
        entries: usize,
        expected: usize,
    },
    SnapshotCounterOutOfRange {
        signature: u16,
        stride: i16,
        counter: u32,
        max_counter: u32,
    },
}

impl fmt::Display for SignaturePathPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLineSize => write!(formatter, "signature path line size is zero"),
            Self::ZeroPageBytes => write!(formatter, "signature path page size is zero"),
            Self::ZeroSignatureTableEntries => {
                write!(formatter, "signature path signature table has no entries")
            }
            Self::ZeroPatternTableEntries => {
                write!(formatter, "signature path pattern table has no entries")
            }
            Self::ZeroStridesPerPatternEntry => {
                write!(formatter, "signature path pattern entries have no stride slots")
            }
            Self::ZeroRatioDenominator => {
                write!(formatter, "signature path confidence ratio denominator is zero")
            }
            Self::LineSizeNotPowerOfTwo { line_size } => write!(
                formatter,
                "signature path line size {line_size} is not a power of two"
            ),
            Self::PageLineMismatch {
                page_bytes,
                line_size,
            } => write!(
                formatter,
                "signature path page size {page_bytes} is not a multiple of line size {line_size}"
            ),
            Self::PageLineCountOutOfRange {
                page_bytes,
                line_size,
            } => write!(
                formatter,
                "signature path page size {page_bytes} with line size {line_size} does not fit stride blocks"
            ),
            Self::SignatureShiftOutOfRange { signature_shift } => write!(
                formatter,
                "signature path signature shift {signature_shift} is outside 0..=15"
            ),
            Self::SignatureBitsOutOfRange { signature_bits } => write!(
                formatter,
                "signature path signature bit count {signature_bits} is outside 1..=16"
            ),
            Self::CounterBitsOutOfRange { counter_bits } => write!(
                formatter,
                "signature path counter bit count {counter_bits} is outside 1..=31"
            ),
            Self::RatioOutOfRange {
                numerator,
                denominator,
            } => write!(
                formatter,
                "signature path confidence ratio {numerator}/{denominator} is above one"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "signature path snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSignatureEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "signature path snapshot has {entries} signature entries for {table_entries} slots"
            ),
            Self::SnapshotPatternEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "signature path snapshot has {entries} pattern entries for {table_entries} slots"
            ),
            Self::SnapshotPatternEntryShapeMismatch {
                signature,
                entries,
                expected,
            } => write!(
                formatter,
                "signature path snapshot pattern {signature:#x} has {entries} stride entries instead of {expected}"
            ),
            Self::SnapshotCounterOutOfRange {
                signature,
                stride,
                counter,
                max_counter,
            } => write!(
                formatter,
                "signature path snapshot pattern {signature:#x} stride {stride} has counter {counter} above {max_counter}"
            ),
        }
    }
}

impl Error for SignaturePathPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl SignaturePathPrefetchAccess {
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
pub struct SignaturePathPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    delta_blocks: i16,
    stride: i64,
    signature: u16,
    path_confidence_ppm: u32,
    degree_index: u32,
    auxiliary: bool,
}

impl SignaturePathPrefetchCandidate {
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

    pub const fn delta_blocks(&self) -> i16 {
        self.delta_blocks
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn signature(&self) -> u16 {
        self.signature
    }

    pub const fn path_confidence_ppm(&self) -> u32 {
        self.path_confidence_ppm
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }

    pub const fn auxiliary(&self) -> bool {
        self.auxiliary
    }
}

impl PrefetchCandidate for SignaturePathPrefetchCandidate {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathSignatureEntrySnapshot {
    ppn: u64,
    secure: bool,
    signature: u16,
    last_block: i16,
    last_used: u64,
}

impl SignaturePathSignatureEntrySnapshot {
    pub const fn ppn(&self) -> u64 {
        self.ppn
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn signature(&self) -> u16 {
        self.signature
    }

    pub const fn last_block(&self) -> i16 {
        self.last_block
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathPatternStrideSnapshot {
    stride: i16,
    counter: u32,
}

impl SignaturePathPatternStrideSnapshot {
    pub const fn stride(&self) -> i16 {
        self.stride
    }

    pub const fn counter(&self) -> u32 {
        self.counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathPatternEntrySnapshot {
    signature: u16,
    stride_entries: Vec<SignaturePathPatternStrideSnapshot>,
    last_used: u64,
}

impl SignaturePathPatternEntrySnapshot {
    pub const fn signature(&self) -> u16 {
        self.signature
    }

    pub fn stride_entries(&self) -> &[SignaturePathPatternStrideSnapshot] {
        &self.stride_entries
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathPrefetcherSnapshot {
    config: SignaturePathPrefetcherConfig,
    signature_entries: Vec<SignaturePathSignatureEntrySnapshot>,
    pattern_entries: Vec<SignaturePathPatternEntrySnapshot>,
    access_clock: u64,
    last_candidates: Vec<SignaturePathPrefetchCandidate>,
}

impl SignaturePathPrefetcherSnapshot {
    pub const fn config(&self) -> &SignaturePathPrefetcherConfig {
        &self.config
    }

    pub fn signature_entries(&self) -> &[SignaturePathSignatureEntrySnapshot] {
        &self.signature_entries
    }

    pub fn pattern_entries(&self) -> &[SignaturePathPatternEntrySnapshot] {
        &self.pattern_entries
    }

    pub const fn access_clock(&self) -> u64 {
        self.access_clock
    }

    pub fn last_candidates(&self) -> &[SignaturePathPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SignatureEntry {
    ppn: u64,
    secure: bool,
    signature: u16,
    last_block: i16,
    last_used: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatternStrideEntry {
    stride: i16,
    counter: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatternEntry {
    signature: u16,
    stride_entries: Vec<PatternStrideEntry>,
    last_used: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignatureLookup {
    miss: bool,
    stride: i16,
    signature: u16,
    initial_confidence_ppm: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AddPrefetchRequest {
    access: SignaturePathPrefetchAccess,
    ppn: u64,
    last_block: i16,
    delta: i16,
    signature: u16,
    path_confidence_ppm: u32,
    auxiliary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathPrefetcher {
    config: SignaturePathPrefetcherConfig,
    signature_entries: Vec<SignatureEntry>,
    pattern_entries: Vec<PatternEntry>,
    access_clock: u64,
    last_candidates: Vec<SignaturePathPrefetchCandidate>,
}

impl SignaturePathPrefetcher {
    pub const fn new(config: SignaturePathPrefetcherConfig) -> Self {
        Self {
            config,
            signature_entries: Vec::new(),
            pattern_entries: Vec::new(),
            access_clock: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &SignaturePathPrefetcherConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: SignaturePathPrefetchAccess,
    ) -> Result<&[SignaturePathPrefetchCandidate], SignaturePathPrefetcherError> {
        self.last_candidates.clear();

        let request_addr = access.address().get();
        let ppn = request_addr / self.config.page_bytes();
        let current_block =
            ((request_addr % self.config.page_bytes()) / self.config.line_size()) as i16;

        let lookup = self.lookup_signature(ppn, access.secure(), current_block);
        if lookup.miss || lookup.stride == 0 {
            return Ok(&self.last_candidates);
        }

        self.update_pattern_table(lookup.signature, lookup.stride);
        let current_signature = self.update_signature(lookup.signature, lookup.stride);
        self.update_signature_for_page(ppn, access.secure(), current_signature);

        self.collect_lookahead(
            access,
            ppn,
            current_block,
            current_signature,
            lookup.initial_confidence_ppm,
        );

        if self.last_candidates.is_empty() {
            self.add_prefetch(AddPrefetchRequest {
                access,
                ppn,
                last_block: current_block,
                delta: 1,
                signature: 0,
                path_confidence_ppm: 0,
                auxiliary: true,
            });
        }

        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> SignaturePathPrefetcherSnapshot {
        SignaturePathPrefetcherSnapshot {
            config: self.config.clone(),
            signature_entries: self
                .signature_entries
                .iter()
                .map(|entry| SignaturePathSignatureEntrySnapshot {
                    ppn: entry.ppn,
                    secure: entry.secure,
                    signature: entry.signature,
                    last_block: entry.last_block,
                    last_used: entry.last_used,
                })
                .collect(),
            pattern_entries: self
                .pattern_entries
                .iter()
                .map(|entry| SignaturePathPatternEntrySnapshot {
                    signature: entry.signature,
                    stride_entries: entry
                        .stride_entries
                        .iter()
                        .map(|stride_entry| SignaturePathPatternStrideSnapshot {
                            stride: stride_entry.stride,
                            counter: stride_entry.counter,
                        })
                        .collect(),
                    last_used: entry.last_used,
                })
                .collect(),
            access_clock: self.access_clock,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &SignaturePathPrefetcherSnapshot,
    ) -> Result<(), SignaturePathPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(SignaturePathPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.signature_entries().len() > self.config.signature_table_entries() {
            return Err(
                SignaturePathPrefetcherError::SnapshotSignatureEntryCountOutOfRange {
                    entries: snapshot.signature_entries().len(),
                    table_entries: self.config.signature_table_entries(),
                },
            );
        }
        if snapshot.pattern_entries().len() > self.config.pattern_table_entries() {
            return Err(
                SignaturePathPrefetcherError::SnapshotPatternEntryCountOutOfRange {
                    entries: snapshot.pattern_entries().len(),
                    table_entries: self.config.pattern_table_entries(),
                },
            );
        }
        for entry in snapshot.pattern_entries() {
            if entry.stride_entries().len() != self.config.strides_per_pattern_entry() {
                return Err(
                    SignaturePathPrefetcherError::SnapshotPatternEntryShapeMismatch {
                        signature: entry.signature(),
                        entries: entry.stride_entries().len(),
                        expected: self.config.strides_per_pattern_entry(),
                    },
                );
            }
            for stride_entry in entry.stride_entries() {
                if stride_entry.counter() > self.config.max_counter() {
                    return Err(SignaturePathPrefetcherError::SnapshotCounterOutOfRange {
                        signature: entry.signature(),
                        stride: stride_entry.stride(),
                        counter: stride_entry.counter(),
                        max_counter: self.config.max_counter(),
                    });
                }
            }
        }

        self.signature_entries = snapshot
            .signature_entries()
            .iter()
            .map(|entry| SignatureEntry {
                ppn: entry.ppn(),
                secure: entry.secure(),
                signature: entry.signature(),
                last_block: entry.last_block(),
                last_used: entry.last_used(),
            })
            .collect();
        self.pattern_entries = snapshot
            .pattern_entries()
            .iter()
            .map(|entry| PatternEntry {
                signature: entry.signature(),
                stride_entries: entry
                    .stride_entries()
                    .iter()
                    .map(|stride_entry| PatternStrideEntry {
                        stride: stride_entry.stride(),
                        counter: stride_entry.counter(),
                    })
                    .collect(),
                last_used: entry.last_used(),
            })
            .collect();
        self.access_clock = snapshot.access_clock();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn signature_for_page(&self, ppn: u64, secure: bool) -> Option<u16> {
        self.signature_entries
            .iter()
            .find(|entry| entry.ppn == ppn && entry.secure == secure)
            .map(|entry| entry.signature)
    }

    pub fn pattern_strides(&self, signature: u16) -> Vec<(i16, u32)> {
        self.pattern_entries
            .iter()
            .find(|entry| entry.signature == signature)
            .map(|entry| {
                entry
                    .stride_entries
                    .iter()
                    .filter(|stride_entry| stride_entry.stride != 0)
                    .map(|stride_entry| (stride_entry.stride, stride_entry.counter))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn last_candidates(&self) -> &[SignaturePathPrefetchCandidate] {
        &self.last_candidates
    }

    fn collect_lookahead(
        &mut self,
        access: SignaturePathPrefetchAccess,
        ppn: u64,
        mut current_stride: i16,
        mut current_signature: u16,
        mut current_confidence_ppm: u32,
    ) {
        while self
            .config
            .lookahead_confidence_threshold()
            .confidence_exceeds_ppm(current_confidence_ppm)
        {
            let Some(pattern_index) = self.pattern_index(current_signature) else {
                break;
            };
            let stride_entries = self.pattern_entries[pattern_index].stride_entries.clone();
            let mut lookahead: Option<PatternStrideEntry> = None;
            let mut max_counter = 0;
            for entry in stride_entries {
                if max_counter < entry.counter {
                    max_counter = entry.counter;
                    lookahead = Some(entry.clone());
                }
                if entry.stride != 0
                    && self
                        .config
                        .prefetch_confidence_threshold()
                        .counter_at_least(entry.counter, self.config.max_counter())
                {
                    self.add_prefetch(AddPrefetchRequest {
                        access,
                        ppn,
                        last_block: current_stride,
                        delta: entry.stride,
                        signature: current_signature,
                        path_confidence_ppm: current_confidence_ppm,
                        auxiliary: false,
                    });
                }
            }

            let Some(lookahead) = lookahead else {
                break;
            };
            current_confidence_ppm = multiply_confidence(
                current_confidence_ppm,
                lookahead_confidence_ppm(lookahead.counter, self.config.max_counter()),
            );
            current_signature = self.update_signature(current_signature, lookahead.stride);
            current_stride = current_stride.saturating_add(lookahead.stride);
        }
    }

    fn add_prefetch(&mut self, request: AddPrefetchRequest) {
        let block = request.last_block as i64 + request.delta as i64;
        let page_lines = (self.config.page_bytes() / self.config.line_size()) as i64;
        let (pf_ppn, pf_block) = if block < 0 {
            let pages_back = ((-block) + page_lines - 1) / page_lines;
            if pages_back as u64 > request.ppn {
                return;
            }
            (
                request.ppn - pages_back as u64,
                block + page_lines * pages_back,
            )
        } else if block >= page_lines {
            let pages_forward = block / page_lines;
            let Some(pf_ppn) = request.ppn.checked_add(pages_forward as u64) else {
                return;
            };
            (pf_ppn, block - page_lines * pages_forward)
        } else {
            (request.ppn, block)
        };

        let Some(page_base) = pf_ppn.checked_mul(self.config.page_bytes()) else {
            return;
        };
        let Some(block_offset) = (pf_block as u64).checked_mul(self.config.line_size()) else {
            return;
        };
        let Some(address) = page_base.checked_add(block_offset) else {
            return;
        };
        let degree_index = self
            .last_candidates
            .len()
            .saturating_add(1)
            .min(u32::MAX as usize) as u32;
        let stride = (request.delta as i128) * (self.config.line_size() as i128);
        self.last_candidates.push(SignaturePathPrefetchCandidate {
            address: Address::new(address),
            source_address: request.access.address(),
            context: request.access.requestor(),
            pc: request.access.pc(),
            secure: request.access.secure(),
            delta_blocks: request.delta,
            stride: stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            signature: request.signature,
            path_confidence_ppm: request.path_confidence_ppm,
            degree_index,
            auxiliary: request.auxiliary,
        });
    }

    fn lookup_signature(&mut self, ppn: u64, secure: bool, current_block: i16) -> SignatureLookup {
        let lru = self.next_lru();
        if let Some(index) = self
            .signature_entries
            .iter()
            .position(|entry| entry.ppn == ppn && entry.secure == secure)
        {
            let entry = &mut self.signature_entries[index];
            let stride = current_block - entry.last_block;
            let signature = entry.signature;
            entry.last_block = current_block;
            entry.last_used = lru;
            SignatureLookup {
                miss: false,
                stride,
                signature,
                initial_confidence_ppm: CONFIDENCE_PPM,
            }
        } else {
            if self.signature_entries.len() == self.config.signature_table_entries() {
                self.evict_lru_signature();
            }
            self.signature_entries.push(SignatureEntry {
                ppn,
                secure,
                signature: current_block as u16,
                last_block: current_block,
                last_used: lru,
            });
            SignatureLookup {
                miss: true,
                stride: current_block,
                signature: current_block as u16,
                initial_confidence_ppm: CONFIDENCE_PPM,
            }
        }
    }

    fn update_signature_for_page(&mut self, ppn: u64, secure: bool, signature: u16) {
        if let Some(entry) = self
            .signature_entries
            .iter_mut()
            .find(|entry| entry.ppn == ppn && entry.secure == secure)
        {
            entry.signature = signature;
        }
    }

    fn update_pattern_table(&mut self, signature: u16, stride: i16) {
        if stride == 0 {
            return;
        }
        let pattern_index = self.get_pattern_index(signature);
        let entry = &mut self.pattern_entries[pattern_index];
        if let Some(index) = entry
            .stride_entries
            .iter()
            .position(|stride_entry| stride_entry.stride == stride)
        {
            entry.stride_entries[index].counter = entry.stride_entries[index]
                .counter
                .saturating_add(1)
                .min(self.config.max_counter());
            return;
        }

        let mut victim = 0;
        let mut current_counter = u32::MAX;
        for index in 0..entry.stride_entries.len() {
            if entry.stride_entries[index].counter < current_counter {
                victim = index;
                current_counter = entry.stride_entries[index].counter;
            }
            entry.stride_entries[index].counter =
                entry.stride_entries[index].counter.saturating_sub(1);
        }
        entry.stride_entries[victim] = PatternStrideEntry { stride, counter: 1 };
    }

    fn get_pattern_index(&mut self, signature: u16) -> usize {
        let lru = self.next_lru();
        if let Some(index) = self.pattern_index(signature) {
            self.pattern_entries[index].last_used = lru;
            return index;
        }
        if self.pattern_entries.len() == self.config.pattern_table_entries() {
            self.evict_lru_pattern();
        }
        self.pattern_entries.push(PatternEntry {
            signature,
            stride_entries: vec![
                PatternStrideEntry {
                    stride: 0,
                    counter: 0,
                };
                self.config.strides_per_pattern_entry()
            ],
            last_used: lru,
        });
        self.pattern_entries.len() - 1
    }

    fn pattern_index(&self, signature: u16) -> Option<usize> {
        self.pattern_entries
            .iter()
            .position(|entry| entry.signature == signature)
    }

    fn update_signature(&self, signature: u16, stride: i16) -> u16 {
        let mask = if self.config.signature_bits() == 16 {
            u16::MAX as u32
        } else {
            (1_u32 << self.config.signature_bits()) - 1
        };
        let shifted = (signature as u32) << self.config.signature_shift();
        ((shifted ^ ((stride as u16) as u32)) & mask) as u16
    }

    fn evict_lru_signature(&mut self) {
        if let Some(index) = self
            .signature_entries
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(index, _)| index)
        {
            self.signature_entries.remove(index);
        }
    }

    fn evict_lru_pattern(&mut self) {
        if let Some(index) = self
            .pattern_entries
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(index, _)| index)
        {
            self.pattern_entries.remove(index);
        }
    }

    fn next_lru(&mut self) -> u64 {
        self.access_clock = self.access_clock.saturating_add(1);
        self.access_clock
    }
}

fn lookahead_confidence_ppm(counter: u32, max_counter: u32) -> u32 {
    let confidence = ((counter as u128) * (CONFIDENCE_PPM as u128) / (max_counter as u128)) as u32;
    confidence.min(LOOKAHEAD_CONFIDENCE_CAP_PPM)
}

fn multiply_confidence(current_ppm: u32, factor_ppm: u32) -> u32 {
    ((current_ppm as u128) * (factor_ppm as u128) / (CONFIDENCE_PPM as u128)) as u32
}
