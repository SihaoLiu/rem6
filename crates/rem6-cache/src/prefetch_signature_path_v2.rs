use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::allocation::max_vector_len;
use crate::prefetch::PrefetchCandidate;
use crate::prefetch_signature_path::{
    SignaturePathPrefetchAccess, SignaturePathPrefetcherConfig, SignaturePathPrefetcherError,
    SignaturePathRatio,
};

const CONFIDENCE_PPM: u32 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathV2PrefetcherConfig {
    base: SignaturePathPrefetcherConfig,
    global_history_register_entries: usize,
}

impl SignaturePathV2PrefetcherConfig {
    pub fn new(
        base: SignaturePathPrefetcherConfig,
        global_history_register_entries: usize,
    ) -> Result<Self, SignaturePathV2PrefetcherError> {
        if global_history_register_entries == 0 {
            return Err(SignaturePathV2PrefetcherError::ZeroGlobalHistoryRegisterEntries);
        }
        validate_signature_path_v2_vector_length(
            "global history register entries",
            global_history_register_entries,
            maximum_global_history_register_entries(),
        )?;
        Ok(Self {
            base,
            global_history_register_entries,
        })
    }

    pub const fn base(&self) -> &SignaturePathPrefetcherConfig {
        &self.base
    }

    pub const fn global_history_register_entries(&self) -> usize {
        self.global_history_register_entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignaturePathV2PrefetcherError {
    Base(SignaturePathPrefetcherError),
    ZeroGlobalHistoryRegisterEntries,
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    SnapshotConfigMismatch {
        expected: Box<SignaturePathV2PrefetcherConfig>,
        actual: Box<SignaturePathV2PrefetcherConfig>,
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
    SnapshotGlobalHistoryEntryCountOutOfRange {
        entries: usize,
        table_entries: usize,
    },
    SnapshotConfidenceOutOfRange {
        confidence_ppm: u32,
    },
    UsefulPrefetchesExceedIssued {
        useful: u64,
        issued: u64,
    },
}

impl fmt::Display for SignaturePathV2PrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Base(error) => write!(formatter, "{error}"),
            Self::ZeroGlobalHistoryRegisterEntries => {
                write!(formatter, "signature path v2 global history has no entries")
            }
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "signature path v2 {field} length {length} exceeds maximum {maximum}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "signature path v2 snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotSignatureEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "signature path v2 snapshot has {entries} signature entries for {table_entries} slots"
            ),
            Self::SnapshotPatternEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "signature path v2 snapshot has {entries} pattern entries for {table_entries} slots"
            ),
            Self::SnapshotPatternEntryShapeMismatch {
                signature,
                entries,
                expected,
            } => write!(
                formatter,
                "signature path v2 snapshot pattern {signature:#x} has {entries} stride entries instead of {expected}"
            ),
            Self::SnapshotCounterOutOfRange {
                signature,
                stride,
                counter,
                max_counter,
            } => write!(
                formatter,
                "signature path v2 snapshot pattern {signature:#x} stride {stride} has counter {counter} above {max_counter}"
            ),
            Self::SnapshotGlobalHistoryEntryCountOutOfRange {
                entries,
                table_entries,
            } => write!(
                formatter,
                "signature path v2 snapshot has {entries} global-history entries for {table_entries} slots"
            ),
            Self::SnapshotConfidenceOutOfRange { confidence_ppm } => write!(
                formatter,
                "signature path v2 snapshot confidence {confidence_ppm} ppm is above one"
            ),
            Self::UsefulPrefetchesExceedIssued { useful, issued } => write!(
                formatter,
                "signature path v2 useful prefetch count {useful} exceeds issued count {issued}"
            ),
        }
    }
}

impl Error for SignaturePathV2PrefetcherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Base(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SignaturePathPrefetcherError> for SignaturePathV2PrefetcherError {
    fn from(error: SignaturePathPrefetcherError) -> Self {
        Self::Base(error)
    }
}

fn maximum_global_history_register_entries() -> usize {
    max_vector_len::<GlobalHistoryEntry>()
        .min(max_vector_len::<SignaturePathV2GlobalHistoryEntrySnapshot>())
}

fn validate_signature_path_v2_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), SignaturePathV2PrefetcherError> {
    if length > maximum {
        return Err(SignaturePathV2PrefetcherError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathV2PrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    delta_blocks: i16,
    stride: i64,
    signature: u16,
    path_confidence_ppm: u32,
    prefetch_confidence_ppm: u32,
    degree_index: u32,
}

impl SignaturePathV2PrefetchCandidate {
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

    pub const fn prefetch_confidence_ppm(&self) -> u32 {
        self.prefetch_confidence_ppm
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for SignaturePathV2PrefetchCandidate {
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
pub struct SignaturePathV2SignatureEntrySnapshot {
    ppn: u64,
    secure: bool,
    signature: u16,
    last_block: i16,
    last_used: u64,
}

impl SignaturePathV2SignatureEntrySnapshot {
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
pub struct SignaturePathV2PatternStrideSnapshot {
    stride: i16,
    counter: u32,
}

impl SignaturePathV2PatternStrideSnapshot {
    pub const fn stride(&self) -> i16 {
        self.stride
    }

    pub const fn counter(&self) -> u32 {
        self.counter
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathV2PatternEntrySnapshot {
    signature: u16,
    counter: u32,
    stride_entries: Vec<SignaturePathV2PatternStrideSnapshot>,
    last_used: u64,
}

impl SignaturePathV2PatternEntrySnapshot {
    pub const fn signature(&self) -> u16 {
        self.signature
    }

    pub const fn counter(&self) -> u32 {
        self.counter
    }

    pub fn stride_entries(&self) -> &[SignaturePathV2PatternStrideSnapshot] {
        &self.stride_entries
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignaturePathV2GlobalHistoryEntrySnapshot {
    signature: u16,
    confidence_ppm: u32,
    last_block: i16,
    delta: i16,
    last_used: u64,
}

impl SignaturePathV2GlobalHistoryEntrySnapshot {
    pub const fn signature(&self) -> u16 {
        self.signature
    }

    pub const fn confidence_ppm(&self) -> u32 {
        self.confidence_ppm
    }

    pub const fn last_block(&self) -> i16 {
        self.last_block
    }

    pub const fn delta(&self) -> i16 {
        self.delta
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathV2PrefetcherSnapshot {
    config: SignaturePathV2PrefetcherConfig,
    signature_entries: Vec<SignaturePathV2SignatureEntrySnapshot>,
    pattern_entries: Vec<SignaturePathV2PatternEntrySnapshot>,
    global_history_entries: Vec<SignaturePathV2GlobalHistoryEntrySnapshot>,
    access_clock: u64,
    issued_prefetches: u64,
    useful_prefetches: u64,
    last_candidates: Vec<SignaturePathV2PrefetchCandidate>,
}

impl SignaturePathV2PrefetcherSnapshot {
    pub const fn config(&self) -> &SignaturePathV2PrefetcherConfig {
        &self.config
    }

    pub fn signature_entries(&self) -> &[SignaturePathV2SignatureEntrySnapshot] {
        &self.signature_entries
    }

    pub fn pattern_entries(&self) -> &[SignaturePathV2PatternEntrySnapshot] {
        &self.pattern_entries
    }

    pub fn global_history_entries(&self) -> &[SignaturePathV2GlobalHistoryEntrySnapshot] {
        &self.global_history_entries
    }

    pub const fn access_clock(&self) -> u64 {
        self.access_clock
    }

    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }

    pub fn last_candidates(&self) -> &[SignaturePathV2PrefetchCandidate] {
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
    counter: u32,
    stride_entries: Vec<PatternStrideEntry>,
    last_used: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GlobalHistoryEntry {
    signature: u16,
    confidence_ppm: u32,
    last_block: i16,
    delta: i16,
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
    prefetch_confidence_ppm: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignaturePathV2Prefetcher {
    config: SignaturePathV2PrefetcherConfig,
    signature_entries: Vec<SignatureEntry>,
    pattern_entries: Vec<PatternEntry>,
    global_history_entries: Vec<GlobalHistoryEntry>,
    access_clock: u64,
    issued_prefetches: u64,
    useful_prefetches: u64,
    last_candidates: Vec<SignaturePathV2PrefetchCandidate>,
}

impl SignaturePathV2Prefetcher {
    pub const fn new(config: SignaturePathV2PrefetcherConfig) -> Self {
        Self {
            config,
            signature_entries: Vec::new(),
            pattern_entries: Vec::new(),
            global_history_entries: Vec::new(),
            access_clock: 0,
            issued_prefetches: 0,
            useful_prefetches: 0,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &SignaturePathV2PrefetcherConfig {
        &self.config
    }

    pub fn observe(
        &mut self,
        access: SignaturePathPrefetchAccess,
    ) -> Result<&[SignaturePathV2PrefetchCandidate], SignaturePathV2PrefetcherError> {
        self.last_candidates.clear();

        let request_addr = access.address().get();
        let ppn = request_addr / self.config.base().page_bytes();
        let current_block = ((request_addr % self.config.base().page_bytes())
            / self.config.base().line_size()) as i16;

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

        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> SignaturePathV2PrefetcherSnapshot {
        SignaturePathV2PrefetcherSnapshot {
            config: self.config.clone(),
            signature_entries: self
                .signature_entries
                .iter()
                .map(|entry| SignaturePathV2SignatureEntrySnapshot {
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
                .map(|entry| SignaturePathV2PatternEntrySnapshot {
                    signature: entry.signature,
                    counter: entry.counter,
                    stride_entries: entry
                        .stride_entries
                        .iter()
                        .map(|stride_entry| SignaturePathV2PatternStrideSnapshot {
                            stride: stride_entry.stride,
                            counter: stride_entry.counter,
                        })
                        .collect(),
                    last_used: entry.last_used,
                })
                .collect(),
            global_history_entries: self
                .global_history_entries
                .iter()
                .map(|entry| SignaturePathV2GlobalHistoryEntrySnapshot {
                    signature: entry.signature,
                    confidence_ppm: entry.confidence_ppm,
                    last_block: entry.last_block,
                    delta: entry.delta,
                    last_used: entry.last_used,
                })
                .collect(),
            access_clock: self.access_clock,
            issued_prefetches: self.issued_prefetches,
            useful_prefetches: self.useful_prefetches,
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &SignaturePathV2PrefetcherSnapshot,
    ) -> Result<(), SignaturePathV2PrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(SignaturePathV2PrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }
        if snapshot.signature_entries().len() > self.config.base().signature_table_entries() {
            return Err(
                SignaturePathV2PrefetcherError::SnapshotSignatureEntryCountOutOfRange {
                    entries: snapshot.signature_entries().len(),
                    table_entries: self.config.base().signature_table_entries(),
                },
            );
        }
        if snapshot.pattern_entries().len() > self.config.base().pattern_table_entries() {
            return Err(
                SignaturePathV2PrefetcherError::SnapshotPatternEntryCountOutOfRange {
                    entries: snapshot.pattern_entries().len(),
                    table_entries: self.config.base().pattern_table_entries(),
                },
            );
        }
        if snapshot.global_history_entries().len() > self.config.global_history_register_entries() {
            return Err(
                SignaturePathV2PrefetcherError::SnapshotGlobalHistoryEntryCountOutOfRange {
                    entries: snapshot.global_history_entries().len(),
                    table_entries: self.config.global_history_register_entries(),
                },
            );
        }
        self.validate_pattern_entries(snapshot.pattern_entries())?;
        for entry in snapshot.global_history_entries() {
            if entry.confidence_ppm() > CONFIDENCE_PPM {
                return Err(
                    SignaturePathV2PrefetcherError::SnapshotConfidenceOutOfRange {
                        confidence_ppm: entry.confidence_ppm(),
                    },
                );
            }
        }
        if snapshot.useful_prefetches() > snapshot.issued_prefetches() {
            return Err(
                SignaturePathV2PrefetcherError::UsefulPrefetchesExceedIssued {
                    useful: snapshot.useful_prefetches(),
                    issued: snapshot.issued_prefetches(),
                },
            );
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
                counter: entry.counter(),
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
        self.global_history_entries = snapshot
            .global_history_entries()
            .iter()
            .map(|entry| GlobalHistoryEntry {
                signature: entry.signature(),
                confidence_ppm: entry.confidence_ppm(),
                last_block: entry.last_block(),
                delta: entry.delta(),
                last_used: entry.last_used(),
            })
            .collect();
        self.access_clock = snapshot.access_clock();
        self.issued_prefetches = snapshot.issued_prefetches();
        self.useful_prefetches = snapshot.useful_prefetches();
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    pub fn pattern_total_counter(&self, signature: u16) -> Option<u32> {
        self.pattern_entries
            .iter()
            .find(|entry| entry.signature == signature)
            .map(|entry| entry.counter)
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

    pub fn global_history_entries(&self) -> Vec<SignaturePathV2GlobalHistoryEntrySnapshot> {
        self.global_history_entries
            .iter()
            .map(|entry| SignaturePathV2GlobalHistoryEntrySnapshot {
                signature: entry.signature,
                confidence_ppm: entry.confidence_ppm,
                last_block: entry.last_block,
                delta: entry.delta,
                last_used: entry.last_used,
            })
            .collect()
    }

    pub fn set_prefetch_accuracy_counts(
        &mut self,
        issued: u64,
        useful: u64,
    ) -> Result<(), SignaturePathV2PrefetcherError> {
        if useful > issued {
            return Err(
                SignaturePathV2PrefetcherError::UsefulPrefetchesExceedIssued { useful, issued },
            );
        }
        self.issued_prefetches = issued;
        self.useful_prefetches = useful;
        Ok(())
    }

    pub const fn issued_prefetches(&self) -> u64 {
        self.issued_prefetches
    }

    pub const fn useful_prefetches(&self) -> u64 {
        self.useful_prefetches
    }

    pub fn prefetch_accuracy_ppm(&self) -> u32 {
        self.prefetch_accuracy_ppm_inner()
    }

    pub fn last_candidates(&self) -> &[SignaturePathV2PrefetchCandidate] {
        &self.last_candidates
    }

    fn validate_pattern_entries(
        &self,
        entries: &[SignaturePathV2PatternEntrySnapshot],
    ) -> Result<(), SignaturePathV2PrefetcherError> {
        for entry in entries {
            if entry.stride_entries().len() != self.config.base().strides_per_pattern_entry() {
                return Err(
                    SignaturePathV2PrefetcherError::SnapshotPatternEntryShapeMismatch {
                        signature: entry.signature(),
                        entries: entry.stride_entries().len(),
                        expected: self.config.base().strides_per_pattern_entry(),
                    },
                );
            }
            if entry.counter() > self.config.base().max_counter() {
                return Err(SignaturePathV2PrefetcherError::SnapshotCounterOutOfRange {
                    signature: entry.signature(),
                    stride: 0,
                    counter: entry.counter(),
                    max_counter: self.config.base().max_counter(),
                });
            }
            for stride_entry in entry.stride_entries() {
                if stride_entry.counter() > self.config.base().max_counter() {
                    return Err(SignaturePathV2PrefetcherError::SnapshotCounterOutOfRange {
                        signature: entry.signature(),
                        stride: stride_entry.stride(),
                        counter: stride_entry.counter(),
                        max_counter: self.config.base().max_counter(),
                    });
                }
            }
        }
        Ok(())
    }

    fn collect_lookahead(
        &mut self,
        access: SignaturePathPrefetchAccess,
        ppn: u64,
        mut current_block: i16,
        mut current_signature: u16,
        mut current_confidence_ppm: u32,
    ) {
        let mut visited_signatures = Vec::new();
        while ratio_exceeds(
            self.config.base().lookahead_confidence_threshold(),
            current_confidence_ppm,
        ) {
            if visited_signatures.contains(&current_signature) {
                break;
            }
            visited_signatures.push(current_signature);
            let Some(pattern_index) = self.pattern_index(current_signature) else {
                break;
            };
            let pattern = self.pattern_entries[pattern_index].clone();
            let mut lookahead: Option<PatternStrideEntry> = None;
            let mut max_counter = 0;
            for entry in pattern.stride_entries {
                if max_counter < entry.counter {
                    max_counter = entry.counter;
                    lookahead = Some(entry.clone());
                }
                if entry.stride == 0 {
                    continue;
                }
                let prefetch_confidence_ppm =
                    confidence_from_counter(entry.counter, pattern.counter);
                if ratio_at_least(
                    self.config.base().prefetch_confidence_threshold(),
                    prefetch_confidence_ppm,
                ) {
                    self.add_prefetch(AddPrefetchRequest {
                        access,
                        ppn,
                        last_block: current_block,
                        delta: entry.stride,
                        signature: current_signature,
                        path_confidence_ppm: current_confidence_ppm,
                        prefetch_confidence_ppm,
                    });
                }
            }

            let Some(lookahead) = lookahead else {
                break;
            };
            let factor_ppm = multiply_confidence(
                self.prefetch_accuracy_ppm_inner(),
                confidence_from_counter(lookahead.counter, pattern.counter),
            );
            current_confidence_ppm = multiply_confidence(current_confidence_ppm, factor_ppm);
            current_signature = self.update_signature(current_signature, lookahead.stride);
            current_block = current_block.saturating_add(lookahead.stride);
        }
    }

    fn add_prefetch(&mut self, request: AddPrefetchRequest) {
        let block = request.last_block as i64 + request.delta as i64;
        let page_lines = (self.config.base().page_bytes() / self.config.base().line_size()) as i64;
        let (pf_ppn, pf_block) = if block < 0 {
            self.insert_global_history(
                request.signature,
                request.path_confidence_ppm,
                request.last_block,
                request.delta,
            );
            let pages_back = ((-block) + page_lines - 1) / page_lines;
            if pages_back as u64 > request.ppn {
                return;
            }
            (
                request.ppn - pages_back as u64,
                block + page_lines * pages_back,
            )
        } else if block >= page_lines {
            self.insert_global_history(
                request.signature,
                request.path_confidence_ppm,
                request.last_block,
                request.delta,
            );
            let pages_forward = block / page_lines;
            let Some(pf_ppn) = request.ppn.checked_add(pages_forward as u64) else {
                return;
            };
            (pf_ppn, block - page_lines * pages_forward)
        } else {
            (request.ppn, block)
        };

        let Some(page_base) = pf_ppn.checked_mul(self.config.base().page_bytes()) else {
            return;
        };
        let Some(block_offset) = (pf_block as u64).checked_mul(self.config.base().line_size())
        else {
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
        let stride = (request.delta as i128) * (self.config.base().line_size() as i128);
        self.last_candidates.push(SignaturePathV2PrefetchCandidate {
            address: Address::new(address),
            source_address: request.access.address(),
            context: request.access.requestor(),
            pc: request.access.pc(),
            secure: request.access.secure(),
            delta_blocks: request.delta,
            stride: stride.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            signature: request.signature,
            path_confidence_ppm: request.path_confidence_ppm,
            prefetch_confidence_ppm: request.prefetch_confidence_ppm,
            degree_index,
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
            return SignatureLookup {
                miss: false,
                stride,
                signature,
                initial_confidence_ppm: CONFIDENCE_PPM,
            };
        }

        let (signature, initial_confidence_ppm, stride) =
            self.signature_from_global_history(current_block);
        if self.signature_entries.len() == self.config.base().signature_table_entries() {
            self.evict_lru_signature();
        }
        self.signature_entries.push(SignatureEntry {
            ppn,
            secure,
            signature,
            last_block: current_block,
            last_used: lru,
        });
        SignatureLookup {
            miss: true,
            stride,
            signature,
            initial_confidence_ppm,
        }
    }

    fn signature_from_global_history(&mut self, current_block: i16) -> (u16, u32, i16) {
        if let Some(index) = self
            .global_history_entries
            .iter()
            .position(|entry| entry.last_block.saturating_add(entry.delta) == current_block)
        {
            let lru = self.next_lru();
            let entry = &mut self.global_history_entries[index];
            entry.last_used = lru;
            (entry.signature, entry.confidence_ppm, entry.delta)
        } else {
            (current_block as u16, CONFIDENCE_PPM, current_block)
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
        let stride_index = if let Some(index) = entry
            .stride_entries
            .iter()
            .position(|stride_entry| stride_entry.stride == stride)
        {
            index
        } else {
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
            entry.stride_entries[victim] = PatternStrideEntry { stride, counter: 0 };
            victim
        };

        if entry.counter >= self.config.base().max_counter()
            || entry.stride_entries[stride_index].counter >= self.config.base().max_counter()
        {
            entry.counter >>= 1;
            for stride_entry in &mut entry.stride_entries {
                stride_entry.counter >>= 1;
            }
        }
        entry.counter = entry
            .counter
            .saturating_add(1)
            .min(self.config.base().max_counter());
        entry.stride_entries[stride_index].counter = entry.stride_entries[stride_index]
            .counter
            .saturating_add(1)
            .min(self.config.base().max_counter());
    }

    fn get_pattern_index(&mut self, signature: u16) -> usize {
        let lru = self.next_lru();
        if let Some(index) = self.pattern_index(signature) {
            self.pattern_entries[index].last_used = lru;
            return index;
        }
        if self.pattern_entries.len() == self.config.base().pattern_table_entries() {
            self.evict_lru_pattern();
        }
        self.pattern_entries.push(PatternEntry {
            signature,
            counter: 0,
            stride_entries: vec![
                PatternStrideEntry {
                    stride: 0,
                    counter: 0,
                };
                self.config.base().strides_per_pattern_entry()
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
        let mask = if self.config.base().signature_bits() == 16 {
            u16::MAX as u32
        } else {
            (1_u32 << self.config.base().signature_bits()) - 1
        };
        let shifted = (signature as u32) << self.config.base().signature_shift();
        ((shifted ^ ((stride as u16) as u32)) & mask) as u16
    }

    fn insert_global_history(
        &mut self,
        signature: u16,
        confidence_ppm: u32,
        last_block: i16,
        delta: i16,
    ) {
        if self.global_history_entries.len() == self.config.global_history_register_entries() {
            self.evict_lru_global_history();
        }
        let last_used = self.next_lru();
        self.global_history_entries.push(GlobalHistoryEntry {
            signature,
            confidence_ppm,
            last_block,
            delta,
            last_used,
        });
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

    fn evict_lru_global_history(&mut self) {
        if let Some(index) = self
            .global_history_entries
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(index, _)| index)
        {
            self.global_history_entries.remove(index);
        }
    }

    fn prefetch_accuracy_ppm_inner(&self) -> u32 {
        if self.issued_prefetches == 0 {
            CONFIDENCE_PPM
        } else {
            ((self.useful_prefetches as u128) * (CONFIDENCE_PPM as u128)
                / (self.issued_prefetches as u128)) as u32
        }
    }

    fn next_lru(&mut self) -> u64 {
        self.access_clock = self.access_clock.saturating_add(1);
        self.access_clock
    }
}

fn ratio_at_least(threshold: SignaturePathRatio, confidence_ppm: u32) -> bool {
    (confidence_ppm as u128) * (threshold.denominator() as u128)
        >= (threshold.numerator() as u128) * (CONFIDENCE_PPM as u128)
}

fn ratio_exceeds(threshold: SignaturePathRatio, confidence_ppm: u32) -> bool {
    (confidence_ppm as u128) * (threshold.denominator() as u128)
        > (threshold.numerator() as u128) * (CONFIDENCE_PPM as u128)
}

fn confidence_from_counter(counter: u32, total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        ((counter as u128) * (CONFIDENCE_PPM as u128) / (total as u128)) as u32
    }
}

fn multiply_confidence(current_ppm: u32, factor_ppm: u32) -> u32 {
    ((current_ppm as u128) * (factor_ppm as u128) / (CONFIDENCE_PPM as u128)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefetch_signature_path::SignaturePathPrefetcherConfigOptions;

    fn test_config() -> SignaturePathV2PrefetcherConfig {
        let base = SignaturePathPrefetcherConfig::new(SignaturePathPrefetcherConfigOptions {
            line_size: 64,
            page_bytes: 4096,
            signature_shift: 2,
            signature_bits: 8,
            signature_table_entries: 4,
            pattern_table_entries: 4,
            strides_per_pattern_entry: 2,
            counter_bits: 2,
            prefetch_confidence_threshold: SignaturePathRatio::new(3, 10).unwrap(),
            lookahead_confidence_threshold: SignaturePathRatio::new(3, 10).unwrap(),
        })
        .unwrap();
        SignaturePathV2PrefetcherConfig::new(base, 2).unwrap()
    }

    #[test]
    fn restore_rejects_useful_prefetches_above_issued_without_mutation() {
        let config = test_config();
        let mut snapshot_source = SignaturePathV2Prefetcher::new(config.clone());
        snapshot_source.set_prefetch_accuracy_counts(2, 1).unwrap();
        let mut snapshot = snapshot_source.snapshot();
        snapshot.issued_prefetches = 1;
        snapshot.useful_prefetches = 2;

        let mut restored = SignaturePathV2Prefetcher::new(config);
        restored.set_prefetch_accuracy_counts(4, 1).unwrap();
        assert_eq!(
            restored.restore(&snapshot),
            Err(
                SignaturePathV2PrefetcherError::UsefulPrefetchesExceedIssued {
                    useful: 2,
                    issued: 1,
                }
            )
        );
        assert_eq!(restored.issued_prefetches(), 4);
        assert_eq!(restored.useful_prefetches(), 1);
    }
}
