use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcptPrefetcherConfig {
    deltas_per_entry: usize,
    delta_bits: u32,
    delta_mask_bits: u32,
    table_entries: usize,
    use_requestor_id: bool,
}

impl DcptPrefetcherConfig {
    pub fn new(
        deltas_per_entry: usize,
        delta_bits: u32,
        delta_mask_bits: u32,
        table_entries: usize,
        use_requestor_id: bool,
    ) -> Result<Self, DcptPrefetcherError> {
        if deltas_per_entry == 0 {
            return Err(DcptPrefetcherError::ZeroDeltasPerEntry);
        }
        if deltas_per_entry < 4 {
            return Err(DcptPrefetcherError::DeltaHistoryTooSmall { deltas_per_entry });
        }
        if !(2..=63).contains(&delta_bits) {
            return Err(DcptPrefetcherError::DeltaBitsOutOfRange { delta_bits });
        }
        if delta_mask_bits >= 64 {
            return Err(DcptPrefetcherError::DeltaMaskBitsOutOfRange { delta_mask_bits });
        }
        if table_entries == 0 {
            return Err(DcptPrefetcherError::ZeroTableEntries);
        }

        Ok(Self {
            deltas_per_entry,
            delta_bits,
            delta_mask_bits,
            table_entries,
            use_requestor_id,
        })
    }

    pub const fn deltas_per_entry(&self) -> usize {
        self.deltas_per_entry
    }

    pub const fn delta_bits(&self) -> u32 {
        self.delta_bits
    }

    pub const fn delta_mask_bits(&self) -> u32 {
        self.delta_mask_bits
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn use_requestor_id(&self) -> bool {
        self.use_requestor_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DcptPrefetcherError {
    ZeroDeltasPerEntry,
    ZeroTableEntries,
    DeltaHistoryTooSmall {
        deltas_per_entry: usize,
    },
    DeltaBitsOutOfRange {
        delta_bits: u32,
    },
    DeltaMaskBitsOutOfRange {
        delta_mask_bits: u32,
    },
    SnapshotConfigMismatch {
        expected: Box<DcptPrefetcherConfig>,
        actual: Box<DcptPrefetcherConfig>,
    },
    SnapshotContextOutOfRange {
        context: AgentId,
        entries: usize,
        table_entries: usize,
    },
    SnapshotEntryShapeMismatch {
        pc: u64,
        deltas: usize,
        expected: usize,
    },
}

impl fmt::Display for DcptPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDeltasPerEntry => write!(formatter, "DCPT delta history is empty"),
            Self::ZeroTableEntries => write!(formatter, "DCPT table has no entries"),
            Self::DeltaHistoryTooSmall { deltas_per_entry } => write!(
                formatter,
                "DCPT delta history has {deltas_per_entry} entries but needs at least four"
            ),
            Self::DeltaBitsOutOfRange { delta_bits } => {
                write!(
                    formatter,
                    "DCPT delta bit width {delta_bits} is outside 2..=63"
                )
            }
            Self::DeltaMaskBitsOutOfRange { delta_mask_bits } => write!(
                formatter,
                "DCPT delta mask bit count {delta_mask_bits} is outside 0..64"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "DCPT snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::SnapshotContextOutOfRange {
                context,
                entries,
                table_entries,
            } => write!(
                formatter,
                "DCPT snapshot context {} has {entries} entries for {table_entries} slots",
                context.get()
            ),
            Self::SnapshotEntryShapeMismatch {
                pc,
                deltas,
                expected,
            } => write!(
                formatter,
                "DCPT snapshot entry for pc {pc:#x} has {deltas} deltas instead of {expected}"
            ),
        }
    }
}

impl Error for DcptPrefetcherError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DcptPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl DcptPrefetchAccess {
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
pub struct DcptPrefetchCandidate {
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    delta: i64,
    degree_index: u32,
}

impl DcptPrefetchCandidate {
    fn new(
        address: Address,
        source_address: Address,
        context: AgentId,
        pc: u64,
        secure: bool,
        delta: i64,
        degree_index: u32,
    ) -> Self {
        Self {
            address,
            source_address,
            context,
            pc,
            secure,
            delta,
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

    pub const fn delta(&self) -> i64 {
        self.delta
    }

    pub const fn stride(&self) -> i64 {
        self.delta
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for DcptPrefetchCandidate {
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
pub struct DcptPrefetchEntrySnapshot {
    pc: u64,
    secure: bool,
    last_address: Address,
    deltas: Vec<i64>,
}

impl DcptPrefetchEntrySnapshot {
    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn last_address(&self) -> Address {
        self.last_address
    }

    pub fn deltas(&self) -> &[i64] {
        &self.deltas
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcptPrefetchContextSnapshot {
    context: AgentId,
    entries: Vec<DcptPrefetchEntrySnapshot>,
    next_victim: usize,
}

impl DcptPrefetchContextSnapshot {
    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub fn entries(&self) -> &[DcptPrefetchEntrySnapshot] {
        &self.entries
    }

    pub const fn next_victim(&self) -> usize {
        self.next_victim
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcptPrefetcherSnapshot {
    config: DcptPrefetcherConfig,
    contexts: Vec<DcptPrefetchContextSnapshot>,
    last_candidates: Vec<DcptPrefetchCandidate>,
}

impl DcptPrefetcherSnapshot {
    pub const fn config(&self) -> &DcptPrefetcherConfig {
        &self.config
    }

    pub fn contexts(&self) -> &[DcptPrefetchContextSnapshot] {
        &self.contexts
    }

    pub fn last_candidates(&self) -> &[DcptPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DcptPrefetchEntry {
    pc: u64,
    secure: bool,
    last_address: Address,
    deltas: Vec<i64>,
}

impl DcptPrefetchEntry {
    fn new(config: &DcptPrefetcherConfig, pc: u64, secure: bool, last_address: Address) -> Self {
        Self {
            pc,
            secure,
            last_address,
            deltas: vec![0; config.deltas_per_entry()],
        }
    }

    fn snapshot(&self) -> DcptPrefetchEntrySnapshot {
        DcptPrefetchEntrySnapshot {
            pc: self.pc,
            secure: self.secure,
            last_address: self.last_address,
            deltas: self.deltas.clone(),
        }
    }

    fn from_snapshot(snapshot: &DcptPrefetchEntrySnapshot) -> Self {
        Self {
            pc: snapshot.pc(),
            secure: snapshot.secure(),
            last_address: snapshot.last_address(),
            deltas: snapshot.deltas().to_vec(),
        }
    }

    fn push_address(&mut self, config: &DcptPrefetcherConfig, address: Address) {
        let delta = address.get() as i128 - self.last_address.get() as i128;
        if delta == 0 {
            return;
        }

        let stored_delta = clamp_delta(delta, config.delta_bits());
        self.deltas.remove(0);
        self.deltas.push(stored_delta);
        self.last_address = address;
    }

    fn candidates(
        &self,
        access: DcptPrefetchAccess,
        context: AgentId,
        config: &DcptPrefetcherConfig,
    ) -> Vec<DcptPrefetchCandidate> {
        let mut candidates = Vec::new();
        if self.deltas.len() < 4 {
            return candidates;
        }

        let penultimate = self.deltas[self.deltas.len() - 2];
        let last = self.deltas[self.deltas.len() - 1];
        if penultimate == 0 || last == 0 {
            return candidates;
        }

        for index in 0..self.deltas.len() - 2 {
            let previous_penultimate = self.deltas[index];
            let previous_last = self.deltas[index + 1];
            if masked_delta(previous_penultimate, config.delta_mask_bits())
                == masked_delta(penultimate, config.delta_mask_bits())
                && masked_delta(previous_last, config.delta_mask_bits())
                    == masked_delta(last, config.delta_mask_bits())
            {
                let mut address = self.last_address;
                for delta in self.deltas.iter().copied().skip(index + 2) {
                    let Some(next_address) = offset_address(address, delta) else {
                        break;
                    };
                    let degree_index =
                        candidates.len().saturating_add(1).min(u32::MAX as usize) as u32;
                    candidates.push(DcptPrefetchCandidate::new(
                        next_address,
                        access.address(),
                        context,
                        access.pc(),
                        access.secure(),
                        delta,
                        degree_index,
                    ));
                    address = next_address;
                }
                break;
            }
        }

        candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DcptPrefetchContext {
    entries: Vec<DcptPrefetchEntry>,
    next_victim: usize,
}

impl DcptPrefetchContext {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_victim: 0,
        }
    }

    fn snapshot(&self, context: AgentId) -> DcptPrefetchContextSnapshot {
        DcptPrefetchContextSnapshot {
            context,
            entries: self
                .entries
                .iter()
                .map(DcptPrefetchEntry::snapshot)
                .collect(),
            next_victim: self.next_victim,
        }
    }

    fn from_snapshot(snapshot: &DcptPrefetchContextSnapshot) -> Self {
        Self {
            entries: snapshot
                .entries()
                .iter()
                .map(DcptPrefetchEntry::from_snapshot)
                .collect(),
            next_victim: snapshot.next_victim(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DcptPrefetcher {
    config: DcptPrefetcherConfig,
    contexts: BTreeMap<AgentId, DcptPrefetchContext>,
    last_candidates: Vec<DcptPrefetchCandidate>,
}

impl DcptPrefetcher {
    pub fn new(config: DcptPrefetcherConfig) -> Self {
        Self {
            config,
            contexts: BTreeMap::new(),
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &DcptPrefetcherConfig {
        &self.config
    }

    pub fn context_count(&self) -> usize {
        self.contexts.len()
    }

    pub fn entry_count(&self, requestor: AgentId) -> usize {
        let context = self.context_key(requestor);
        self.contexts
            .get(&context)
            .map_or(0, |context| context.entries.len())
    }

    pub fn last_candidates(&self) -> &[DcptPrefetchCandidate] {
        &self.last_candidates
    }

    pub fn observe(
        &mut self,
        access: DcptPrefetchAccess,
    ) -> Result<&[DcptPrefetchCandidate], DcptPrefetcherError> {
        self.last_candidates.clear();
        let context_key = self.context_key(access.requestor());
        let config = self.config.clone();
        let context = self
            .contexts
            .entry(context_key)
            .or_insert_with(DcptPrefetchContext::new);

        let Some(index) = context
            .entries
            .iter()
            .position(|entry| entry.pc == access.pc() && entry.secure == access.secure())
        else {
            allocate_entry(
                &config,
                context,
                access.pc(),
                access.secure(),
                access.address(),
            );
            return Ok(&self.last_candidates);
        };

        let entry = &mut context.entries[index];
        entry.push_address(&config, access.address());
        self.last_candidates = entry.candidates(access, context_key, &config);
        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> DcptPrefetcherSnapshot {
        DcptPrefetcherSnapshot {
            config: self.config.clone(),
            contexts: self
                .contexts
                .iter()
                .map(|(context, table)| table.snapshot(*context))
                .collect(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &DcptPrefetcherSnapshot,
    ) -> Result<(), DcptPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(DcptPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }

        let mut contexts = BTreeMap::new();
        for context in snapshot.contexts() {
            if context.entries().len() > self.config.table_entries() {
                return Err(DcptPrefetcherError::SnapshotContextOutOfRange {
                    context: context.context(),
                    entries: context.entries().len(),
                    table_entries: self.config.table_entries(),
                });
            }
            for entry in context.entries() {
                if entry.deltas().len() != self.config.deltas_per_entry() {
                    return Err(DcptPrefetcherError::SnapshotEntryShapeMismatch {
                        pc: entry.pc(),
                        deltas: entry.deltas().len(),
                        expected: self.config.deltas_per_entry(),
                    });
                }
            }
            contexts.insert(
                context.context(),
                DcptPrefetchContext::from_snapshot(context),
            );
        }

        self.contexts = contexts;
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }

    fn context_key(&self, requestor: AgentId) -> AgentId {
        if self.config.use_requestor_id() {
            requestor
        } else {
            AgentId::new(0)
        }
    }
}

fn allocate_entry(
    config: &DcptPrefetcherConfig,
    context: &mut DcptPrefetchContext,
    pc: u64,
    secure: bool,
    last_address: Address,
) {
    let entry = DcptPrefetchEntry::new(config, pc, secure, last_address);
    if context.entries.len() == config.table_entries() {
        let victim_index = context.next_victim % context.entries.len();
        context.entries[victim_index] = entry;
        context.next_victim = (victim_index + 1) % config.table_entries();
    } else {
        context.entries.push(entry);
    }
}

fn clamp_delta(delta: i128, delta_bits: u32) -> i64 {
    let max_positive_delta = (1_i128 << (delta_bits - 1)) - 1;
    let min_negative_delta = -(1_i128 << (delta_bits - 1));
    if delta > max_positive_delta || delta < min_negative_delta {
        0
    } else {
        delta as i64
    }
}

fn masked_delta(delta: i64, mask_bits: u32) -> i64 {
    delta >> mask_bits
}

fn offset_address(address: Address, delta: i64) -> Option<Address> {
    let next = address.get() as i128 + delta as i128;
    if !(0..=u64::MAX as i128).contains(&next) {
        return None;
    }
    Some(Address::new(next as u64))
}
