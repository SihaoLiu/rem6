use std::collections::{BTreeMap, BTreeSet};

use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_memory::MemoryTargetId;

use crate::{DramAccess, DramAccessKind, DramMemoryTechnology, ExternalMemoryProfile};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramActivityMarker {
    pub(crate) offset: usize,
}

impl DramActivityMarker {
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DramBankActivity {
    access_count: usize,
    read_byte_count: u64,
    write_byte_count: u64,
    row_hit_count: usize,
    row_miss_count: usize,
    command_count: usize,
    first_arrival_cycle: u64,
    last_ready_cycle: u64,
    total_ready_latency_cycles: u64,
    max_ready_latency_cycles: u64,
    qos_access_count: usize,
    qos_byte_count: u64,
    qos_escalated_access_count: usize,
    qos_priority_access_counts: BTreeMap<QosPriority, usize>,
    qos_priority_byte_counts: BTreeMap<QosPriority, u64>,
    qos_requestor_access_counts: BTreeMap<QosRequestorId, usize>,
    qos_requestor_byte_counts: BTreeMap<QosRequestorId, u64>,
}

impl DramBankActivity {
    pub(crate) fn record(&mut self, access: &DramAccess) {
        if self.access_count == 0 {
            self.first_arrival_cycle = access.arrival_cycle();
        } else {
            self.first_arrival_cycle = self.first_arrival_cycle.min(access.arrival_cycle());
        }
        self.access_count += 1;
        match access.kind() {
            DramAccessKind::Read => self.read_byte_count += access.byte_count(),
            DramAccessKind::Write => self.write_byte_count += access.byte_count(),
        }
        if access.row_hit() {
            self.row_hit_count += 1;
        } else {
            self.row_miss_count += 1;
        }
        self.command_count += access.commands().len();
        self.last_ready_cycle = self.last_ready_cycle.max(access.ready_cycle());
        let ready_latency = access.ready_cycle() - access.arrival_cycle();
        self.total_ready_latency_cycles += ready_latency;
        self.max_ready_latency_cycles = self.max_ready_latency_cycles.max(ready_latency);
        if let Some(qos) = access.qos() {
            self.qos_access_count += 1;
            self.qos_byte_count += qos.bytes();
            if qos.escalated() {
                self.qos_escalated_access_count += 1;
            }
            *self
                .qos_priority_access_counts
                .entry(qos.effective_priority())
                .or_default() += 1;
            *self
                .qos_priority_byte_counts
                .entry(qos.effective_priority())
                .or_default() += qos.bytes();
            *self
                .qos_requestor_access_counts
                .entry(qos.requestor())
                .or_default() += 1;
            *self
                .qos_requestor_byte_counts
                .entry(qos.requestor())
                .or_default() += qos.bytes();
        }
    }

    pub const fn access_count(&self) -> usize {
        self.access_count
    }

    pub const fn read_byte_count(&self) -> u64 {
        self.read_byte_count
    }

    pub const fn write_byte_count(&self) -> u64 {
        self.write_byte_count
    }

    pub const fn row_hit_count(&self) -> usize {
        self.row_hit_count
    }

    pub const fn row_miss_count(&self) -> usize {
        self.row_miss_count
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn first_arrival_cycle(&self) -> u64 {
        self.first_arrival_cycle
    }

    pub const fn last_ready_cycle(&self) -> u64 {
        self.last_ready_cycle
    }

    pub const fn total_ready_latency_cycles(&self) -> u64 {
        self.total_ready_latency_cycles
    }

    pub const fn max_ready_latency_cycles(&self) -> u64 {
        self.max_ready_latency_cycles
    }

    pub const fn has_row_misses(&self) -> bool {
        self.row_miss_count != 0
    }

    pub const fn qos_access_count(&self) -> usize {
        self.qos_access_count
    }

    pub const fn qos_byte_count(&self) -> u64 {
        self.qos_byte_count
    }

    pub const fn qos_escalated_access_count(&self) -> usize {
        self.qos_escalated_access_count
    }

    pub fn qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.qos_priority_access_counts
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.qos_priority_byte_counts
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_priorities(&self) -> Vec<QosPriority> {
        let mut priorities = BTreeSet::new();
        priorities.extend(self.qos_priority_access_counts.keys().copied());
        priorities.extend(self.qos_priority_byte_counts.keys().copied());
        priorities.into_iter().collect()
    }

    pub fn qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.qos_requestor_access_counts
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.qos_requestor_byte_counts
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_requestors(&self) -> Vec<QosRequestorId> {
        let mut requestors = BTreeSet::new();
        requestors.extend(self.qos_requestor_access_counts.keys().copied());
        requestors.extend(self.qos_requestor_byte_counts.keys().copied());
        requestors.into_iter().collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DramPortActivity {
    access_count: usize,
    read_count: usize,
    write_count: usize,
    turnaround_count: usize,
    command_count: usize,
}

impl DramPortActivity {
    pub(crate) fn record(&mut self, access: &DramAccess, previous: Option<DramAccessKind>) {
        self.access_count += 1;
        match access.kind() {
            DramAccessKind::Read => self.read_count += 1,
            DramAccessKind::Write => self.write_count += 1,
        }
        if previous.is_some_and(|kind| kind != access.kind()) {
            self.turnaround_count += 1;
        }
        self.command_count += access.commands().len();
    }

    pub const fn access_count(self) -> usize {
        self.access_count
    }

    pub const fn read_count(self) -> usize {
        self.read_count
    }

    pub const fn write_count(self) -> usize {
        self.write_count
    }

    pub const fn turnaround_count(self) -> usize {
        self.turnaround_count
    }

    pub const fn command_count(self) -> usize {
        self.command_count
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DramActivityProfile {
    active_port_count: usize,
    active_bank_count: usize,
    access_count: usize,
    read_count: usize,
    write_count: usize,
    read_byte_count: u64,
    write_byte_count: u64,
    row_hit_count: usize,
    row_miss_count: usize,
    command_count: usize,
    turnaround_count: usize,
    total_ready_latency_cycles: u64,
    max_ready_latency_cycles: u64,
    qos_access_count: usize,
    qos_byte_count: u64,
    qos_escalated_access_count: usize,
    qos_priority_access_counts: BTreeMap<QosPriority, usize>,
    qos_priority_byte_counts: BTreeMap<QosPriority, u64>,
    qos_requestor_access_counts: BTreeMap<QosRequestorId, usize>,
    qos_requestor_byte_counts: BTreeMap<QosRequestorId, u64>,
}

impl DramActivityProfile {
    pub(crate) fn from_activities(
        ports: &BTreeMap<u32, DramPortActivity>,
        banks: &BTreeMap<(u32, u32), DramBankActivity>,
    ) -> Self {
        let mut profile = Self {
            active_port_count: ports.len(),
            active_bank_count: banks.len(),
            ..Self::default()
        };
        for port in ports.values() {
            profile.access_count += port.access_count();
            profile.read_count += port.read_count();
            profile.write_count += port.write_count();
            profile.command_count += port.command_count();
            profile.turnaround_count += port.turnaround_count();
        }
        for bank in banks.values() {
            profile.row_hit_count += bank.row_hit_count();
            profile.row_miss_count += bank.row_miss_count();
            profile.read_byte_count += bank.read_byte_count();
            profile.write_byte_count += bank.write_byte_count();
            profile.total_ready_latency_cycles += bank.total_ready_latency_cycles();
            profile.max_ready_latency_cycles = profile
                .max_ready_latency_cycles
                .max(bank.max_ready_latency_cycles());
            profile.qos_access_count += bank.qos_access_count();
            profile.qos_byte_count += bank.qos_byte_count();
            profile.qos_escalated_access_count += bank.qos_escalated_access_count();
            merge_count_map(
                &mut profile.qos_priority_access_counts,
                &bank.qos_priority_access_counts,
            );
            merge_count_map(
                &mut profile.qos_priority_byte_counts,
                &bank.qos_priority_byte_counts,
            );
            merge_count_map(
                &mut profile.qos_requestor_access_counts,
                &bank.qos_requestor_access_counts,
            );
            merge_count_map(
                &mut profile.qos_requestor_byte_counts,
                &bank.qos_requestor_byte_counts,
            );
        }
        profile
    }

    pub fn merge_window(mut self, later: Self) -> Self {
        self.active_port_count += later.active_port_count;
        self.active_bank_count += later.active_bank_count;
        self.access_count += later.access_count;
        self.read_count += later.read_count;
        self.write_count += later.write_count;
        self.read_byte_count += later.read_byte_count;
        self.write_byte_count += later.write_byte_count;
        self.row_hit_count += later.row_hit_count;
        self.row_miss_count += later.row_miss_count;
        self.command_count += later.command_count;
        self.turnaround_count += later.turnaround_count;
        self.total_ready_latency_cycles += later.total_ready_latency_cycles;
        self.max_ready_latency_cycles = self
            .max_ready_latency_cycles
            .max(later.max_ready_latency_cycles);
        self.qos_access_count += later.qos_access_count;
        self.qos_byte_count += later.qos_byte_count;
        self.qos_escalated_access_count += later.qos_escalated_access_count;
        merge_count_map(
            &mut self.qos_priority_access_counts,
            &later.qos_priority_access_counts,
        );
        merge_count_map(
            &mut self.qos_priority_byte_counts,
            &later.qos_priority_byte_counts,
        );
        merge_count_map(
            &mut self.qos_requestor_access_counts,
            &later.qos_requestor_access_counts,
        );
        merge_count_map(
            &mut self.qos_requestor_byte_counts,
            &later.qos_requestor_byte_counts,
        );
        self
    }

    pub const fn active_port_count(&self) -> usize {
        self.active_port_count
    }

    pub const fn active_bank_count(&self) -> usize {
        self.active_bank_count
    }

    pub const fn access_count(&self) -> usize {
        self.access_count
    }

    pub const fn read_count(&self) -> usize {
        self.read_count
    }

    pub const fn write_count(&self) -> usize {
        self.write_count
    }

    pub const fn read_byte_count(&self) -> u64 {
        self.read_byte_count
    }

    pub const fn write_byte_count(&self) -> u64 {
        self.write_byte_count
    }

    pub const fn row_hit_count(&self) -> usize {
        self.row_hit_count
    }

    pub const fn row_miss_count(&self) -> usize {
        self.row_miss_count
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn turnaround_count(&self) -> usize {
        self.turnaround_count
    }

    pub const fn total_ready_latency_cycles(&self) -> u64 {
        self.total_ready_latency_cycles
    }

    pub const fn max_ready_latency_cycles(&self) -> u64 {
        self.max_ready_latency_cycles
    }

    pub const fn has_row_misses(&self) -> bool {
        self.row_miss_count != 0
    }

    pub const fn is_empty(&self) -> bool {
        self.access_count == 0
    }

    pub const fn qos_access_count(&self) -> usize {
        self.qos_access_count
    }

    pub const fn qos_byte_count(&self) -> u64 {
        self.qos_byte_count
    }

    pub const fn qos_escalated_access_count(&self) -> usize {
        self.qos_escalated_access_count
    }

    pub fn qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.qos_priority_access_counts
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.qos_priority_byte_counts
            .get(&priority)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_priorities(&self) -> Vec<QosPriority> {
        let mut priorities = BTreeSet::new();
        priorities.extend(self.qos_priority_access_counts.keys().copied());
        priorities.extend(self.qos_priority_byte_counts.keys().copied());
        priorities.into_iter().collect()
    }

    pub fn qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.qos_requestor_access_counts
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.qos_requestor_byte_counts
            .get(&requestor)
            .copied()
            .unwrap_or(0)
    }

    pub fn qos_requestors(&self) -> Vec<QosRequestorId> {
        let mut requestors = BTreeSet::new();
        requestors.extend(self.qos_requestor_access_counts.keys().copied());
        requestors.extend(self.qos_requestor_byte_counts.keys().copied());
        requestors.into_iter().collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryActivityMarker {
    targets: BTreeMap<MemoryTargetId, DramActivityMarker>,
}

impl DramMemoryActivityMarker {
    pub(crate) fn new(targets: BTreeMap<MemoryTargetId, DramActivityMarker>) -> Self {
        Self { targets }
    }

    pub(crate) fn marker_for(&self, target: MemoryTargetId) -> Option<DramActivityMarker> {
        self.targets.get(&target).copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramTargetActivity {
    target: MemoryTargetId,
    profile: DramActivityProfile,
    memory_profile: Option<ExternalMemoryProfile>,
}

impl DramTargetActivity {
    pub const fn new(target: MemoryTargetId, profile: DramActivityProfile) -> Self {
        Self {
            target,
            profile,
            memory_profile: None,
        }
    }

    pub const fn with_memory_profile(mut self, memory_profile: ExternalMemoryProfile) -> Self {
        self.memory_profile = Some(memory_profile);
        self
    }

    pub fn merge_window(mut self, later: Self) -> Self {
        self.profile = self.profile.merge_window(later.profile);
        if self.memory_profile.is_none() {
            self.memory_profile = later.memory_profile;
        } else {
            debug_assert!(
                later.memory_profile.is_none() || self.memory_profile == later.memory_profile
            );
        }
        self
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn memory_profile(&self) -> Option<&ExternalMemoryProfile> {
        self.memory_profile.as_ref()
    }

    pub fn profile(&self) -> DramActivityProfile {
        self.profile.clone()
    }

    pub fn persistent_write_count(&self) -> usize {
        if self.has_persistent_media() {
            self.profile.write_count()
        } else {
            0
        }
    }

    pub fn persistent_write_byte_count(&self) -> u64 {
        if self.has_persistent_media() {
            self.profile.write_byte_count()
        } else {
            0
        }
    }

    fn has_persistent_media(&self) -> bool {
        self.memory_profile
            .as_ref()
            .is_some_and(|profile| profile.technology() == DramMemoryTechnology::Nvm)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DramMemoryActivityProfile {
    active_target_count: usize,
    profile: DramActivityProfile,
}

impl DramMemoryActivityProfile {
    pub fn from_target_activities<'a, I>(activities: I) -> Self
    where
        I: IntoIterator<Item = &'a DramTargetActivity>,
    {
        let mut active_target_count = 0;
        let mut profile = DramActivityProfile::default();
        for activity in activities {
            if !activity.profile().is_empty() {
                active_target_count += 1;
                profile = profile.merge_window(activity.profile());
            }
        }
        Self {
            active_target_count,
            profile,
        }
    }

    pub const fn active_target_count(&self) -> usize {
        self.active_target_count
    }

    pub const fn active_port_count(&self) -> usize {
        self.profile.active_port_count()
    }

    pub const fn active_bank_count(&self) -> usize {
        self.profile.active_bank_count()
    }

    pub const fn access_count(&self) -> usize {
        self.profile.access_count()
    }

    pub const fn read_count(&self) -> usize {
        self.profile.read_count()
    }

    pub const fn write_count(&self) -> usize {
        self.profile.write_count()
    }

    pub const fn read_byte_count(&self) -> u64 {
        self.profile.read_byte_count()
    }

    pub const fn write_byte_count(&self) -> u64 {
        self.profile.write_byte_count()
    }

    pub const fn row_hit_count(&self) -> usize {
        self.profile.row_hit_count()
    }

    pub const fn row_miss_count(&self) -> usize {
        self.profile.row_miss_count()
    }

    pub const fn command_count(&self) -> usize {
        self.profile.command_count()
    }

    pub const fn turnaround_count(&self) -> usize {
        self.profile.turnaround_count()
    }

    pub const fn total_ready_latency_cycles(&self) -> u64 {
        self.profile.total_ready_latency_cycles()
    }

    pub const fn max_ready_latency_cycles(&self) -> u64 {
        self.profile.max_ready_latency_cycles()
    }

    pub const fn has_row_misses(&self) -> bool {
        self.profile.has_row_misses()
    }

    pub const fn is_empty(&self) -> bool {
        self.profile.is_empty()
    }

    pub const fn qos_access_count(&self) -> usize {
        self.profile.qos_access_count()
    }

    pub const fn qos_byte_count(&self) -> u64 {
        self.profile.qos_byte_count()
    }

    pub const fn qos_escalated_access_count(&self) -> usize {
        self.profile.qos_escalated_access_count()
    }

    pub fn qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.profile.qos_priority_access_count(priority)
    }

    pub fn qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.profile.qos_priority_byte_count(priority)
    }

    pub fn qos_priorities(&self) -> Vec<QosPriority> {
        self.profile.qos_priorities()
    }

    pub fn qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.profile.qos_requestor_access_count(requestor)
    }

    pub fn qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.profile.qos_requestor_byte_count(requestor)
    }

    pub fn qos_requestors(&self) -> Vec<QosRequestorId> {
        self.profile.qos_requestors()
    }
}

fn merge_count_map<K, V>(target: &mut BTreeMap<K, V>, source: &BTreeMap<K, V>)
where
    K: Copy + Ord,
    V: Copy + Default + std::ops::AddAssign,
{
    for (key, value) in source {
        *target.entry(*key).or_default() += *value;
    }
}

pub(crate) fn collect_dram_bank_activity(
    accesses: &[DramAccess],
) -> BTreeMap<(u32, u32), DramBankActivity> {
    let mut activities = BTreeMap::<(u32, u32), DramBankActivity>::new();
    for access in accesses {
        activities
            .entry((access.parallel_port(), access.bank()))
            .or_default()
            .record(access);
    }
    activities
}

pub(crate) fn collect_dram_port_activity(
    accesses: &[DramAccess],
) -> BTreeMap<u32, DramPortActivity> {
    let mut activities = BTreeMap::<u32, DramPortActivity>::new();
    let mut previous_kind = BTreeMap::<u32, DramAccessKind>::new();
    for access in accesses {
        let port = access.parallel_port();
        activities
            .entry(port)
            .or_default()
            .record(access, previous_kind.get(&port).copied());
        previous_kind.insert(port, access.kind());
    }
    activities
}
