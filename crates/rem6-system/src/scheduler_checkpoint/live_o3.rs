use std::collections::BTreeSet;

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_kernel::{
    PartitionSnapshot, ScheduledEventKind, SchedulerCheckpointAccess, SchedulerInstanceId,
    SchedulerSnapshot,
};

use crate::riscv_checkpoint::RiscvO3LiveSchedulerRestore;
use crate::{schedule_o3_writeback_wake, O3WritebackWakeSchedule};

use super::{
    decode_registered_snapshot, resolve_owned_events_for_scheduler, LiveO3SchedulerValidationMode,
    ResolvedSchedulerCheckpointEvents, SchedulerCheckpointContext, SchedulerCheckpointError,
    SchedulerCheckpointOwnedEvent,
};

impl SchedulerCheckpointContext<'_> {
    pub(crate) fn validate_live_o3_scheduler_restores(
        &self,
        registry: &CheckpointRegistry,
        restores: &[RiscvO3LiveSchedulerRestore],
        owned_events: &[SchedulerCheckpointOwnedEvent],
        mode: LiveO3SchedulerValidationMode,
    ) -> Result<Vec<CheckpointComponentId>, SchedulerCheckpointError> {
        let restored = decode_registered_snapshot(&self.component, registry)?;
        let current = self.scheduler.snapshot();
        let discarded = resolve_owned_events_for_scheduler(
            self.scheduler.instance_id(),
            &current,
            owned_events,
        );
        validate_live_o3_for_scheduler(
            self.scheduler.instance_id(),
            &current,
            &restored,
            &discarded,
            restores,
            mode,
        )
    }

    pub(crate) fn rebind_live_o3_scheduler_restores(
        &mut self,
        restores: &[RiscvO3LiveSchedulerRestore],
    ) -> BTreeSet<CheckpointComponentId> {
        rebind_live_o3_for_scheduler(&mut self.scheduler, restores)
    }
}

pub(super) fn validate_live_o3_for_scheduler(
    scheduler: SchedulerInstanceId,
    current: &SchedulerSnapshot,
    restored: &SchedulerSnapshot,
    resolved_events: &ResolvedSchedulerCheckpointEvents,
    restores: &[RiscvO3LiveSchedulerRestore],
    mode: LiveO3SchedulerValidationMode,
) -> Result<Vec<CheckpointComponentId>, SchedulerCheckpointError> {
    let matching = restores
        .iter()
        .filter(|restore| restore.wake().scheduler_instance_raw == scheduler.checkpoint_raw())
        .collect::<Vec<_>>();
    let mut ticks = BTreeSet::new();
    let mut orders = BTreeSet::new();
    for restore in &matching {
        let wake = restore.wake();
        let invalid = |reason: &str| SchedulerCheckpointError::InvalidLiveO3Authority {
            component: restore.component().clone(),
            reason: reason.to_string(),
        };
        if mode == LiveO3SchedulerValidationMode::SourceCapture {
            let owned = restore.core().owned_o3_writeback_wakes();
            let [(owned_scheduler, owned_event)] = owned.as_slice() else {
                return Err(invalid("source core does not own exactly one O3 wake"));
            };
            if *owned_scheduler != scheduler
                || resolved_events
                    .discarded
                    .iter()
                    .filter(|event| *event == owned_event)
                    .count()
                    != 1
                || owned_event.partition() != wake.partition
                || owned_event.tick() != wake.tick
                || owned_event.order() != wake.scheduler_order
                || owned_event.kind() != wake.kind
            {
                return Err(invalid(
                    "saved O3 wake does not match source scheduler authority",
                ));
            }
        }
        let partition = restored
            .partitions()
            .get(wake.partition.index() as usize)
            .filter(|partition| partition.partition() == wake.partition)
            .ok_or_else(|| invalid("saved partition is not present"))?;
        if wake.tick < restored.now() || wake.tick < partition.now() {
            return Err(invalid(
                "saved wake tick is before the restored scheduler clock",
            ));
        }
        let rebind_count = matching
            .iter()
            .filter(|candidate| candidate.wake().partition == wake.partition)
            .count() as u64;
        let (next_local, next_order) = resolved_events
            .preserved
            .iter()
            .filter(|event| event.partition() == wake.partition)
            .fold(
                (partition.next_event_local(), partition.next_event_order()),
                |(local, order), event| {
                    (
                        local.max(event.id().local().saturating_add(1)),
                        order.max(event.order().saturating_add(1)),
                    )
                },
            );
        if [next_local, next_order]
            .into_iter()
            .any(|frontier| frontier.checked_add(rebind_count).is_none())
        {
            return Err(invalid("restored scheduler event frontier is exhausted"));
        }
        if wake.scheduler_order >= partition.next_event_order() {
            return Err(invalid(
                "saved wake order is outside the scheduler order frontier",
            ));
        }
        if partition
            .pending_events()
            .iter()
            .any(|event| event.order() == wake.scheduler_order)
        {
            return Err(invalid("saved wake order is occupied"));
        }
        if partition
            .pending_events()
            .iter()
            .any(|event| event.tick() == wake.tick)
        {
            return Err(invalid("restored scheduler has a same-tick competitor"));
        }
        if !matches!(
            wake.kind,
            ScheduledEventKind::Serial | ScheduledEventKind::Parallel
        ) {
            return Err(invalid("saved wake kind is invalid"));
        }
        if !ticks.insert((wake.partition, wake.tick)) {
            return Err(invalid(
                "another live O3 wake has the same partition and tick",
            ));
        }
        if !orders.insert((wake.partition, wake.scheduler_order)) {
            return Err(invalid("another live O3 wake has the same scheduler order"));
        }
        let competitor = current
            .partitions()
            .get(wake.partition.index() as usize)
            .into_iter()
            .flat_map(PartitionSnapshot::pending_events)
            .any(|event| event.tick() == wake.tick && !resolved_events.discarded.contains(event));
        if competitor {
            return Err(invalid("destination scheduler has a same-tick competitor"));
        }
    }
    Ok(matching
        .into_iter()
        .map(|restore| restore.component().clone())
        .collect())
}

pub(super) fn rebind_live_o3_for_scheduler(
    scheduler: &mut SchedulerCheckpointAccess<'_>,
    restores: &[RiscvO3LiveSchedulerRestore],
) -> BTreeSet<CheckpointComponentId> {
    let scheduler_raw = scheduler.instance_id().checkpoint_raw();
    let mut rebound = BTreeSet::new();
    for restore in restores
        .iter()
        .filter(|restore| restore.wake().scheduler_instance_raw == scheduler_raw)
    {
        let wake = restore.wake();
        restore.core().forget_discarded_o3_writeback_wakes();
        let desired = restore
            .core()
            .requested_o3_writeback_wake_tick(scheduler.snapshot().now());
        assert_eq!(
            desired,
            Some(wake.tick),
            "validated O3 wake deadline changed"
        );
        schedule_o3_writeback_wake(
            restore.core(),
            scheduler,
            wake.tick,
            wake.kind,
            O3WritebackWakeSchedule::CheckpointRebound,
        )
        .expect("validated O3 wake schedule succeeds");
        rebound.insert(restore.component().clone());
    }
    rebound
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::Register;
    use rem6_kernel::PartitionedScheduler;

    use super::*;

    #[allow(dead_code)]
    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/live_o3.rs"
        ));
    }

    #[test]
    fn live_o3_source_cross_reference_rejects_wrong_kind_atomically() {
        assert_source_cross_reference_rejected(|wake| {
            wake.kind = match wake.kind {
                ScheduledEventKind::Serial => ScheduledEventKind::Parallel,
                ScheduledEventKind::Parallel => ScheduledEventKind::Serial,
            };
        });
    }

    #[test]
    fn live_o3_source_cross_reference_rejects_wrong_partition_atomically() {
        assert_source_cross_reference_rejected(|wake| {
            wake.partition = rem6_kernel::PartitionId::new(1);
        });
    }

    #[test]
    fn live_o3_restore_preflight_rejects_missing_wake_partition_atomically() {
        let mut scheduler = PartitionedScheduler::new(2).unwrap();
        let seeded = support::seed_live_core(0, &mut scheduler, ScheduledEventKind::Serial);
        let current = scheduler.snapshot();
        let restored = SchedulerSnapshot::with_parallel_worker_limit(
            current.now(),
            current.min_remote_delay(),
            current.max_parallel_workers(),
            vec![quiescent_partition(&current.partitions()[1])],
        );
        let restore = RiscvO3LiveSchedulerRestore::for_scheduler_validation_test(
            CheckpointComponentId::new("cpu0").unwrap(),
            seeded.core.clone(),
            seeded.live.wake,
        );
        let resolved = ResolvedSchedulerCheckpointEvents {
            discarded: vec![seeded.wake],
            preserved: Vec::new(),
        };
        seeded
            .core
            .write_register(Register::new(7).unwrap(), 0xcafe);
        let scheduler_before = scheduler.snapshot();
        let hart_before = seeded.core.checkpoint_hart_state();
        let runtime_before = seeded.core.o3_runtime_snapshot();
        let stats_before = seeded.core.o3_runtime_stats();
        let wakes_before = seeded.core.owned_o3_writeback_wakes();

        let error = validate_live_o3_for_scheduler(
            scheduler.instance_id(),
            &current,
            &restored,
            &resolved,
            &[restore],
            LiveO3SchedulerValidationMode::Restore,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            SchedulerCheckpointError::InvalidLiveO3Authority { ref reason, .. }
                if reason == "saved partition is not present"
        ));
        assert_eq!(scheduler.snapshot(), scheduler_before);
        assert_eq!(seeded.core.checkpoint_hart_state(), hart_before);
        assert_eq!(seeded.core.o3_runtime_snapshot(), runtime_before);
        assert_eq!(seeded.core.o3_runtime_stats(), stats_before);
        assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    }

    fn assert_source_cross_reference_rejected(
        mutation: impl FnOnce(&mut rem6_cpu::RiscvO3LiveCheckpointWake),
    ) {
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let seeded = support::seed_live_core(0, &mut scheduler, ScheduledEventKind::Serial);
        let current = scheduler.snapshot();
        let restored = quiescent_projection(&current);
        let mut wake = seeded.live.wake;
        mutation(&mut wake);
        let restore = RiscvO3LiveSchedulerRestore::for_scheduler_validation_test(
            CheckpointComponentId::new("cpu0").unwrap(),
            seeded.core.clone(),
            wake,
        );
        let resolved = ResolvedSchedulerCheckpointEvents {
            discarded: vec![seeded.wake],
            preserved: Vec::new(),
        };
        seeded
            .core
            .write_register(Register::new(7).unwrap(), 0xcafe);
        let scheduler_before = scheduler.snapshot();
        let hart_before = seeded.core.checkpoint_hart_state();
        let runtime_before = seeded.core.o3_runtime_snapshot();
        let stats_before = seeded.core.o3_runtime_stats();
        let wakes_before = seeded.core.owned_o3_writeback_wakes();

        let error = validate_live_o3_for_scheduler(
            scheduler.instance_id(),
            &current,
            &restored,
            &resolved,
            &[restore],
            LiveO3SchedulerValidationMode::SourceCapture,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            SchedulerCheckpointError::InvalidLiveO3Authority { ref reason, .. }
                if reason == "saved O3 wake does not match source scheduler authority"
        ));
        assert_eq!(scheduler.snapshot(), scheduler_before);
        assert_eq!(seeded.core.checkpoint_hart_state(), hart_before);
        assert_eq!(seeded.core.o3_runtime_snapshot(), runtime_before);
        assert_eq!(seeded.core.o3_runtime_stats(), stats_before);
        assert_eq!(seeded.core.owned_o3_writeback_wakes(), wakes_before);
    }

    fn quiescent_projection(source: &SchedulerSnapshot) -> SchedulerSnapshot {
        let partitions = source
            .partitions()
            .iter()
            .map(quiescent_partition)
            .collect();
        SchedulerSnapshot::with_parallel_worker_limit(
            source.now(),
            source.min_remote_delay(),
            source.max_parallel_workers(),
            partitions,
        )
    }

    fn quiescent_partition(partition: &PartitionSnapshot) -> PartitionSnapshot {
        PartitionSnapshot::quiescent_with_orders(
            partition.partition(),
            partition.now(),
            partition.next_event_local(),
            partition.next_event_order(),
            partition.next_remote_order(),
            partition.next_progress_order(),
        )
    }
}
