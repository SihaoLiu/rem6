use std::collections::BTreeMap;
use std::sync::MutexGuard;

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::PartitionedScheduler;

use crate::riscv_checkpoint::RiscvO3LiveSchedulerRestore;

use super::{
    decode_registered_snapshot, rebind_live_o3_for_scheduler, resolve_owned_events,
    restore_scheduler_projection, snapshot_owned_event, validate_live_o3_for_scheduler,
    validate_scheduler_projection_restore, LiveO3SchedulerValidationMode,
    ResolvedSchedulerCheckpointEvents, SchedulerCheckpointBank, SchedulerCheckpointError,
    SchedulerCheckpointOwnedEvent, SchedulerCheckpointPort, SchedulerCheckpointQuiescenceReport,
    SchedulerCheckpointRecord, SchedulerCheckpointSourceSnapshot,
};

struct LockedSchedulerCheckpointPort<'a> {
    port: &'a SchedulerCheckpointPort,
    scheduler: MutexGuard<'a, PartitionedScheduler>,
}

pub(crate) struct SchedulerCheckpointBankGuard<'a> {
    ports: Vec<LockedSchedulerCheckpointPort<'a>>,
}

impl SchedulerCheckpointBank {
    fn ports_in_lock_order(
        &self,
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Vec<&SchedulerCheckpointPort> {
        let mut ports = self
            .ports
            .iter()
            .filter(|(component, _)| excluded_component != Some(*component))
            .map(|(_, port)| port)
            .collect::<Vec<_>>();
        ports.sort_unstable_by_key(|port| port.scheduler_storage());
        ports
    }

    pub(crate) fn lock_except(
        &self,
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<SchedulerCheckpointBankGuard<'_>, SchedulerCheckpointError> {
        let mut ports = self
            .ports_in_lock_order(excluded_component)
            .into_iter()
            .map(|port| {
                port.lock_scheduler()
                    .map(|scheduler| LockedSchedulerCheckpointPort { port, scheduler })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ports.sort_unstable_by(|left, right| left.port.component().cmp(right.port.component()));
        Ok(SchedulerCheckpointBankGuard { ports })
    }

    pub(crate) fn try_lock_except(
        &self,
        excluded_component: Option<&CheckpointComponentId>,
    ) -> Result<SchedulerCheckpointBankGuard<'_>, SchedulerCheckpointError> {
        let mut ports = self
            .ports_in_lock_order(excluded_component)
            .into_iter()
            .map(|port| {
                port.try_scheduler()
                    .map(|scheduler| LockedSchedulerCheckpointPort { port, scheduler })
            })
            .collect::<Result<Vec<_>, _>>()?;
        ports.sort_unstable_by(|left, right| left.port.component().cmp(right.port.component()));
        Ok(SchedulerCheckpointBankGuard { ports })
    }
}

impl SchedulerCheckpointBankGuard<'_> {
    pub(crate) fn retain_pending_owned_events(
        &self,
        events: &mut Vec<SchedulerCheckpointOwnedEvent>,
    ) {
        let snapshots = self.snapshots();
        events.retain(|event| {
            snapshots
                .values()
                .find(|source| source.scheduler == event.scheduler)
                .is_none_or(|source| snapshot_owned_event(&source.snapshot, event.event).is_some())
        });
    }

    pub(crate) fn validate_quiescent_capture_with_owned_events(
        &self,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        let snapshots = self.snapshots();
        let excluded = resolve_owned_events(&snapshots, owned_events);
        for (component, source) in snapshots {
            let excluded_events = excluded
                .get(&component)
                .map(ResolvedSchedulerCheckpointEvents::excluded)
                .unwrap_or_default();
            let report = SchedulerCheckpointQuiescenceReport::from_snapshot_excluding(
                component,
                &source.snapshot,
                &excluded_events,
            );
            if !report.is_quiescent() {
                return Err(SchedulerCheckpointError::NonQuiescent { report });
            }
        }
        Ok(())
    }

    pub(crate) fn capture_all_into_with_owned_events(
        &self,
        registry: &mut CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        let snapshots = self.snapshots();
        let excluded = resolve_owned_events(&snapshots, owned_events);
        let captured = self
            .ports
            .iter()
            .map(|locked| {
                let component = locked.port.component();
                let source = snapshots
                    .get(component)
                    .expect("locked scheduler checkpoint port has a snapshot");
                let excluded_events = excluded
                    .get(component)
                    .map(ResolvedSchedulerCheckpointEvents::excluded)
                    .unwrap_or_default();
                locked
                    .port
                    .capture_record_from_snapshot(&source.snapshot, &excluded_events)
                    .map(|record| (locked.port, record))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in &captured {
            port.write_record(registry, record)?;
        }
        Ok(captured.into_iter().map(|(_, record)| record).collect())
    }

    pub(crate) fn validate_restore_from_with_owned_events(
        &mut self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<(), SchedulerCheckpointError> {
        let snapshots = self.snapshots();
        let excluded = resolve_owned_events(&snapshots, owned_events);
        let decoded = self.decode_records(registry)?;
        self.validate_records(&decoded, &excluded)
    }

    pub(crate) fn validate_live_o3_scheduler_restores(
        &self,
        registry: &CheckpointRegistry,
        restores: &[RiscvO3LiveSchedulerRestore],
        owned_events: &[SchedulerCheckpointOwnedEvent],
        mode: LiveO3SchedulerValidationMode,
    ) -> Result<Vec<CheckpointComponentId>, SchedulerCheckpointError> {
        let mut matched = Vec::new();
        let snapshots = self.snapshots();
        let resolved = resolve_owned_events(&snapshots, owned_events);
        let empty = ResolvedSchedulerCheckpointEvents::default();
        for locked in &self.ports {
            let component = locked.port.component();
            let restored = decode_registered_snapshot(component, registry)?;
            let current = &snapshots
                .get(component)
                .expect("locked scheduler checkpoint port has a snapshot")
                .snapshot;
            let resolved_events = resolved.get(component).unwrap_or(&empty);
            matched.extend(validate_live_o3_for_scheduler(
                locked.port.scheduler_instance(),
                current,
                &restored,
                resolved_events,
                restores,
                mode,
            )?);
        }
        Ok(matched)
    }

    pub(crate) fn restore_all_from_with_owned_events(
        &mut self,
        registry: &CheckpointRegistry,
        owned_events: &[SchedulerCheckpointOwnedEvent],
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        let snapshots = self.snapshots();
        let excluded = resolve_owned_events(&snapshots, owned_events);
        let decoded = self.decode_records(registry)?;
        self.validate_records(&decoded, &excluded)?;
        for (locked, record) in self.ports.iter_mut().zip(&decoded) {
            let component = locked.port.component().clone();
            let owned_events = excluded.get(&component).cloned().unwrap_or_default();
            let mut scheduler = locked.scheduler.checkpoint_access();
            restore_scheduler_projection(&mut scheduler, record.snapshot(), &owned_events)
                .map_err(|error| SchedulerCheckpointError::Scheduler { component, error })?;
        }
        Ok(decoded)
    }

    pub(crate) fn rebind_live_o3_scheduler_restores(
        &mut self,
        restores: &[RiscvO3LiveSchedulerRestore],
    ) {
        for locked in &mut self.ports {
            let mut scheduler = locked.scheduler.checkpoint_access();
            rebind_live_o3_for_scheduler(&mut scheduler, restores);
        }
    }

    fn snapshots(&self) -> BTreeMap<CheckpointComponentId, SchedulerCheckpointSourceSnapshot> {
        self.ports
            .iter()
            .map(|locked| {
                (
                    locked.port.component().clone(),
                    SchedulerCheckpointSourceSnapshot {
                        scheduler: locked.port.scheduler_instance(),
                        snapshot: locked.scheduler.snapshot(),
                    },
                )
            })
            .collect()
    }

    fn decode_records(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<SchedulerCheckpointRecord>, SchedulerCheckpointError> {
        self.ports
            .iter()
            .map(|locked| locked.port.decode_from(registry))
            .collect()
    }

    fn validate_records(
        &mut self,
        records: &[SchedulerCheckpointRecord],
        excluded: &BTreeMap<CheckpointComponentId, ResolvedSchedulerCheckpointEvents>,
    ) -> Result<(), SchedulerCheckpointError> {
        for (locked, record) in self.ports.iter_mut().zip(records) {
            let component = locked.port.component().clone();
            let events = excluded.get(&component).cloned().unwrap_or_default();
            let scheduler = locked.scheduler.checkpoint_access();
            validate_scheduler_projection_restore(&scheduler, record.snapshot(), &events)
                .map_err(|error| SchedulerCheckpointError::Scheduler { component, error })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rem6_checkpoint::CheckpointComponentId;
    use rem6_kernel::PartitionedScheduler;

    use super::{SchedulerCheckpointBank, SchedulerCheckpointPort};

    #[test]
    fn checkpoint_banks_share_scheduler_storage_lock_order() {
        let first = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let second = Arc::new(Mutex::new(PartitionedScheduler::new(1).unwrap()));
        let forward = SchedulerCheckpointBank::new([
            port("a", Arc::clone(&first)),
            port("b", Arc::clone(&second)),
        ])
        .unwrap();
        let reverse = SchedulerCheckpointBank::new([
            port("a", Arc::clone(&second)),
            port("b", Arc::clone(&first)),
        ])
        .unwrap();

        let forward_order = forward
            .ports_in_lock_order(None)
            .into_iter()
            .map(SchedulerCheckpointPort::scheduler_storage)
            .collect::<Vec<_>>();
        let reverse_order = reverse
            .ports_in_lock_order(None)
            .into_iter()
            .map(SchedulerCheckpointPort::scheduler_storage)
            .collect::<Vec<_>>();

        assert_eq!(forward_order, reverse_order);
        assert!(forward_order.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn port(
        component: &str,
        scheduler: Arc<Mutex<PartitionedScheduler>>,
    ) -> SchedulerCheckpointPort {
        SchedulerCheckpointPort::new(CheckpointComponentId::new(component).unwrap(), scheduler)
    }
}
