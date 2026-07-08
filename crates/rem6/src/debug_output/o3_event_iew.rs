use rem6_cpu::O3RuntimeTraceRecord;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3EventIewTotals {
    dispatched_insts: u64,
    insts_to_commit: u64,
    writeback_count: u64,
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
        self.dispatched_insts = self.dispatched_insts.saturating_add(1);
        self.writeback_count = self.writeback_count.saturating_add(1);
        self.insts_to_commit = self
            .insts_to_commit
            .saturating_add(u64::from(event.rob_committed()));
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

    pub(super) const fn dispatched_insts(self) -> u64 {
        self.dispatched_insts
    }

    pub(super) const fn insts_to_commit(self) -> u64 {
        self.insts_to_commit
    }

    pub(super) const fn writeback_count(self) -> u64 {
        self.writeback_count
    }

    pub(super) const fn writeback_rate_ppm(self, span_ticks: u64) -> u64 {
        ratio_ppm(self.writeback_count, span_ticks)
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

    pub(super) fn stats(self) -> [(&'static str, u64); 8] {
        [
            ("event.iew_dispatched_insts", self.dispatched_insts),
            ("event.iew_insts_to_commit", self.insts_to_commit),
            ("event.iew_writeback_count", self.writeback_count),
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

const fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        0
    } else {
        let ppm = (numerator as u128).saturating_mul(1_000_000) / (denominator as u128);
        if ppm > u64::MAX as u128 {
            u64::MAX
        } else {
            ppm as u64
        }
    }
}
