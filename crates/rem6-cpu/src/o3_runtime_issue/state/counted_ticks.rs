use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3LiveIssueCountedTicks {
    ticks: BTreeSet<u64>,
}

impl O3LiveIssueCountedTicks {
    pub(super) fn contains(&self, tick: u64) -> bool {
        self.ticks.contains(&tick)
    }

    pub(super) fn record(&mut self, tick: u64) -> bool {
        self.ticks.insert(tick)
    }

    pub(super) fn prune_before(&mut self, earliest_tick: u64) {
        self.ticks.retain(|tick| *tick >= earliest_tick);
    }

    pub(super) fn clear(&mut self) {
        self.ticks.clear();
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.ticks.len()
    }

    #[cfg(test)]
    pub(super) fn values(&self) -> Vec<u64> {
        self.ticks.iter().copied().collect()
    }
}

#[cfg(test)]
#[path = "counted_ticks_tests.rs"]
mod tests;
