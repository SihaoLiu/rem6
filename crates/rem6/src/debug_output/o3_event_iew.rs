use rem6_cpu::O3RuntimeTraceRecord;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3EventIewTotals {
    dependency_producers: u64,
    dependency_consumers: u64,
    predicted_taken_incorrect: u64,
    predicted_not_taken_incorrect: u64,
}

impl Rem6O3EventIewTotals {
    pub(super) fn from_events(events: &[O3RuntimeTraceRecord]) -> Self {
        let mut totals = Self::default();
        for event in events {
            totals.add_event(*event);
        }
        totals
    }

    pub(super) fn add_event(&mut self, event: O3RuntimeTraceRecord) {
        self.dependency_producers = self
            .dependency_producers
            .saturating_add(event.iew_dependency_producers());
        self.dependency_consumers = self
            .dependency_consumers
            .saturating_add(event.iew_dependency_consumers());
        if event.branch_mispredicted() {
            if event.branch_predicted_taken() {
                self.predicted_taken_incorrect = self.predicted_taken_incorrect.saturating_add(1);
            } else {
                self.predicted_not_taken_incorrect =
                    self.predicted_not_taken_incorrect.saturating_add(1);
            }
        }
    }

    pub(super) const fn dependency_producers(self) -> u64 {
        self.dependency_producers
    }

    pub(super) const fn dependency_consumers(self) -> u64 {
        self.dependency_consumers
    }

    pub(super) const fn predicted_taken_incorrect(self) -> u64 {
        self.predicted_taken_incorrect
    }

    pub(super) const fn predicted_not_taken_incorrect(self) -> u64 {
        self.predicted_not_taken_incorrect
    }

    pub(super) const fn branch_mispredicts(self) -> u64 {
        self.predicted_taken_incorrect
            .saturating_add(self.predicted_not_taken_incorrect)
    }

    pub(super) fn stats(self) -> [(&'static str, u64); 5] {
        [
            ("event.iew_dependency_producers", self.dependency_producers),
            ("event.iew_dependency_consumers", self.dependency_consumers),
            (
                "event.iew_predicted_taken_incorrect",
                self.predicted_taken_incorrect,
            ),
            (
                "event.iew_predicted_not_taken_incorrect",
                self.predicted_not_taken_incorrect,
            ),
            ("event.iew_branch_mispredicts", self.branch_mispredicts()),
        ]
    }
}
