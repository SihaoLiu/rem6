use crate::probes::{ProbeEvent, ProbePayload, ProbePointId};
use crate::StatsError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstTrackerUpdate {
    counter: u64,
    remaining_thresholds: bool,
}

impl InstTrackerUpdate {
    pub const fn new(counter: u64, remaining_thresholds: bool) -> Self {
        Self {
            counter,
            remaining_thresholds,
        }
    }

    pub const fn counter(&self) -> u64 {
        self.counter
    }

    pub const fn has_remaining_thresholds(&self) -> bool {
        self.remaining_thresholds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalInstTrackerSnapshot {
    counter: u64,
    thresholds: Vec<u64>,
}

impl GlobalInstTrackerSnapshot {
    pub fn new(counter: u64, thresholds: Vec<u64>) -> Self {
        Self {
            counter,
            thresholds,
        }
    }

    pub const fn counter(&self) -> u64 {
        self.counter
    }

    pub fn thresholds(&self) -> &[u64] {
        &self.thresholds
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalInstTracker {
    counter: u64,
    thresholds: Vec<u64>,
}

impl GlobalInstTracker {
    pub fn new(thresholds: Vec<u64>) -> Self {
        let mut tracker = Self {
            counter: 0,
            thresholds: Vec::new(),
        };
        for threshold in thresholds {
            tracker.add_threshold(threshold);
        }
        tracker
    }

    pub fn from_snapshot(snapshot: &GlobalInstTrackerSnapshot) -> Result<Self, StatsError> {
        let mut thresholds = Vec::new();
        for threshold in snapshot.thresholds() {
            if *threshold <= snapshot.counter() {
                return Err(StatsError::UnreachableInstThreshold {
                    threshold: *threshold,
                    counter: snapshot.counter(),
                });
            }
            insert_threshold(&mut thresholds, *threshold)
                .map_err(|threshold| StatsError::DuplicateInstThreshold { threshold })?;
        }

        Ok(Self {
            counter: snapshot.counter(),
            thresholds,
        })
    }

    pub const fn counter(&self) -> u64 {
        self.counter
    }

    pub fn thresholds(&self) -> &[u64] {
        &self.thresholds
    }

    pub fn add_threshold(&mut self, threshold: u64) {
        let _ = insert_threshold(&mut self.thresholds, threshold);
    }

    pub fn reset_counter(&mut self) {
        self.counter = 0;
    }

    pub fn reset_thresholds(&mut self) {
        self.thresholds.clear();
    }

    pub fn record_retired_inst(&mut self) -> Result<Option<InstTrackerUpdate>, StatsError> {
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or(StatsError::InstTrackerCounterOverflow)?;

        match self.thresholds.binary_search(&self.counter) {
            Ok(index) => {
                self.thresholds.remove(index);
                Ok(Some(InstTrackerUpdate::new(
                    self.counter,
                    !self.thresholds.is_empty(),
                )))
            }
            Err(_) => Ok(None),
        }
    }

    pub fn snapshot(&self) -> GlobalInstTrackerSnapshot {
        GlobalInstTrackerSnapshot::new(self.counter, self.thresholds.clone())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalInstTracker {
    listening: bool,
}

impl LocalInstTracker {
    pub const fn new(start_listening: bool) -> Self {
        Self {
            listening: start_listening,
        }
    }

    pub const fn is_listening(&self) -> bool {
        self.listening
    }

    pub fn start_listening(&mut self) {
        self.listening = true;
    }

    pub fn stop_listening(&mut self) {
        self.listening = false;
    }

    pub fn observe_retired_inst(
        &self,
        global: &mut GlobalInstTracker,
    ) -> Result<Option<InstTrackerUpdate>, StatsError> {
        if !self.listening {
            return Ok(None);
        }
        global.record_retired_inst()
    }

    pub fn observe_retired_insts_probe_event(
        &self,
        event: &ProbeEvent,
        retired_insts_point: ProbePointId,
        global: &mut GlobalInstTracker,
    ) -> Result<Option<InstTrackerUpdate>, StatsError> {
        if !self.listening || event.point() != retired_insts_point {
            return Ok(None);
        }
        let ProbePayload::Counter { amount: _ } = event.payload() else {
            return Ok(None);
        };
        global.record_retired_inst()
    }
}

fn insert_threshold(thresholds: &mut Vec<u64>, threshold: u64) -> Result<(), u64> {
    match thresholds.binary_search(&threshold) {
        Ok(_) => Err(threshold),
        Err(index) => {
            thresholds.insert(index, threshold);
            Ok(())
        }
    }
}
