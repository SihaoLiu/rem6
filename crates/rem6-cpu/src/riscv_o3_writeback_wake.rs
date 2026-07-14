use rem6_kernel::{PendingEventSnapshot, SchedulerInstanceId, Tick};

use crate::{CpuFetchEvent, RiscvCore, RiscvCoreState};

impl RiscvCoreState {
    pub(crate) fn wake_ready_o3_scalar_memory_younger_window(
        &mut self,
        tick: Tick,
        fetch_events: &[CpuFetchEvent],
    ) {
        crate::riscv_live_retire_window::wake_o3_scalar_memory_younger_window(
            self,
            tick,
            fetch_events,
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RiscvO3WritebackWake {
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
}

impl RiscvO3WritebackWake {
    pub(crate) const fn new(scheduler: SchedulerInstanceId, event: PendingEventSnapshot) -> Self {
        Self { scheduler, event }
    }

    pub(crate) fn tick(self) -> Tick {
        self.event.tick()
    }

    pub(crate) const fn scheduler(self) -> SchedulerInstanceId {
        self.scheduler
    }

    pub(crate) const fn event(self) -> PendingEventSnapshot {
        self.event
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvO3WritebackWakeState {
    desired_tick: Option<Tick>,
    scheduled: Option<RiscvO3WritebackWake>,
    detached: Vec<RiscvO3WritebackWake>,
}

impl RiscvO3WritebackWakeState {
    pub(crate) fn set_desired_tick(&mut self, desired: Option<Tick>, now: Tick) {
        self.prune(now);
        if self.desired_tick == desired {
            return;
        }
        if let Some(wake) = self.scheduled.take() {
            if !self.detached.contains(&wake) {
                self.detached.push(wake);
            }
        }
        self.desired_tick = desired;
    }

    pub(crate) fn requested_tick(&mut self, now: Tick) -> Option<Tick> {
        self.prune(now);
        if self.scheduled.is_some() {
            return None;
        }
        self.desired_tick.filter(|tick| *tick > now)
    }

    pub(crate) fn mark_scheduled(
        &mut self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        let wake = RiscvO3WritebackWake::new(scheduler, event);
        if self.desired_tick == Some(wake.tick()) && self.scheduled.is_none() {
            self.scheduled = Some(wake);
        }
    }

    pub(crate) fn mark_fired(&mut self, now: Tick) {
        if self.scheduled.is_some_and(|wake| wake.tick() <= now) {
            self.scheduled = None;
        }
        self.prune(now);
    }

    pub(crate) fn owned_wakes(&self) -> Vec<RiscvO3WritebackWake> {
        self.scheduled
            .into_iter()
            .chain(self.detached.iter().copied())
            .collect()
    }

    fn prune(&mut self, now: Tick) {
        self.detached.retain(|wake| wake.tick() >= now);
    }
}

impl RiscvCore {
    pub fn requested_o3_writeback_wake_tick(&self, now: Tick) -> Option<Tick> {
        let mut state = self.state.lock().expect("riscv core lock");
        let desired = state
            .o3_runtime
            .earliest_unpublished_scalar_load_writeback_tick();
        state.o3_writeback_wake.set_desired_tick(desired, now);
        state.o3_writeback_wake.requested_tick(now)
    }

    pub fn mark_o3_writeback_wake_scheduled(
        &self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_writeback_wake
            .mark_scheduled(scheduler, event);
    }

    pub fn mark_o3_writeback_wake_fired(&self, now: Tick) {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_writeback_wake
            .mark_fired(now);
    }

    pub fn owned_o3_writeback_wakes(&self) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.state
            .lock()
            .expect("riscv core lock")
            .o3_writeback_wake
            .owned_wakes()
            .into_iter()
            .map(|wake| (wake.scheduler(), wake.event()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rem6_kernel::{PartitionId, PartitionedScheduler};

    use super::*;

    #[test]
    fn o3_writeback_wake_identical_request_deduplicates() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.set_desired_tick(Some(20), 11);

        assert_eq!(state.requested_tick(11), None);
        assert_eq!(state.owned_wakes().len(), 1);
    }

    #[test]
    fn o3_writeback_wake_earlier_request_detaches_later_schedule() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.set_desired_tick(Some(15), 11);

        assert_eq!(state.requested_tick(11), Some(15));
        assert_eq!(state.owned_wakes().len(), 1);
    }

    #[test]
    fn o3_writeback_wake_fired_schedule_clears_ownership() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);

        state.mark_fired(20);

        assert!(state.owned_wakes().is_empty());
    }

    #[test]
    fn o3_writeback_wake_detached_schedule_prunes_after_later_tick() {
        let mut state = RiscvO3WritebackWakeState::default();
        state.set_desired_tick(Some(20), 10);
        let (scheduler, event) = wake(20);
        state.mark_scheduled(scheduler, event);
        state.set_desired_tick(Some(15), 11);

        assert_eq!(state.owned_wakes().len(), 1);
        assert_eq!(state.requested_tick(20), None);
        assert_eq!(state.owned_wakes().len(), 1);
        assert_eq!(state.requested_tick(21), None);
        assert!(state.owned_wakes().is_empty());
    }

    fn wake(
        tick: u64,
    ) -> (
        rem6_kernel::SchedulerInstanceId,
        rem6_kernel::PendingEventSnapshot,
    ) {
        let mut scheduler = PartitionedScheduler::new(1).unwrap();
        let event = scheduler
            .schedule_at(PartitionId::new(0), tick, |_| {})
            .unwrap();
        (
            scheduler.instance_id(),
            scheduler.pending_event_snapshot(event).unwrap(),
        )
    }
}
