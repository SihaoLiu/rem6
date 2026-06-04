use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, MemoryOperation, MemoryRequest};
use rem6_transport::TransportQosClass;

use crate::allocation::max_vector_len;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MshrHandle(u64);

impl MshrHandle {
    pub const fn new(index: u64) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MshrTargetSource {
    Demand,
    Snoop,
    Prefetch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MshrTargetPostFillAction {
    SatisfyLocally,
    ForwardDownstream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MshrQosClass {
    requestor: u32,
    priority: u8,
}

impl MshrQosClass {
    pub const fn new(requestor: u32, priority: u8) -> Self {
        Self {
            requestor,
            priority,
        }
    }

    pub const fn requestor(self) -> u32 {
        self.requestor
    }

    pub const fn priority(self) -> u8 {
        self.priority
    }

    pub const fn transport_qos_class(self) -> TransportQosClass {
        TransportQosClass::from_raw(self.requestor, self.priority)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MshrQosProfile {
    entry_count: usize,
    target_count: usize,
    qos_target_count: usize,
    effective_entry_count: usize,
    priority_targets: BTreeMap<u8, usize>,
    requestor_targets: BTreeMap<u32, usize>,
    effective_priority_entries: BTreeMap<u8, usize>,
    effective_requestor_entries: BTreeMap<u32, usize>,
    best_effective_priority: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrQueueConfig {
    entries: usize,
    targets_per_mshr: usize,
    demand_reserve: usize,
}

impl MshrQueueConfig {
    pub fn new(
        entries: usize,
        targets_per_mshr: usize,
        demand_reserve: usize,
    ) -> Result<Self, MshrQueueError> {
        if entries == 0 {
            return Err(MshrQueueError::ZeroEntries);
        }
        validate_mshr_vector_length("entries", entries, maximum_mshr_entries())?;
        if targets_per_mshr == 0 {
            return Err(MshrQueueError::ZeroTargetsPerMshr);
        }
        validate_mshr_vector_length(
            "targets per MSHR",
            targets_per_mshr,
            maximum_mshr_targets_per_entry(),
        )?;
        if demand_reserve >= entries {
            return Err(MshrQueueError::DemandReserveExceedsEntries {
                demand_reserve,
                entries,
            });
        }
        Ok(Self {
            entries,
            targets_per_mshr,
            demand_reserve,
        })
    }

    pub const fn entries(&self) -> usize {
        self.entries
    }

    pub const fn targets_per_mshr(&self) -> usize {
        self.targets_per_mshr
    }

    pub const fn demand_reserve(&self) -> usize {
        self.demand_reserve
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MshrQueueError {
    ZeroEntries,
    ZeroTargetsPerMshr,
    DemandReserveExceedsEntries {
        demand_reserve: usize,
        entries: usize,
    },
    VectorLengthTooLarge {
        field: &'static str,
        length: usize,
        maximum: usize,
    },
    EntrySlotsFull {
        entries: usize,
    },
    TargetSlotsFull {
        handle: MshrHandle,
        line: Address,
        targets_per_mshr: usize,
    },
    PrefetchReserveBlocked {
        allocated: usize,
        entries: usize,
        demand_reserve: usize,
    },
    UnknownEntry {
        handle: MshrHandle,
    },
    EntryAlreadyInService {
        handle: MshrHandle,
    },
    EntryNotInService {
        handle: MshrHandle,
    },
    SnapshotConfigMismatch {
        expected: MshrQueueConfig,
        actual: MshrQueueConfig,
    },
    SnapshotTooManyEntries {
        entries: usize,
        max_entries: usize,
    },
    SnapshotTooManyTargets {
        handle: MshrHandle,
        targets: usize,
        max_targets: usize,
    },
    SnapshotEmptyTargets {
        handle: MshrHandle,
    },
    SnapshotTargetLineMismatch {
        handle: MshrHandle,
        entry_line: Address,
        target_line: Address,
    },
    DuplicateSnapshotHandle {
        handle: MshrHandle,
    },
    SnapshotNextHandleNotAfterEntry {
        next_handle: u64,
        handle: MshrHandle,
    },
    SnapshotNextOrderNotAfterEntry {
        next_order: u64,
        handle: MshrHandle,
        order: u64,
    },
    SnapshotNextOrderNotAfterTarget {
        next_order: u64,
        handle: MshrHandle,
        order: u64,
    },
}

impl fmt::Display for MshrQueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroEntries => write!(formatter, "MSHR queue has no entries"),
            Self::ZeroTargetsPerMshr => write!(formatter, "MSHR queue entries have no targets"),
            Self::DemandReserveExceedsEntries {
                demand_reserve,
                entries,
            } => write!(
                formatter,
                "MSHR demand reserve {demand_reserve} leaves no entry in a {entries}-entry queue"
            ),
            Self::VectorLengthTooLarge {
                field,
                length,
                maximum,
            } => write!(
                formatter,
                "MSHR queue {field} length {length} exceeds maximum {maximum}"
            ),
            Self::EntrySlotsFull { entries } => {
                write!(formatter, "MSHR queue has all {entries} entries allocated")
            }
            Self::TargetSlotsFull {
                handle,
                line,
                targets_per_mshr,
            } => write!(
                formatter,
                "MSHR {:?} for line {:#x} already has {targets_per_mshr} targets",
                handle,
                line.get()
            ),
            Self::PrefetchReserveBlocked {
                allocated,
                entries,
                demand_reserve,
            } => write!(
                formatter,
                "MSHR prefetch blocked with {allocated}/{entries} entries allocated and {demand_reserve} reserved for demand"
            ),
            Self::UnknownEntry { handle } => write!(formatter, "unknown MSHR entry {handle:?}"),
            Self::EntryAlreadyInService { handle } => {
                write!(formatter, "MSHR entry {handle:?} is already in service")
            }
            Self::EntryNotInService { handle } => {
                write!(formatter, "MSHR entry {handle:?} is not in service")
            }
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "MSHR snapshot config {actual:?} does not match queue config {expected:?}"
            ),
            Self::SnapshotTooManyEntries {
                entries,
                max_entries,
            } => write!(
                formatter,
                "MSHR snapshot has {entries} entries for {max_entries} slots"
            ),
            Self::SnapshotTooManyTargets {
                handle,
                targets,
                max_targets,
            } => write!(
                formatter,
                "MSHR snapshot entry {handle:?} has {targets} targets for {max_targets} slots"
            ),
            Self::SnapshotEmptyTargets { handle } => {
                write!(formatter, "MSHR snapshot entry {handle:?} has no targets")
            }
            Self::SnapshotTargetLineMismatch {
                handle,
                entry_line,
                target_line,
            } => write!(
                formatter,
                "MSHR snapshot entry {handle:?} for line {:#x} contains target for line {:#x}",
                entry_line.get(),
                target_line.get()
            ),
            Self::DuplicateSnapshotHandle { handle } => {
                write!(formatter, "MSHR snapshot repeats entry handle {handle:?}")
            }
            Self::SnapshotNextHandleNotAfterEntry {
                next_handle,
                handle,
            } => write!(
                formatter,
                "MSHR snapshot next handle {next_handle} is not after entry handle {handle:?}"
            ),
            Self::SnapshotNextOrderNotAfterEntry {
                next_order,
                handle,
                order,
            } => write!(
                formatter,
                "MSHR snapshot next order {next_order} is not after entry {handle:?} order {order}"
            ),
            Self::SnapshotNextOrderNotAfterTarget {
                next_order,
                handle,
                order,
            } => write!(
                formatter,
                "MSHR snapshot next order {next_order} is not after entry {handle:?} target order {order}"
            ),
        }
    }
}

impl Error for MshrQueueError {}

fn maximum_mshr_entries() -> usize {
    max_vector_len::<MshrEntry>()
        .min(max_vector_len::<MshrHandle>())
        .min(max_vector_len::<usize>())
}

fn maximum_mshr_targets_per_entry() -> usize {
    max_vector_len::<MshrTarget>().min(max_vector_len::<MemoryRequest>())
}

fn validate_mshr_vector_length(
    field: &'static str,
    length: usize,
    maximum: usize,
) -> Result<(), MshrQueueError> {
    if length > maximum {
        return Err(MshrQueueError::VectorLengthTooLarge {
            field,
            length,
            maximum,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrTarget {
    request: MemoryRequest,
    ready_tick: u64,
    order: u64,
    source: MshrTargetSource,
    alloc_on_fill: bool,
    qos: Option<MshrQosClass>,
}

impl MshrTarget {
    fn new(
        request: MemoryRequest,
        ready_tick: u64,
        order: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) -> Self {
        Self {
            request,
            ready_tick,
            order,
            source,
            alloc_on_fill,
            qos,
        }
    }

    pub fn from_parts(
        request: MemoryRequest,
        ready_tick: u64,
        order: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) -> Self {
        Self::new(request, ready_tick, order, source, alloc_on_fill, qos)
    }

    pub const fn request(&self) -> &MemoryRequest {
        &self.request
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub const fn source(&self) -> MshrTargetSource {
        self.source
    }

    pub const fn alloc_on_fill(&self) -> bool {
        self.alloc_on_fill
    }

    pub const fn qos(&self) -> Option<MshrQosClass> {
        self.qos
    }

    pub const fn post_fill_action(&self) -> MshrTargetPostFillAction {
        match self.request.operation() {
            MemoryOperation::WriteClean
            | MemoryOperation::WritebackClean
            | MemoryOperation::WritebackDirty
            | MemoryOperation::CleanShared
            | MemoryOperation::CleanEvict
            | MemoryOperation::Invalidate => MshrTargetPostFillAction::ForwardDownstream,
            _ => MshrTargetPostFillAction::SatisfyLocally,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrEntry {
    handle: MshrHandle,
    line: Address,
    ready_tick: u64,
    order: u64,
    in_service: bool,
    pending_modified: bool,
    targets: Vec<MshrTarget>,
}

impl MshrEntry {
    fn new(
        handle: MshrHandle,
        request: MemoryRequest,
        ready_tick: u64,
        order: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) -> Self {
        let line = request.line_address();
        let target = MshrTarget::new(request, ready_tick, order, source, alloc_on_fill, qos);
        Self {
            handle,
            line,
            ready_tick,
            order,
            in_service: false,
            pending_modified: false,
            targets: vec![target],
        }
    }

    pub fn from_parts(
        handle: MshrHandle,
        line: Address,
        ready_tick: u64,
        order: u64,
        in_service: bool,
        pending_modified: bool,
        targets: Vec<MshrTarget>,
    ) -> Self {
        Self {
            handle,
            line,
            ready_tick,
            order,
            in_service,
            pending_modified,
            targets,
        }
    }

    fn add_target(
        &mut self,
        request: MemoryRequest,
        ready_tick: u64,
        order: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) {
        self.targets.push(MshrTarget::new(
            request,
            ready_tick,
            order,
            source,
            alloc_on_fill,
            qos,
        ));
    }

    pub const fn handle(&self) -> MshrHandle {
        self.handle
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn ready_tick(&self) -> u64 {
        self.ready_tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub const fn in_service(&self) -> bool {
        self.in_service
    }

    pub const fn pending_modified(&self) -> bool {
        self.pending_modified
    }

    pub fn targets(&self) -> &[MshrTarget] {
        &self.targets
    }

    pub fn effective_qos(&self) -> Option<MshrQosClass> {
        self.targets
            .iter()
            .filter_map(|target| {
                target
                    .qos()
                    .map(|qos| (qos.priority(), target.order(), qos))
            })
            .min_by_key(|(priority, order, _)| (*priority, *order))
            .map(|(_, _, qos)| qos)
    }

    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    pub fn can_merge_request(&self, request: &MemoryRequest) -> bool {
        self.line == request.line_address()
            && !request.is_uncacheable()
            && !request.is_strict_ordered()
            && self.targets.iter().all(|target| {
                !target.request().is_uncacheable() && !target.request().is_strict_ordered()
            })
    }
}

impl MshrQosProfile {
    pub fn from_entries<'a, I>(entries: I) -> Self
    where
        I: IntoIterator<Item = &'a MshrEntry>,
    {
        let mut profile = Self::default();

        for entry in entries {
            profile.entry_count += 1;
            profile.target_count += entry.target_count();

            for target in entry.targets() {
                if let Some(qos) = target.qos() {
                    profile.qos_target_count += 1;
                    *profile.priority_targets.entry(qos.priority()).or_insert(0) += 1;
                    *profile
                        .requestor_targets
                        .entry(qos.requestor())
                        .or_insert(0) += 1;
                }
            }

            if let Some(qos) = entry.effective_qos() {
                profile.effective_entry_count += 1;
                *profile
                    .effective_priority_entries
                    .entry(qos.priority())
                    .or_insert(0) += 1;
                *profile
                    .effective_requestor_entries
                    .entry(qos.requestor())
                    .or_insert(0) += 1;
                profile.best_effective_priority = Some(
                    profile
                        .best_effective_priority
                        .map_or(qos.priority(), |priority| priority.min(qos.priority())),
                );
            }
        }

        profile
    }

    pub fn from_profiles<I>(profiles: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        profiles
            .into_iter()
            .fold(Self::default(), |merged, profile| merged.merge(profile))
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.entry_count += other.entry_count;
        self.target_count += other.target_count;
        self.qos_target_count += other.qos_target_count;
        self.effective_entry_count += other.effective_entry_count;
        merge_counts(&mut self.priority_targets, other.priority_targets);
        merge_counts(&mut self.requestor_targets, other.requestor_targets);
        merge_counts(
            &mut self.effective_priority_entries,
            other.effective_priority_entries,
        );
        merge_counts(
            &mut self.effective_requestor_entries,
            other.effective_requestor_entries,
        );
        self.best_effective_priority =
            match (self.best_effective_priority, other.best_effective_priority) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (Some(priority), None) | (None, Some(priority)) => Some(priority),
                (None, None) => None,
            };
        self
    }

    pub const fn entry_count(&self) -> usize {
        self.entry_count
    }

    pub const fn target_count(&self) -> usize {
        self.target_count
    }

    pub const fn qos_target_count(&self) -> usize {
        self.qos_target_count
    }

    pub const fn effective_entry_count(&self) -> usize {
        self.effective_entry_count
    }

    pub const fn has_qos(&self) -> bool {
        self.qos_target_count != 0
    }

    pub const fn is_empty(&self) -> bool {
        self.entry_count == 0 && self.target_count == 0
    }

    pub fn priority_target_count(&self, priority: u8) -> usize {
        self.priority_targets.get(&priority).copied().unwrap_or(0)
    }

    pub fn priority_target_counts(&self) -> BTreeMap<u8, usize> {
        self.priority_targets.clone()
    }

    pub fn requestor_target_count(&self, requestor: u32) -> usize {
        self.requestor_targets.get(&requestor).copied().unwrap_or(0)
    }

    pub fn requestor_target_counts(&self) -> BTreeMap<u32, usize> {
        self.requestor_targets.clone()
    }

    pub fn effective_priority_entry_count(&self, priority: u8) -> usize {
        self.effective_priority_entries
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn effective_priority_entry_counts(&self) -> BTreeMap<u8, usize> {
        self.effective_priority_entries.clone()
    }

    pub fn effective_requestor_entry_count(&self, requestor: u32) -> usize {
        self.effective_requestor_entries
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn effective_requestor_entry_counts(&self) -> BTreeMap<u32, usize> {
        self.effective_requestor_entries.clone()
    }

    pub const fn best_effective_priority(&self) -> Option<u8> {
        self.best_effective_priority
    }
}

fn merge_counts<K>(counts: &mut BTreeMap<K, usize>, other: BTreeMap<K, usize>)
where
    K: Ord,
{
    for (key, value) in other {
        *counts.entry(key).or_insert(0) += value;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrQueueUpdate {
    handle: MshrHandle,
    line: Address,
    allocated_new_entry: bool,
    target_count: usize,
    allocated_count: usize,
}

impl MshrQueueUpdate {
    fn new(entry: &MshrEntry, allocated_new_entry: bool, allocated_count: usize) -> Self {
        Self {
            handle: entry.handle,
            line: entry.line,
            allocated_new_entry,
            target_count: entry.target_count(),
            allocated_count,
        }
    }

    pub const fn handle(&self) -> MshrHandle {
        self.handle
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn allocated_new_entry(&self) -> bool {
        self.allocated_new_entry
    }

    pub const fn target_count(&self) -> usize {
        self.target_count
    }

    pub const fn allocated_count(&self) -> usize {
        self.allocated_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrCompletion {
    handle: MshrHandle,
    line: Address,
    targets: Vec<MshrTarget>,
}

impl MshrCompletion {
    fn new(entry: MshrEntry) -> Self {
        Self {
            handle: entry.handle,
            line: entry.line,
            targets: entry.targets,
        }
    }

    pub const fn handle(&self) -> MshrHandle {
        self.handle
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub fn targets(&self) -> &[MshrTarget] {
        &self.targets
    }

    pub fn local_targets(&self) -> impl Iterator<Item = &MshrTarget> {
        self.targets
            .iter()
            .filter(|target| target.post_fill_action() == MshrTargetPostFillAction::SatisfyLocally)
    }

    pub fn post_fill_downstream_requests(&self) -> Vec<MemoryRequest> {
        self.targets
            .iter()
            .filter(|target| {
                target.post_fill_action() == MshrTargetPostFillAction::ForwardDownstream
            })
            .map(|target| target.request().clone())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrQueueSnapshot {
    config: MshrQueueConfig,
    entries: Vec<MshrEntry>,
    next_handle: u64,
    next_order: u64,
}

impl MshrQueueSnapshot {
    pub fn new(
        config: MshrQueueConfig,
        entries: Vec<MshrEntry>,
        next_handle: u64,
        next_order: u64,
    ) -> Self {
        Self {
            config,
            entries,
            next_handle,
            next_order,
        }
    }

    pub const fn config(&self) -> &MshrQueueConfig {
        &self.config
    }

    pub fn entries(&self) -> &[MshrEntry] {
        &self.entries
    }

    pub const fn next_handle(&self) -> u64 {
        self.next_handle
    }

    pub const fn next_order(&self) -> u64 {
        self.next_order
    }

    pub fn qos_profile(&self) -> MshrQosProfile {
        MshrQosProfile::from_entries(&self.entries)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MshrQueue {
    config: MshrQueueConfig,
    entries: Vec<MshrEntry>,
    next_handle: u64,
    next_order: u64,
}

impl MshrQueue {
    pub fn new(config: MshrQueueConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
            next_handle: 0,
            next_order: 0,
        }
    }

    pub const fn config(&self) -> &MshrQueueConfig {
        &self.config
    }

    pub fn allocated_count(&self) -> usize {
        self.entries.len()
    }

    pub fn in_service_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.in_service).count()
    }

    pub fn qos_profile(&self) -> MshrQosProfile {
        MshrQosProfile::from_entries(&self.entries)
    }

    pub fn can_accept_prefetch(&self) -> bool {
        self.entries.len()
            < self
                .config
                .entries
                .saturating_sub(self.config.demand_reserve + 1)
    }

    pub fn can_allocate_entry(&self, source: MshrTargetSource) -> Result<(), MshrQueueError> {
        if source == MshrTargetSource::Prefetch && !self.can_accept_prefetch() {
            return Err(MshrQueueError::PrefetchReserveBlocked {
                allocated: self.entries.len(),
                entries: self.config.entries,
                demand_reserve: self.config.demand_reserve,
            });
        }
        if self.entries.len() >= self.config.entries {
            return Err(MshrQueueError::EntrySlotsFull {
                entries: self.config.entries,
            });
        }

        Ok(())
    }

    pub fn entry(&self, handle: MshrHandle) -> Result<&MshrEntry, MshrQueueError> {
        self.entries
            .iter()
            .find(|entry| entry.handle == handle)
            .ok_or(MshrQueueError::UnknownEntry { handle })
    }

    pub fn find_line(&self, line: Address) -> Option<&MshrEntry> {
        self.entries.iter().find(|entry| entry.line == line)
    }

    pub fn allocate_or_merge(
        &mut self,
        request: MemoryRequest,
        ready_tick: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        self.allocate_or_merge_inner(request, ready_tick, source, alloc_on_fill, None)
    }

    pub fn allocate_or_merge_with_qos(
        &mut self,
        request: MemoryRequest,
        ready_tick: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: MshrQosClass,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        self.allocate_or_merge_inner(request, ready_tick, source, alloc_on_fill, Some(qos))
    }

    pub(crate) fn allocate_or_merge_optional_qos(
        &mut self,
        request: MemoryRequest,
        ready_tick: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        self.allocate_or_merge_inner(request, ready_tick, source, alloc_on_fill, qos)
    }

    fn allocate_or_merge_inner(
        &mut self,
        request: MemoryRequest,
        ready_tick: u64,
        source: MshrTargetSource,
        alloc_on_fill: bool,
        qos: Option<MshrQosClass>,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        let line = request.line_address();
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.can_merge_request(&request))
        {
            let allocated_count = self.entries.len();
            let entry = &mut self.entries[index];
            if entry.target_count() >= self.config.targets_per_mshr {
                return Err(MshrQueueError::TargetSlotsFull {
                    handle: entry.handle,
                    line,
                    targets_per_mshr: self.config.targets_per_mshr,
                });
            }
            let order = self.next_order();
            let entry = &mut self.entries[index];
            entry.add_target(request, ready_tick, order, source, alloc_on_fill, qos);
            return Ok(MshrQueueUpdate::new(entry, false, allocated_count));
        }

        self.can_allocate_entry(source)?;

        let entry = MshrEntry::new(
            self.next_handle(),
            request,
            ready_tick,
            self.next_order(),
            source,
            alloc_on_fill,
            qos,
        );
        self.entries.push(entry);
        let allocated_count = self.entries.len();
        let entry = self.entries.last().expect("entry was just pushed");
        Ok(MshrQueueUpdate::new(entry, true, allocated_count))
    }

    pub fn ready_handles(&self, tick: u64) -> Vec<MshrHandle> {
        let mut pending = self
            .entries
            .iter()
            .filter(|entry| !entry.in_service && entry.ready_tick <= tick)
            .collect::<Vec<_>>();
        let mut ordered = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let eligible = ordering_eligible_entries(&pending);
            let grant_index = eligible
                .into_iter()
                .min_by_key(|index| mshr_ready_sort_key(pending[*index]))
                .expect("ready MSHR eligibility must produce a candidate");
            ordered.push(pending.remove(grant_index).handle);
        }
        ordered
    }

    pub fn mark_in_service(
        &mut self,
        handle: MshrHandle,
        pending_modified: bool,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        let allocated_count = self.entries.len();
        let entry = self.entry_mut(handle)?;
        if entry.in_service {
            return Err(MshrQueueError::EntryAlreadyInService { handle });
        }
        entry.in_service = true;
        entry.pending_modified = pending_modified;
        Ok(MshrQueueUpdate::new(entry, false, allocated_count))
    }

    pub fn mark_pending(&mut self, handle: MshrHandle) -> Result<MshrQueueUpdate, MshrQueueError> {
        let allocated_count = self.entries.len();
        let entry = self.entry_mut(handle)?;
        if !entry.in_service {
            return Err(MshrQueueError::EntryNotInService { handle });
        }
        entry.in_service = false;
        entry.pending_modified = false;
        Ok(MshrQueueUpdate::new(entry, false, allocated_count))
    }

    pub fn delay_until(
        &mut self,
        handle: MshrHandle,
        ready_tick: u64,
    ) -> Result<MshrQueueUpdate, MshrQueueError> {
        let allocated_count = self.entries.len();
        let entry = self.entry_mut(handle)?;
        entry.ready_tick = ready_tick;
        Ok(MshrQueueUpdate::new(entry, false, allocated_count))
    }

    pub fn complete(&mut self, handle: MshrHandle) -> Result<MshrCompletion, MshrQueueError> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.handle == handle)
            .ok_or(MshrQueueError::UnknownEntry { handle })?;
        Ok(MshrCompletion::new(self.entries.remove(index)))
    }

    pub fn snapshot(&self) -> MshrQueueSnapshot {
        MshrQueueSnapshot::new(
            self.config.clone(),
            self.entries.clone(),
            self.next_handle,
            self.next_order,
        )
    }

    pub fn restore(&mut self, snapshot: &MshrQueueSnapshot) -> Result<(), MshrQueueError> {
        if self.config != snapshot.config {
            return Err(MshrQueueError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config.clone(),
            });
        }
        self.validate_snapshot_shape(snapshot)?;
        self.entries.clone_from(&snapshot.entries);
        self.next_handle = snapshot.next_handle;
        self.next_order = snapshot.next_order;
        Ok(())
    }

    fn validate_snapshot_shape(&self, snapshot: &MshrQueueSnapshot) -> Result<(), MshrQueueError> {
        if snapshot.entries.len() > self.config.entries {
            return Err(MshrQueueError::SnapshotTooManyEntries {
                entries: snapshot.entries.len(),
                max_entries: self.config.entries,
            });
        }
        let mut handles = BTreeSet::new();
        for entry in &snapshot.entries {
            if entry.target_count() > self.config.targets_per_mshr {
                return Err(MshrQueueError::SnapshotTooManyTargets {
                    handle: entry.handle(),
                    targets: entry.target_count(),
                    max_targets: self.config.targets_per_mshr,
                });
            }
            if entry.targets().is_empty() {
                return Err(MshrQueueError::SnapshotEmptyTargets {
                    handle: entry.handle(),
                });
            }
            if !handles.insert(entry.handle()) {
                return Err(MshrQueueError::DuplicateSnapshotHandle {
                    handle: entry.handle(),
                });
            }
            if snapshot.next_handle <= entry.handle().index() {
                return Err(MshrQueueError::SnapshotNextHandleNotAfterEntry {
                    next_handle: snapshot.next_handle,
                    handle: entry.handle(),
                });
            }
            if snapshot.next_order <= entry.order() {
                return Err(MshrQueueError::SnapshotNextOrderNotAfterEntry {
                    next_order: snapshot.next_order,
                    handle: entry.handle(),
                    order: entry.order(),
                });
            }
            for target in entry.targets() {
                let target_line = target.request().line_address();
                if target_line != entry.line() {
                    return Err(MshrQueueError::SnapshotTargetLineMismatch {
                        handle: entry.handle(),
                        entry_line: entry.line(),
                        target_line,
                    });
                }
                if snapshot.next_order <= target.order() {
                    return Err(MshrQueueError::SnapshotNextOrderNotAfterTarget {
                        next_order: snapshot.next_order,
                        handle: entry.handle(),
                        order: target.order(),
                    });
                }
            }
        }
        Ok(())
    }

    fn entry_mut(&mut self, handle: MshrHandle) -> Result<&mut MshrEntry, MshrQueueError> {
        self.entries
            .iter_mut()
            .find(|entry| entry.handle == handle)
            .ok_or(MshrQueueError::UnknownEntry { handle })
    }

    fn next_handle(&mut self) -> MshrHandle {
        let handle = MshrHandle::new(self.next_handle);
        self.next_handle = self.next_handle.saturating_add(1);
        handle
    }

    fn next_order(&mut self) -> u64 {
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        order
    }
}

fn ordering_eligible_entries(pending: &[&MshrEntry]) -> Vec<usize> {
    let eligible = pending
        .iter()
        .enumerate()
        .filter_map(|(candidate_index, candidate)| {
            let blocked = pending
                .iter()
                .any(|other| mshr_entry_orders_before(other, candidate));
            (!blocked).then_some(candidate_index)
        })
        .collect::<Vec<_>>();
    debug_assert!(
        !eligible.is_empty(),
        "oldest ready MSHR entry is always ordering-eligible"
    );
    eligible
}

fn mshr_entry_orders_before(earlier: &MshrEntry, later: &MshrEntry) -> bool {
    earlier.targets().iter().any(|earlier_target| {
        later.targets().iter().any(|later_target| {
            earlier_target.order() < later_target.order()
                && earlier_target
                    .request()
                    .orders_before(later_target.request())
        })
    })
}

fn mshr_ready_sort_key(entry: &MshrEntry) -> (u8, u64, u64, MshrHandle) {
    (
        entry
            .effective_qos()
            .map(MshrQosClass::priority)
            .unwrap_or(u8::MAX),
        entry.ready_tick,
        entry.order,
        entry.handle,
    )
}
