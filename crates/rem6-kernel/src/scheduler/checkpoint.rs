use super::*;

impl PartitionedScheduler {
    #[doc(hidden)]
    pub fn checkpoint_access(&mut self) -> SchedulerCheckpointAccess<'_> {
        SchedulerCheckpointAccess { scheduler: self }
    }

    fn restore_checkpoint_projection(
        &mut self,
        snapshot: &SchedulerSnapshot,
        discarded_events: &[PendingEventSnapshot],
        preserved_events: &[PendingEventSnapshot],
    ) -> Result<(), SchedulerError> {
        self.validate_checkpoint_projection(snapshot, discarded_events, preserved_events)?;

        let mut preserved = Vec::with_capacity(preserved_events.len());
        for event in preserved_events {
            preserved.push(
                self.partitions[event.id().partition().index() as usize]
                    .remove_event(event.id())
                    .expect("validated preserved event is pending"),
            );
        }
        for event in discarded_events {
            self.partitions[event.id().partition().index() as usize]
                .remove_event(event.id())
                .expect("validated discarded event is pending");
        }
        self.restore_quiescent(snapshot)?;
        for event in preserved {
            self.partitions[event.id.partition().index() as usize].restore_preserved_event(event);
        }
        Ok(())
    }

    fn validate_checkpoint_projection(
        &self,
        snapshot: &SchedulerSnapshot,
        discarded_events: &[PendingEventSnapshot],
        preserved_events: &[PendingEventSnapshot],
    ) -> Result<(), SchedulerError> {
        self.validate_quiescent_snapshot_compatibility(snapshot)?;
        for event in discarded_events.iter().chain(preserved_events) {
            if self.pending_event_snapshot(event.id()) != Some(*event) {
                return Err(SchedulerError::EventNotPending { id: event.id() });
            }
        }
        if has_duplicate_or_conflicting_events(discarded_events, preserved_events) {
            return Err(SchedulerError::RestoreWouldDiscardPendingEvents {
                pending_events: self.total_pending_events(),
            });
        }
        let classified_events = discarded_events.len() + preserved_events.len();
        if self.total_pending_events() != classified_events {
            return Err(SchedulerError::RestoreWouldDiscardPendingEvents {
                pending_events: self.total_pending_events()
                    - classified_events.min(self.total_pending_events()),
            });
        }
        for event in preserved_events {
            let partition = &snapshot.partitions[event.id().partition().index() as usize];
            if event.tick() < partition.now() {
                return Err(SchedulerError::InThePast {
                    partition: event.id().partition(),
                    now: partition.now(),
                    requested: event.tick(),
                });
            }
        }
        Ok(())
    }
}

impl SchedulerContext<'_> {
    #[doc(hidden)]
    pub fn checkpoint_access(&mut self) -> SchedulerCheckpointAccess<'_> {
        SchedulerCheckpointAccess {
            scheduler: self.scheduler,
        }
    }
}

#[doc(hidden)]
pub struct SchedulerCheckpointAccess<'a> {
    scheduler: &'a mut PartitionedScheduler,
}

impl SchedulerCheckpointAccess<'_> {
    pub fn reborrow(&mut self) -> SchedulerCheckpointAccess<'_> {
        SchedulerCheckpointAccess {
            scheduler: self.scheduler,
        }
    }

    pub fn instance_id(&self) -> SchedulerInstanceId {
        self.scheduler.instance_id()
    }

    pub fn storage_id(&self) -> SchedulerStorageId {
        SchedulerStorageId::for_scheduler(self.scheduler)
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        self.scheduler.snapshot()
    }

    pub fn pending_event_snapshot(&self, id: PartitionEventId) -> Option<PendingEventSnapshot> {
        self.scheduler.pending_event_snapshot(id)
    }

    pub fn validate_quiescent_snapshot_compatibility(
        &self,
        snapshot: &SchedulerSnapshot,
    ) -> Result<(), SchedulerError> {
        self.scheduler
            .validate_quiescent_snapshot_compatibility(snapshot)
    }

    pub fn restore_quiescent(
        &mut self,
        snapshot: &SchedulerSnapshot,
    ) -> Result<(), SchedulerError> {
        self.scheduler.restore_quiescent(snapshot)
    }

    pub fn restore_projection(
        &mut self,
        snapshot: &SchedulerSnapshot,
        discarded_events: &[PendingEventSnapshot],
        preserved_events: &[PendingEventSnapshot],
    ) -> Result<(), SchedulerError> {
        self.scheduler
            .restore_checkpoint_projection(snapshot, discarded_events, preserved_events)
    }

    pub fn validate_projection_restore(
        &self,
        snapshot: &SchedulerSnapshot,
        discarded_events: &[PendingEventSnapshot],
        preserved_events: &[PendingEventSnapshot],
    ) -> Result<(), SchedulerError> {
        self.scheduler
            .validate_checkpoint_projection(snapshot, discarded_events, preserved_events)
    }

    pub fn discard_exact_events(
        &mut self,
        events: &[PendingEventSnapshot],
    ) -> Result<(), SchedulerError> {
        for event in events {
            if self.scheduler.pending_event_snapshot(event.id()) != Some(*event) {
                return Err(SchedulerError::EventNotPending { id: event.id() });
            }
        }
        if has_duplicate_or_conflicting_events(events, &[]) {
            return Err(SchedulerError::RestoreWouldDiscardPendingEvents {
                pending_events: events.len(),
            });
        }
        for event in events {
            self.scheduler.partitions[event.id().partition().index() as usize]
                .remove_event(event.id())
                .expect("validated discarded event is pending");
        }
        if !events.is_empty() {
            self.scheduler.timeline_revision = self.scheduler.timeline_revision.wrapping_add(1);
        }
        Ok(())
    }
}

impl PartitionQueue {
    fn restore_preserved_event(&mut self, event: PartitionEvent) {
        self.next_id = self.next_id.max(event.id.local().saturating_add(1));
        self.next_order = self.next_order.max(event.order.saturating_add(1));
        self.pending.push(event);
    }
}

fn has_duplicate_or_conflicting_events(
    discarded_events: &[PendingEventSnapshot],
    preserved_events: &[PendingEventSnapshot],
) -> bool {
    discarded_events
        .iter()
        .any(|event| preserved_events.contains(event))
        || discarded_events
            .iter()
            .enumerate()
            .any(|(index, event)| discarded_events[index + 1..].contains(event))
        || preserved_events
            .iter()
            .enumerate()
            .any(|(index, event)| preserved_events[index + 1..].contains(event))
}
