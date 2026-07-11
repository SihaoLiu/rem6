mod locked_bank;

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use rem6_checkpoint::{
    CheckpointComponentId, CheckpointError, CheckpointManifest, CheckpointRegistry,
};
use rem6_kernel::{
    PartitionId, PartitionSnapshot, PartitionedScheduler, PendingEventSnapshot, ScheduledEventKind,
    SchedulerCheckpointAccess, SchedulerError, SchedulerInstanceId, SchedulerSnapshot,
    SchedulerStorageId, Tick,
};

pub(crate) use locked_bank::SchedulerCheckpointBankGuard;

const SCHEDULER_CHUNK: &str = "scheduler";
const FORMAT_VERSION: u64 = 4;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const PARTITION_RECORD_BYTES: usize = U32_BYTES + U64_BYTES * 6;

pub(crate) fn remove_scheduler_checkpoint_chunk(
    registry: &mut CheckpointRegistry,
    component: &CheckpointComponentId,
) {
    registry.remove_chunk(component, SCHEDULER_CHUNK);
    registry.remove_component_if_empty(component);
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SchedulerCheckpointOwnedEvent {
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
    restore_policy: SchedulerCheckpointRestorePolicy,
}

impl SchedulerCheckpointOwnedEvent {
    pub(crate) const fn discard_on_restore(
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) -> Self {
        Self {
            scheduler,
            event,
            restore_policy: SchedulerCheckpointRestorePolicy::Discard,
        }
    }

    pub(crate) const fn preserve_on_restore(
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) -> Self {
        Self {
            scheduler,
            event,
            restore_policy: SchedulerCheckpointRestorePolicy::Preserve,
        }
    }

    pub(crate) fn same_identity(self, other: Self) -> bool {
        self.scheduler == other.scheduler && self.event == other.event
    }

    pub(crate) fn retain_for_scheduler(
        self,
        scheduler: SchedulerInstanceId,
        snapshot: &SchedulerSnapshot,
    ) -> bool {
        self.scheduler != scheduler || snapshot_owned_event(snapshot, self.event).is_some()
    }

    pub(crate) const fn discards_on_restore(self) -> bool {
        matches!(
            self.restore_policy,
            SchedulerCheckpointRestorePolicy::Discard
        )
    }

    fn is_pending_discard_claim(
        self,
        scheduler: SchedulerInstanceId,
        snapshot: &SchedulerSnapshot,
    ) -> bool {
        self.discards_on_restore()
            && self.scheduler == scheduler
            && snapshot_owned_event(snapshot, self.event).is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SchedulerCheckpointRestorePolicy {
    Discard,
    Preserve,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ResolvedSchedulerCheckpointEvents {
    discarded: Vec<PendingEventSnapshot>,
    preserved: Vec<PendingEventSnapshot>,
}

impl ResolvedSchedulerCheckpointEvents {
    fn push(&mut self, owned: SchedulerCheckpointOwnedEvent, event: PendingEventSnapshot) {
        match owned.restore_policy {
            SchedulerCheckpointRestorePolicy::Discard => self.discarded.push(event),
            SchedulerCheckpointRestorePolicy::Preserve => self.preserved.push(event),
        }
    }

    fn excluded(&self) -> Vec<PendingEventSnapshot> {
        self.discarded
            .iter()
            .chain(&self.preserved)
            .copied()
            .collect()
    }
}

struct SchedulerCheckpointSourceSnapshot {
    scheduler: SchedulerInstanceId,
    snapshot: SchedulerSnapshot,
}

pub(crate) struct SchedulerCheckpointContext<'a> {
    component: CheckpointComponentId,
    scheduler: SchedulerCheckpointAccess<'a>,
}

impl<'a> SchedulerCheckpointContext<'a> {
    pub(crate) fn new(
        component: CheckpointComponentId,
        scheduler: SchedulerCheckpointAccess<'a>,
    ) -> Self {
        Self {
            component,
            scheduler,
        }
    }

    pub(crate) fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub(crate) fn scheduler_instance(&self) -> SchedulerInstanceId {
        self.scheduler.instance_id()
    }

    pub(crate) fn scheduler_storage(&self) -> SchedulerStorageId {
        self.scheduler.storage_id()
    }

    pub(crate) fn scheduler_snapshot(&self) -> SchedulerSnapshot {
        self.scheduler.snapshot()
    }

    pub(crate) fn has_checkpoint_chunk(&self, registry: &CheckpointRegistry) -> bool {
        registry.chunk(&self.component, SCHEDULER_CHUNK).is_some()
    }

    pub(crate) fn manifest_has_restorable_checkpoint_state(
        &self,
        manifest: &CheckpointManifest,
    ) -> bool {
        manifest.states().iter().any(|state| {
            state.component() == &self.component
                && (state.chunks().is_empty()
                    || state
                        .chunks()
                        .iter()
                        .any(|chunk| chunk.name() == SCHEDULER_CHUNK))
        })
    }

    pub(crate) fn manifest_has_empty_checkpoint_state(
        &self,
        manifest: &CheckpointManifest,
    ) -> bool {
        manifest
            .states()
            .iter()
            .any(|state| state.component() == &self.component && state.chunks().is_empty())
    }

    pub(crate) fn remove_checkpoint_chunk(&self, registry: &mut CheckpointRegistry) {
        remove_scheduler_checkpoint_chunk(registry, &self.component);
    }

    pub(crate) fn has_pending_discard_claim(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> bool {
        let scheduler = self.scheduler.instance_id();
        let snapshot = self.scheduler.snapshot();
        owned_events
            .iter()
            .copied()
            .any(|owned| owned.is_pending_discard_claim(scheduler, &snapshot))
    }

    pub(crate) fn validate_capture(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        let snapshot = self.scheduler.snapshot();
        let resolved = resolve_owned_events_for_scheduler(
            self.scheduler.instance_id(),
            &snapshot,
            owned_events,
        );
        let excluded = resolved.excluded();
        let report = SchedulerCheckpointQuiescenceReport::from_snapshot_excluding(
            self.component.clone(),
            &snapshot,
            &excluded,
        );
        if report.is_quiescent() {
            Ok(())
        } else {
            Err(SchedulerCheckpointError::NonQuiescent { report })
        }
    }

    pub(crate) fn capture_into(
        &mut self,
        registry: &mut CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let snapshot = self.scheduler.snapshot();
        let resolved = resolve_owned_events_for_scheduler(
            self.scheduler.instance_id(),
            &snapshot,
            owned_events,
        );
        let excluded = resolved.excluded();
        let report = SchedulerCheckpointQuiescenceReport::from_snapshot_excluding(
            self.component.clone(),
            &snapshot,
            &excluded,
        );
        if !report.is_quiescent() {
            return Err(SchedulerCheckpointError::NonQuiescent { report });
        }
        match registry.register(self.component.clone()) {
            Ok(()) | Err(CheckpointError::DuplicateComponent { .. }) => {}
            Err(error) => return Err(SchedulerCheckpointError::Checkpoint(error)),
        }
        let record = SchedulerCheckpointRecord::new(
            self.component.clone(),
            project_quiescent_snapshot(&snapshot),
        );
        registry
            .write_chunk(
                &self.component,
                SCHEDULER_CHUNK,
                encode_snapshot(record.snapshot()),
            )
            .map_err(SchedulerCheckpointError::Checkpoint)?;
        Ok(record)
    }

    pub(crate) fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        let snapshot = decode_registered_snapshot(&self.component, registry)?;
        let current = self.scheduler.snapshot();
        let resolved = resolve_owned_events_for_scheduler(
            self.scheduler.instance_id(),
            &current,
            owned_events,
        );
        validate_scheduler_projection_restore(&self.scheduler, &snapshot, &resolved).map_err(
            |error| SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            },
        )
    }

    pub(crate) fn restore_from(
        &mut self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let snapshot = decode_registered_snapshot(&self.component, registry)?;
        let current = self.scheduler.snapshot();
        let resolved = resolve_owned_events_for_scheduler(
            self.scheduler.instance_id(),
            &current,
            owned_events,
        );
        restore_scheduler_projection(&mut self.scheduler, &snapshot, &resolved).map_err(
            |error| SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            },
        )?;
        Ok(SchedulerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub(crate) fn validate_discard_pending_owned_events(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        self.pending_discard_events(owned_events).map(|_| ())
    }

    pub(crate) fn discard_pending_owned_events(
        &mut self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        let discarded = self.pending_discard_events(owned_events)?;
        self.scheduler
            .discard_exact_events(&discarded)
            .map_err(|error| SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            })
    }

    fn pending_discard_events(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<Vec<PendingEventSnapshot>, SchedulerCheckpointError> {
        let scheduler = self.scheduler.instance_id();
        let snapshot = self.scheduler.snapshot();
        let ambiguous = owned_events
            .iter()
            .copied()
            .filter(|owned| owned.is_pending_discard_claim(scheduler, &snapshot))
            .filter(|owned| {
                owned_events
                    .iter()
                    .filter(|candidate| candidate.same_identity(*owned))
                    .count()
                    != 1
            })
            .map(|owned| owned.event)
            .fold(Vec::new(), |mut events, event| {
                if !events.contains(&event) {
                    events.push(event);
                }
                events
            });
        if !ambiguous.is_empty() {
            let excluded = snapshot
                .partitions()
                .iter()
                .flat_map(PartitionSnapshot::pending_events)
                .copied()
                .filter(|event| !ambiguous.contains(event))
                .collect::<Vec<_>>();
            let report = SchedulerCheckpointQuiescenceReport::from_snapshot_excluding(
                self.component.clone(),
                &snapshot,
                &excluded,
            );
            return Err(SchedulerCheckpointError::NonQuiescent { report });
        }
        Ok(resolve_owned_events_for_scheduler(scheduler, &snapshot, owned_events).discarded)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: SchedulerSnapshot,
}

impl SchedulerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: SchedulerSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &SchedulerSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerCheckpointPendingPartition {
    partition: PartitionId,
    pending_events: usize,
    first_pending_tick: Option<Tick>,
    last_pending_tick: Option<Tick>,
    serial_pending_events: usize,
    parallel_pending_events: usize,
}

impl SchedulerCheckpointPendingPartition {
    fn from_snapshot_excluding(
        snapshot: &PartitionSnapshot,
        excluded_events: &[PendingEventSnapshot],
    ) -> Option<Self> {
        let pending_events = snapshot
            .pending_events()
            .iter()
            .copied()
            .filter(|event| !excluded_events.contains(event))
            .collect::<Vec<_>>();
        if pending_events.is_empty() {
            return None;
        }

        let mut first_pending_tick = None;
        let mut last_pending_tick = None;
        let mut serial_pending_events = 0;
        let mut parallel_pending_events = 0;
        for event in &pending_events {
            first_pending_tick =
                Some(first_pending_tick.map_or(event.tick(), |tick: Tick| tick.min(event.tick())));
            last_pending_tick =
                Some(last_pending_tick.map_or(event.tick(), |tick: Tick| tick.max(event.tick())));
            match event.kind() {
                ScheduledEventKind::Serial => serial_pending_events += 1,
                ScheduledEventKind::Parallel => parallel_pending_events += 1,
            }
        }

        Some(Self {
            partition: snapshot.partition(),
            pending_events: pending_events.len(),
            first_pending_tick,
            last_pending_tick,
            serial_pending_events,
            parallel_pending_events,
        })
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn pending_event_count(&self) -> usize {
        self.pending_events
    }

    pub fn first_pending_tick(&self) -> Option<Tick> {
        self.first_pending_tick
    }

    pub fn last_pending_tick(&self) -> Option<Tick> {
        self.last_pending_tick
    }

    pub fn event_count_by_kind(&self, kind: ScheduledEventKind) -> usize {
        match kind {
            ScheduledEventKind::Serial => self.serial_pending_events,
            ScheduledEventKind::Parallel => self.parallel_pending_events,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerCheckpointQuiescenceReport {
    component: CheckpointComponentId,
    partition_count: u32,
    pending_partitions: Vec<SchedulerCheckpointPendingPartition>,
}

impl SchedulerCheckpointQuiescenceReport {
    fn from_snapshot(component: CheckpointComponentId, snapshot: &SchedulerSnapshot) -> Self {
        Self::from_snapshot_excluding(component, snapshot, &[])
    }

    fn from_snapshot_excluding(
        component: CheckpointComponentId,
        snapshot: &SchedulerSnapshot,
        excluded_events: &[PendingEventSnapshot],
    ) -> Self {
        let pending_partitions = snapshot
            .partitions()
            .iter()
            .filter_map(|partition| {
                SchedulerCheckpointPendingPartition::from_snapshot_excluding(
                    partition,
                    excluded_events,
                )
            })
            .collect();
        Self {
            component,
            partition_count: snapshot.partitions().len() as u32,
            pending_partitions,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn is_quiescent(&self) -> bool {
        self.pending_partitions.is_empty()
    }

    pub fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub fn pending_event_count(&self) -> usize {
        self.pending_partitions
            .iter()
            .map(SchedulerCheckpointPendingPartition::pending_event_count)
            .sum()
    }

    pub fn pending_partition_count(&self) -> usize {
        self.pending_partitions.len()
    }

    pub fn first_pending_tick(&self) -> Option<Tick> {
        self.pending_partitions
            .iter()
            .filter_map(SchedulerCheckpointPendingPartition::first_pending_tick)
            .min()
    }

    pub fn last_pending_tick(&self) -> Option<Tick> {
        self.pending_partitions
            .iter()
            .filter_map(SchedulerCheckpointPendingPartition::last_pending_tick)
            .max()
    }

    pub fn serial_pending_event_count(&self) -> usize {
        self.event_count_by_kind(ScheduledEventKind::Serial)
    }

    pub fn parallel_pending_event_count(&self) -> usize {
        self.event_count_by_kind(ScheduledEventKind::Parallel)
    }

    pub fn event_count_by_kind(&self, kind: ScheduledEventKind) -> usize {
        self.pending_partitions
            .iter()
            .map(|partition| partition.event_count_by_kind(kind))
            .sum()
    }

    pub fn pending_partitions(&self) -> &[SchedulerCheckpointPendingPartition] {
        &self.pending_partitions
    }

    pub fn pending_partition(
        &self,
        partition: PartitionId,
    ) -> Option<&SchedulerCheckpointPendingPartition> {
        self.pending_partitions
            .iter()
            .find(|report| report.partition() == partition)
    }
}

#[derive(Clone, Debug)]
pub struct SchedulerCheckpointPort {
    component: CheckpointComponentId,
    scheduler_instance: SchedulerInstanceId,
    scheduler_storage: SchedulerStorageId,
    scheduler: Arc<Mutex<PartitionedScheduler>>,
}

impl SchedulerCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        scheduler: Arc<Mutex<PartitionedScheduler>>,
    ) -> Self {
        let (scheduler_instance, scheduler_storage) = {
            let mut scheduler = scheduler.lock().expect("scheduler checkpoint lock");
            let checkpoint = scheduler.checkpoint_access();
            (checkpoint.instance_id(), checkpoint.storage_id())
        };
        Self {
            component,
            scheduler_instance,
            scheduler_storage,
            scheduler,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn scheduler(&self) -> Arc<Mutex<PartitionedScheduler>> {
        Arc::clone(&self.scheduler)
    }

    pub const fn scheduler_instance(&self) -> SchedulerInstanceId {
        self.scheduler_instance
    }

    pub const fn scheduler_storage(&self) -> SchedulerStorageId {
        self.scheduler_storage
    }

    pub fn quiescence_report(&self) -> SchedulerCheckpointQuiescenceReport {
        let snapshot = self
            .source_snapshot()
            .expect("scheduler checkpoint binding")
            .snapshot;
        SchedulerCheckpointQuiescenceReport::from_snapshot(self.component.clone(), &snapshot)
    }

    pub fn try_quiescence_report(
        &self,
    ) -> Result<SchedulerCheckpointQuiescenceReport, SchedulerCheckpointError> {
        let snapshot = self.try_source_snapshot()?.snapshot;
        Ok(SchedulerCheckpointQuiescenceReport::from_snapshot(
            self.component.clone(),
            &snapshot,
        ))
    }

    fn source_snapshot(
        &self,
    ) -> Result<SchedulerCheckpointSourceSnapshot, SchedulerCheckpointError> {
        let scheduler = self.lock_scheduler()?;
        Ok(SchedulerCheckpointSourceSnapshot {
            scheduler: self.scheduler_instance,
            snapshot: scheduler.snapshot(),
        })
    }

    fn try_source_snapshot(
        &self,
    ) -> Result<SchedulerCheckpointSourceSnapshot, SchedulerCheckpointError> {
        let scheduler = self.try_scheduler()?;
        Ok(SchedulerCheckpointSourceSnapshot {
            scheduler: self.scheduler_instance,
            snapshot: scheduler.snapshot(),
        })
    }

    fn try_scheduler(
        &self,
    ) -> Result<MutexGuard<'_, PartitionedScheduler>, SchedulerCheckpointError> {
        let scheduler = match self.scheduler.try_lock() {
            Ok(scheduler) => Ok(scheduler),
            Err(TryLockError::WouldBlock) => Err(SchedulerCheckpointError::SchedulerBusy {
                component: self.component.clone(),
            }),
            Err(TryLockError::Poisoned(error)) => {
                panic!("scheduler checkpoint lock: {error}")
            }
        }?;
        self.validate_scheduler_instance(&scheduler)?;
        Ok(scheduler)
    }

    fn lock_scheduler(
        &self,
    ) -> Result<MutexGuard<'_, PartitionedScheduler>, SchedulerCheckpointError> {
        let scheduler = self.scheduler.lock().expect("scheduler checkpoint lock");
        self.validate_scheduler_instance(&scheduler)?;
        Ok(scheduler)
    }

    fn validate_scheduler_instance(
        &self,
        scheduler: &PartitionedScheduler,
    ) -> Result<(), SchedulerCheckpointError> {
        let live_scheduler = scheduler.instance_id();
        if live_scheduler == self.scheduler_instance {
            return Ok(());
        }
        Err(SchedulerCheckpointError::AttachedSchedulerBindingMismatch {
            component: self.component.clone(),
            bound_scheduler: self.scheduler_instance,
            live_scheduler,
        })
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let record = self.capture_record_excluding(&[])?;
        self.write_record(registry, &record)?;
        Ok(record)
    }

    fn capture_record_excluding(
        &self,
        excluded_events: &[PendingEventSnapshot],
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let snapshot = self.source_snapshot()?.snapshot;
        self.capture_record_from_snapshot(&snapshot, excluded_events)
    }

    fn capture_record_from_snapshot(
        &self,
        snapshot: &SchedulerSnapshot,
        excluded_events: &[PendingEventSnapshot],
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let report = SchedulerCheckpointQuiescenceReport::from_snapshot_excluding(
            self.component.clone(),
            snapshot,
            excluded_events,
        );
        if !report.is_quiescent() {
            return Err(SchedulerCheckpointError::NonQuiescent { report });
        }

        let snapshot = project_quiescent_snapshot(snapshot);
        Ok(SchedulerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn write_record(
        &self,
        registry: &mut CheckpointRegistry,
        record: &SchedulerCheckpointRecord,
    ) -> Result<(), SchedulerCheckpointError> {
        registry
            .write_chunk(
                &self.component,
                SCHEDULER_CHUNK,
                encode_snapshot(record.snapshot()),
            )
            .map_err(SchedulerCheckpointError::Checkpoint)?;
        Ok(())
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SchedulerCheckpointRecord, SchedulerCheckpointError> {
        let snapshot = decode_registered_snapshot(&self.component, registry)?;
        Ok(SchedulerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn restore_snapshot(
        &self,
        snapshot: &SchedulerSnapshot,
    ) -> Result<(), SchedulerCheckpointError> {
        self.restore_snapshot_with_events(snapshot, &ResolvedSchedulerCheckpointEvents::default())
    }

    fn restore_snapshot_with_events(
        &self,
        snapshot: &SchedulerSnapshot,
        owned_events: &ResolvedSchedulerCheckpointEvents,
    ) -> Result<(), SchedulerCheckpointError> {
        let mut scheduler = self.lock_scheduler()?;
        let mut scheduler = scheduler.checkpoint_access();
        restore_scheduler_projection(&mut scheduler, snapshot, owned_events).map_err(|error| {
            SchedulerCheckpointError::Scheduler {
                component: self.component.clone(),
                error,
            }
        })
    }
}

fn decode_registered_snapshot(
    component: &CheckpointComponentId,
    registry: &CheckpointRegistry,
) -> Result<SchedulerSnapshot, SchedulerCheckpointError> {
    let payload = registry.chunk(component, SCHEDULER_CHUNK).ok_or_else(|| {
        SchedulerCheckpointError::MissingChunk {
            component: component.clone(),
            name: SCHEDULER_CHUNK.to_string(),
        }
    })?;
    decode_snapshot(component, payload)
}

fn project_quiescent_snapshot(snapshot: &SchedulerSnapshot) -> SchedulerSnapshot {
    SchedulerSnapshot::with_parallel_worker_limit(
        snapshot.now(),
        snapshot.min_remote_delay(),
        snapshot.max_parallel_workers(),
        snapshot
            .partitions()
            .iter()
            .map(|partition| {
                PartitionSnapshot::quiescent_with_orders(
                    partition.partition(),
                    snapshot.now(),
                    partition.next_event_local(),
                    partition.next_event_order(),
                    partition.next_remote_order(),
                    partition.next_progress_order(),
                )
            })
            .collect(),
    )
}

fn validate_scheduler_projection_restore(
    scheduler: &SchedulerCheckpointAccess<'_>,
    snapshot: &SchedulerSnapshot,
    events: &ResolvedSchedulerCheckpointEvents,
) -> Result<(), SchedulerError> {
    scheduler.validate_projection_restore(snapshot, &events.discarded, &events.preserved)
}

fn restore_scheduler_projection(
    scheduler: &mut SchedulerCheckpointAccess<'_>,
    snapshot: &SchedulerSnapshot,
    owned_events: &ResolvedSchedulerCheckpointEvents,
) -> Result<(), SchedulerError> {
    scheduler.restore_projection(snapshot, &owned_events.discarded, &owned_events.preserved)
}

#[derive(Clone, Debug, Default)]
pub struct SchedulerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, SchedulerCheckpointPort>,
}

impl SchedulerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = SchedulerCheckpointPort>,
    {
        let mut by_component = BTreeMap::<CheckpointComponentId, SchedulerCheckpointPort>::new();
        let mut by_scheduler = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            if by_component
                .values()
                .any(|existing| Arc::ptr_eq(&existing.scheduler, &port.scheduler))
            {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            if by_scheduler
                .insert(port.scheduler_instance(), component.clone())
                .is_some()
            {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub(crate) fn has_component(&self, component: &CheckpointComponentId) -> bool {
        self.ports.contains_key(component)
    }

    pub(crate) fn validate_borrowed_scheduler(
        &self,
        scheduler: &SchedulerCheckpointContext<'_>,
    ) -> Result<bool, SchedulerCheckpointError> {
        let borrowed_component = scheduler.component();
        let borrowed_scheduler = scheduler.scheduler_instance();
        let borrowed_storage = scheduler.scheduler_storage();
        if let Some(port) = self.ports.get(borrowed_component) {
            if port.scheduler_instance() != borrowed_scheduler {
                return Err(SchedulerCheckpointError::BorrowedSchedulerBindingMismatch {
                    borrowed_component: borrowed_component.clone(),
                    borrowed_scheduler,
                    attached_component: borrowed_component.clone(),
                    attached_scheduler: port.scheduler_instance(),
                });
            }
            if port.scheduler_storage() != borrowed_storage {
                return Err(SchedulerCheckpointError::BorrowedSchedulerStorageMismatch {
                    borrowed_component: borrowed_component.clone(),
                    borrowed_scheduler,
                    attached_component: borrowed_component.clone(),
                    attached_scheduler: port.scheduler_instance(),
                });
            }
            return Ok(true);
        }
        if let Some((attached_component, port)) = self
            .ports
            .iter()
            .find(|(_, port)| port.scheduler_storage() == borrowed_storage)
        {
            return Err(SchedulerCheckpointError::BorrowedSchedulerBindingMismatch {
                borrowed_component: borrowed_component.clone(),
                borrowed_scheduler,
                attached_component: attached_component.clone(),
                attached_scheduler: port.scheduler_instance(),
            });
        }
        if let Some((attached_component, port)) = self
            .ports
            .iter()
            .find(|(_, port)| port.scheduler_instance() == borrowed_scheduler)
        {
            return Err(SchedulerCheckpointError::BorrowedSchedulerStorageMismatch {
                borrowed_component: borrowed_component.clone(),
                borrowed_scheduler,
                attached_component: attached_component.clone(),
                attached_scheduler: port.scheduler_instance(),
            });
        }
        Ok(false)
    }

    pub fn quiescence_reports(&self) -> Vec<SchedulerCheckpointQuiescenceReport> {
        self.ports
            .values()
            .map(SchedulerCheckpointPort::quiescence_report)
            .collect()
    }

    pub fn try_quiescence_reports(
        &self,
    ) -> Result<Vec<SchedulerCheckpointQuiescenceReport>, SchedulerCheckpointError> {
        self.ports
            .values()
            .map(SchedulerCheckpointPort::try_quiescence_report)
            .collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.capture_all_into_with_owned_events(registry, &[])
    }

    pub(crate) fn capture_all_into_with_owned_events(
        &self,
        registry: &mut CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.capture_all_into_with_owned_events_except(registry, owned_events, None)
    }

    pub(crate) fn capture_all_into_with_owned_events_except(
        &self,
        registry: &mut CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        let guard = self.lock_except(excluded_component)?;
        guard.capture_all_into_with_owned_events(registry, owned_events)
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.restore_all_from_with_owned_events(registry, &[])
    }

    pub(crate) fn restore_all_from_with_owned_events(
        &self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.restore_all_from_with_owned_events_except(registry, owned_events, None)
    }

    pub(crate) fn restore_all_from_with_owned_events_except(
        &self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        let mut guard = self.lock_except(excluded_component)?;
        guard.restore_all_from_with_owned_events(registry, owned_events)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), SchedulerCheckpointError> {
        self.validate_restore_from_with_owned_events(registry, &[])
    }

    pub(crate) fn validate_restore_from_with_owned_events(
        &self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        self.validate_restore_from_with_owned_events_except(registry, owned_events, None)
    }

    pub(crate) fn validate_restore_from_with_owned_events_except(
        &self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<(), SchedulerCheckpointError> {
        let mut guard = self.lock_except(excluded_component)?;
        guard.validate_restore_from_with_owned_events(registry, owned_events)
    }

    pub fn validate_quiescent_capture(&self) -> Result<(), SchedulerCheckpointError> {
        self.validate_quiescent_capture_with_owned_events(&[])
    }

    pub(crate) fn validate_quiescent_capture_with_owned_events(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        self.validate_quiescent_capture_with_owned_events_except(owned_events, None)
    }

    pub(crate) fn validate_quiescent_capture_with_owned_events_except(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<(), SchedulerCheckpointError> {
        let guard = self.lock_except(excluded_component)?;
        guard.validate_quiescent_capture_with_owned_events(owned_events)
    }
}

fn resolve_owned_events(
    snapshots: &BTreeMap<CheckpointComponentId, SchedulerCheckpointSourceSnapshot>,
    owned_events: &[SchedulerCheckpointOwnedEvent],
) -> BTreeMap<CheckpointComponentId, ResolvedSchedulerCheckpointEvents> {
    let mut resolved = BTreeMap::<CheckpointComponentId, ResolvedSchedulerCheckpointEvents>::new();
    for owned in owned_events.iter().copied() {
        if owned_events
            .iter()
            .filter(|candidate| candidate.same_identity(owned))
            .count()
            != 1
        {
            continue;
        }
        let matching_components = snapshots
            .iter()
            .filter_map(|(component, source)| {
                (source.scheduler == owned.scheduler)
                    .then(|| snapshot_owned_event(&source.snapshot, owned.event))
                    .flatten()
                    .map(|event| (component, event))
            })
            .collect::<Vec<_>>();
        let [(component, event)] = matching_components.as_slice() else {
            continue;
        };
        resolved
            .entry((*component).clone())
            .or_default()
            .push(owned, *event);
    }
    resolved
}

fn resolve_owned_events_for_scheduler(
    scheduler: SchedulerInstanceId,
    snapshot: &SchedulerSnapshot,
    owned_events: &[SchedulerCheckpointOwnedEvent],
) -> ResolvedSchedulerCheckpointEvents {
    let mut resolved = ResolvedSchedulerCheckpointEvents::default();
    for owned in owned_events.iter().copied().filter(|owned| {
        owned.scheduler == scheduler
            && owned_events
                .iter()
                .filter(|candidate| candidate.same_identity(*owned))
                .count()
                == 1
    }) {
        if let Some(event) = snapshot_owned_event(snapshot, owned.event) {
            resolved.push(owned, event);
        }
    }
    resolved
}

fn snapshot_owned_event(
    snapshot: &SchedulerSnapshot,
    owned: PendingEventSnapshot,
) -> Option<PendingEventSnapshot> {
    snapshot
        .partitions()
        .iter()
        .flat_map(PartitionSnapshot::pending_events)
        .copied()
        .find(|event| *event == owned)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerCheckpointError {
    BorrowedSchedulerContextRequired,
    SchedulerBusy {
        component: CheckpointComponentId,
    },
    AttachedSchedulerBindingMismatch {
        component: CheckpointComponentId,
        bound_scheduler: SchedulerInstanceId,
        live_scheduler: SchedulerInstanceId,
    },
    BorrowedSchedulerStorageMismatch {
        borrowed_component: CheckpointComponentId,
        borrowed_scheduler: SchedulerInstanceId,
        attached_component: CheckpointComponentId,
        attached_scheduler: SchedulerInstanceId,
    },
    NonQuiescent {
        report: SchedulerCheckpointQuiescenceReport,
    },
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    BorrowedSchedulerBindingMismatch {
        borrowed_component: CheckpointComponentId,
        borrowed_scheduler: SchedulerInstanceId,
        attached_component: CheckpointComponentId,
        attached_scheduler: SchedulerInstanceId,
    },
    Checkpoint(CheckpointError),
    Scheduler {
        component: CheckpointComponentId,
        error: SchedulerError,
    },
}

impl SchedulerCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::BorrowedSchedulerContextRequired => None,
            Self::SchedulerBusy { component }
            | Self::AttachedSchedulerBindingMismatch { component, .. } => Some(component),
            Self::NonQuiescent { report } => Some(report.component()),
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Scheduler { component, .. } => Some(component),
            Self::BorrowedSchedulerBindingMismatch {
                borrowed_component, ..
            }
            | Self::BorrowedSchedulerStorageMismatch {
                borrowed_component, ..
            } => Some(borrowed_component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for SchedulerCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BorrowedSchedulerContextRequired => write!(
                formatter,
                "scheduler checkpoint host event requires a borrowed scheduler context"
            ),
            Self::SchedulerBusy { component } => write!(
                formatter,
                "scheduler checkpoint component {} is busy",
                component.as_str()
            ),
            Self::AttachedSchedulerBindingMismatch {
                component,
                bound_scheduler,
                live_scheduler,
            } => write!(
                formatter,
                "scheduler checkpoint component {} is bound to scheduler {bound_scheduler:?}, but contains scheduler {live_scheduler:?}",
                component.as_str()
            ),
            Self::NonQuiescent { report } => write!(
                formatter,
                "scheduler checkpoint component {} is not quiescent: {} pending events across {} partitions, first tick {:?}, last tick {:?}",
                report.component().as_str(),
                report.pending_event_count(),
                report.pending_partition_count(),
                report.first_pending_tick(),
                report.last_pending_tick()
            ),
            Self::MissingChunk { component, name } => write!(
                formatter,
                "scheduler checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "scheduler checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::BorrowedSchedulerBindingMismatch {
                borrowed_component,
                borrowed_scheduler,
                attached_component,
                attached_scheduler,
            } => write!(
                formatter,
                "borrowed scheduler checkpoint component {} for scheduler {borrowed_scheduler:?} conflicts with attached component {} for scheduler {attached_scheduler:?}",
                borrowed_component.as_str(),
                attached_component.as_str()
            ),
            Self::BorrowedSchedulerStorageMismatch {
                borrowed_component,
                borrowed_scheduler,
                attached_component,
                attached_scheduler,
            } => write!(
                formatter,
                "borrowed scheduler checkpoint component {} for scheduler {borrowed_scheduler:?} does not use the attached storage for component {} bound to scheduler {attached_scheduler:?}",
                borrowed_component.as_str(),
                attached_component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Scheduler { component, error } => write!(
                formatter,
                "scheduler checkpoint component {} cannot capture or restore scheduler: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for SchedulerCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Scheduler { error, .. } => Some(error),
            Self::BorrowedSchedulerContextRequired
            | Self::SchedulerBusy { .. }
            | Self::AttachedSchedulerBindingMismatch { .. }
            | Self::NonQuiescent { .. }
            | Self::MissingChunk { .. }
            | Self::InvalidChunk { .. }
            | Self::BorrowedSchedulerBindingMismatch { .. }
            | Self::BorrowedSchedulerStorageMismatch { .. } => None,
        }
    }
}

fn encode_snapshot(snapshot: &SchedulerSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, FORMAT_VERSION);
    write_u64(&mut payload, snapshot.now());
    write_u64(&mut payload, snapshot.min_remote_delay());
    write_u64(&mut payload, snapshot.max_parallel_workers() as u64);
    write_u64(&mut payload, snapshot.partitions().len() as u64);
    for partition in snapshot.partitions() {
        write_u32(&mut payload, partition.partition().index());
        write_u64(&mut payload, partition.now());
        write_u64(&mut payload, partition.next_event_local());
        write_u64(&mut payload, partition.next_event_order());
        write_u64(&mut payload, partition.next_remote_order());
        write_u64(&mut payload, partition.next_progress_order());
        write_u64(&mut payload, partition.pending_events().len() as u64);
    }
    payload
}

fn decode_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<SchedulerSnapshot, SchedulerCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("scheduler checkpoint version")?;
    if version != FORMAT_VERSION {
        return Err(cursor.invalid(format!(
            "scheduler checkpoint version {version} is unsupported"
        )));
    }

    let now = cursor.read_u64("scheduler now")?;
    let min_remote_delay = cursor.read_u64("scheduler lookahead")?;
    let max_parallel_workers = cursor.read_count("scheduler parallel worker limit")?;
    let partition_count =
        cursor.read_bounded_count("scheduler partition count", PARTITION_RECORD_BYTES)?;
    let mut partitions = Vec::with_capacity(partition_count);
    for _ in 0..partition_count {
        let partition = PartitionId::new(cursor.read_u32("scheduler partition")?);
        let partition_now = cursor.read_u64("scheduler partition now")?;
        let next_event_local = cursor.read_u64("scheduler next event local")?;
        let next_event_order = cursor.read_u64("scheduler next event order")?;
        let next_remote_order = cursor.read_u64("scheduler next remote order")?;
        let next_progress_order = cursor.read_u64("scheduler next progress order")?;
        let pending_events = cursor.read_count("scheduler pending event count")?;
        if pending_events != 0 {
            return Err(cursor.invalid(format!(
                "quiescent scheduler checkpoint contains {pending_events} pending events"
            )));
        }
        partitions.push(PartitionSnapshot::quiescent_with_orders(
            partition,
            partition_now,
            next_event_local,
            next_event_order,
            next_remote_order,
            next_progress_order,
        ));
    }
    cursor.finish()?;
    Ok(SchedulerSnapshot::with_parallel_worker_limit(
        now,
        min_remote_delay,
        max_parallel_workers,
        partitions,
    ))
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    position: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            position: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, SchedulerCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))
    }

    fn read_bounded_count(
        &mut self,
        name: &str,
        record_bytes: usize,
    ) -> Result<usize, SchedulerCheckpointError> {
        let count = self.read_count(name)?;
        let capacity = self.remaining() / record_bytes;
        if count > capacity {
            return Err(self.invalid(format!(
                "{name} {count} exceeds remaining payload capacity {capacity} records"
            )));
        }
        Ok(count)
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.position)
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, SchedulerCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes.try_into().expect("u32 byte count checked"),
        ))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, SchedulerCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes.try_into().expect("u64 byte count checked"),
        ))
    }

    fn read_bytes(
        &mut self,
        name: &str,
        count: usize,
    ) -> Result<&'a [u8], SchedulerCheckpointError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} offset overflow")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} truncated at byte {} while reading {count} bytes",
                self.position
            )));
        }
        let bytes = &self.payload[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), SchedulerCheckpointError> {
        if self.position != self.payload.len() {
            return Err(self.invalid(format!(
                "scheduler checkpoint has {} trailing bytes",
                self.payload.len() - self.position
            )));
        }
        Ok(())
    }

    fn invalid(&self, reason: String) -> SchedulerCheckpointError {
        SchedulerCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_event(
        scheduler: &Arc<Mutex<PartitionedScheduler>>,
        id: rem6_kernel::PartitionEventId,
    ) -> SchedulerCheckpointOwnedEvent {
        let scheduler = scheduler.lock().unwrap();
        SchedulerCheckpointOwnedEvent::discard_on_restore(
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(id).unwrap(),
        )
    }

    #[test]
    fn owned_event_capture_projects_quiescent_snapshot_without_mutating_live_queue() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(
            PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
        ));
        let id = scheduler
            .lock()
            .unwrap()
            .schedule_parallel_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let owned = owned_event(&scheduler, id);
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();

        let records = bank
            .capture_all_into_with_owned_events(&mut registry, &[owned])
            .unwrap();

        assert_eq!(records.len(), 1);
        assert!(records[0].snapshot().is_quiescent());
        let live_snapshot = scheduler.lock().unwrap().snapshot();
        assert_eq!(live_snapshot.total_pending_events(), 1);
        assert_eq!(
            records[0].snapshot().partitions()[0].next_event_local(),
            live_snapshot.partitions()[0].next_event_local()
        );
        assert_eq!(
            records[0].snapshot().partitions()[0].next_event_order(),
            live_snapshot.partitions()[0].next_event_order()
        );
        assert!(registry.chunk(&component, SCHEDULER_CHUNK).is_some());
    }

    #[test]
    fn owned_event_capture_still_reports_unrelated_pending_work() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(
            PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap(),
        ));
        let owned_id = scheduler
            .lock()
            .unwrap()
            .schedule_parallel_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(1), 13, |_| {})
            .unwrap();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let owned = owned_event(&scheduler, owned_id);
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();

        let error = bank
            .capture_all_into_with_owned_events(&mut registry, &[owned])
            .unwrap_err();

        let SchedulerCheckpointError::NonQuiescent { report } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(report.pending_event_count(), 1);
        assert_eq!(report.first_pending_tick(), Some(13));
        assert_eq!(report.serial_pending_event_count(), 1);
        assert_eq!(report.parallel_pending_event_count(), 0);
        assert_eq!(registry.chunk(&component, SCHEDULER_CHUNK), None);
    }

    #[test]
    fn owned_event_is_bound_to_its_scheduler_instance() {
        let component0 = CheckpointComponentId::new("scheduler0").unwrap();
        let component1 = CheckpointComponentId::new("scheduler1").unwrap();
        let scheduler0 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let scheduler1 = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let id0 = scheduler0
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let id1 = scheduler1
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        assert_eq!(id0, id1);
        let bank = SchedulerCheckpointBank::new([
            SchedulerCheckpointPort::new(component0.clone(), Arc::clone(&scheduler0)),
            SchedulerCheckpointPort::new(component1.clone(), Arc::clone(&scheduler1)),
        ])
        .unwrap();
        let owned = owned_event(&scheduler0, id0);
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();

        let error = bank
            .capture_all_into_with_owned_events(&mut registry, &[owned])
            .unwrap_err();

        let SchedulerCheckpointError::NonQuiescent { report } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(report.component(), &component1);
        assert_eq!(report.pending_event_count(), 1);
        assert_eq!(registry.chunk(&component0, SCHEDULER_CHUNK), None);
        assert_eq!(registry.chunk(&component1, SCHEDULER_CHUNK), None);
    }

    #[test]
    fn unattached_scheduler_wake_cannot_exclude_colliding_attached_event() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let unattached = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let attached = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let unattached_id = unattached
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let attached_id = attached
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        assert_eq!(unattached_id, attached_id);
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&attached),
        )])
        .unwrap();
        let owned = owned_event(&unattached, unattached_id);
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();

        let error = bank
            .capture_all_into_with_owned_events(&mut registry, &[owned])
            .unwrap_err();

        let SchedulerCheckpointError::NonQuiescent { report } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(report.component(), &component);
        assert_eq!(report.pending_event_count(), 1);
        assert_eq!(registry.chunk(&component, SCHEDULER_CHUNK), None);
    }

    #[test]
    fn duplicate_owned_event_claim_does_not_bypass_quiescence() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component.clone(),
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let owned = owned_event(&scheduler, id);
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();

        let error = bank
            .capture_all_into_with_owned_events(&mut registry, &[owned, owned])
            .unwrap_err();

        let SchedulerCheckpointError::NonQuiescent { report } = error else {
            panic!("unexpected error: {error:?}");
        };
        assert_eq!(report.component(), &component);
        assert_eq!(report.pending_event_count(), 1);
        assert_eq!(registry.chunk(&component, SCHEDULER_CHUNK), None);
    }

    #[test]
    fn scheduler_checkpoint_bank_rejects_component_aliases() {
        let component0 = CheckpointComponentId::new("scheduler0").unwrap();
        let component1 = CheckpointComponentId::new("scheduler1").unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));

        let result = SchedulerCheckpointBank::new([
            SchedulerCheckpointPort::new(component0, Arc::clone(&scheduler)),
            SchedulerCheckpointPort::new(component1.clone(), scheduler),
        ]);

        assert_eq!(
            result.unwrap_err(),
            CheckpointError::DuplicateComponent {
                component: component1,
            }
        );
    }

    #[test]
    fn restore_drops_only_the_exact_owned_live_event() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();
        bank.capture_all_into(&mut registry).unwrap();
        let id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let owned = owned_event(&scheduler, id);

        bank.restore_all_from_with_owned_events(&registry, &[owned])
            .unwrap();

        assert!(scheduler.lock().unwrap().is_idle());
    }

    #[test]
    fn restore_preserves_checkpoint_external_control_events() {
        let component = CheckpointComponentId::new("scheduler0").unwrap();
        let scheduler = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let bank = SchedulerCheckpointBank::new([SchedulerCheckpointPort::new(
            component,
            Arc::clone(&scheduler),
        )])
        .unwrap();
        let mut registry = CheckpointRegistry::new();
        bank.register_all(&mut registry).unwrap();
        bank.capture_all_into(&mut registry).unwrap();
        let discarded_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 7, |_| {})
            .unwrap();
        let preserved_id = scheduler
            .lock()
            .unwrap()
            .schedule_at(PartitionId::new(0), 11, |_| {})
            .unwrap();
        let scheduler_snapshot = scheduler.lock().unwrap();
        let scheduler_instance = scheduler_snapshot.instance_id();
        let discarded = SchedulerCheckpointOwnedEvent::discard_on_restore(
            scheduler_instance,
            scheduler_snapshot
                .pending_event_snapshot(discarded_id)
                .unwrap(),
        );
        let preserved = SchedulerCheckpointOwnedEvent::preserve_on_restore(
            scheduler_instance,
            scheduler_snapshot
                .pending_event_snapshot(preserved_id)
                .unwrap(),
        );
        drop(scheduler_snapshot);

        bank.restore_all_from_with_owned_events(&registry, &[discarded, preserved])
            .unwrap();

        let scheduler = scheduler.lock().unwrap();
        assert_eq!(scheduler.snapshot().total_pending_events(), 1);
        assert!(scheduler.pending_event_snapshot(discarded_id).is_none());
        assert_eq!(
            scheduler.pending_event_snapshot(preserved_id),
            Some(preserved.event)
        );
    }
}
