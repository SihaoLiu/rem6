use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueDecisionDelta {
    pub(in crate::o3_runtime) new_cycle: bool,
    pub(in crate::o3_runtime) issued_rows: usize,
    pub(in crate::o3_runtime) resource_blocked_rows: usize,
    pub(in crate::o3_runtime) dependency_blocked_rows: usize,
    pub(in crate::o3_runtime) max_rows_at_tick: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveIssueBlockedKind {
    Resource,
    Dependency,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3LiveIssueActiveTick {
    tick: u64,
    new_cycle: bool,
    issued_sequences: BTreeMap<u64, usize>,
    blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    baseline_issued_sequences: BTreeMap<u64, usize>,
    baseline_blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    max_rows_after_reset: usize,
    observed_after_reset: bool,
    projected_delta: Option<O3LiveIssueDecisionDelta>,
}

impl O3LiveIssueActiveTick {
    pub(super) fn at(tick: u64, new_cycle: bool) -> Self {
        Self {
            tick,
            new_cycle,
            ..Self::default()
        }
    }

    pub(super) const fn tick(&self) -> u64 {
        self.tick
    }

    pub(super) const fn projected_delta(&self) -> Option<O3LiveIssueDecisionDelta> {
        self.projected_delta
    }

    fn refresh_projection(&mut self) {
        self.projected_delta = self.observed_after_reset.then(|| O3LiveIssueDecisionDelta {
            new_cycle: self.new_cycle,
            issued_rows: self.new_issued_rows(),
            resource_blocked_rows: self.new_blocked_rows(O3LiveIssueBlockedKind::Resource),
            dependency_blocked_rows: self.new_blocked_rows(O3LiveIssueBlockedKind::Dependency),
            max_rows_at_tick: self.max_rows_after_reset,
        });
    }

    fn new_issued_rows(&self) -> usize {
        self.issued_sequences
            .iter()
            .fold(0, |total, (sequence, rows)| {
                total.saturating_add(
                    rows.saturating_sub(
                        self.baseline_issued_sequences
                            .get(sequence)
                            .copied()
                            .unwrap_or_default(),
                    ),
                )
            })
    }

    fn new_blocked_rows(&self, kind: O3LiveIssueBlockedKind) -> usize {
        self.blocked_sequences
            .iter()
            .filter(|(sequence, blocked)| {
                **blocked == kind && self.baseline_blocked_sequences.get(sequence) != Some(blocked)
            })
            .count()
    }

    pub(super) fn observe(
        &mut self,
        issued: &[u64],
        resource_blocked: &[u64],
        dependency_blocked: &[u64],
        max_rows_at_tick: usize,
    ) {
        for sequence in issued {
            let rows = self.issued_sequences.entry(*sequence).or_default();
            *rows = rows.saturating_add(1);
        }
        self.blocked_sequences.clear();
        self.blocked_sequences.extend(
            resource_blocked
                .iter()
                .copied()
                .map(|sequence| (sequence, O3LiveIssueBlockedKind::Resource)),
        );
        self.blocked_sequences.extend(
            dependency_blocked
                .iter()
                .copied()
                .map(|sequence| (sequence, O3LiveIssueBlockedKind::Dependency)),
        );
        self.max_rows_after_reset = self.max_rows_after_reset.max(max_rows_at_tick);
        self.observed_after_reset = true;
        self.refresh_projection();
    }

    pub(super) fn reset_baseline(&mut self) {
        self.baseline_issued_sequences = self.issued_sequences.clone();
        self.baseline_blocked_sequences = self.blocked_sequences.clone();
        self.max_rows_after_reset = 0;
        self.observed_after_reset = false;
        self.new_cycle = true;
        self.refresh_projection();
    }

    pub(super) fn remove_blocked(&mut self, sequence: u64) {
        self.blocked_sequences.remove(&sequence);
        self.baseline_blocked_sequences.remove(&sequence);
        self.refresh_projection();
    }

    pub(super) fn clear_blocked(&mut self) -> bool {
        let changed =
            !self.blocked_sequences.is_empty() || !self.baseline_blocked_sequences.is_empty();
        self.blocked_sequences.clear();
        self.baseline_blocked_sequences.clear();
        self.refresh_projection();
        changed
    }

    pub(super) fn retain_blocked_before(&mut self, boundary: u64) {
        self.blocked_sequences
            .retain(|sequence, _| *sequence < boundary);
        self.baseline_blocked_sequences
            .retain(|sequence, _| *sequence < boundary);
        self.refresh_projection();
    }
}
