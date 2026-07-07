use super::{Rem6O3TraceRecord, Rem6O3TraceStat};

const EXECUTION_MODE_STATS: [(&str, &'static str); 3] = [
    ("functional", "execution_mode.functional"),
    ("timing", "execution_mode.timing"),
    ("detailed", "execution_mode.detailed"),
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3ExecutionModeTraceTotals {
    counts: [u64; 3],
}

impl Rem6O3ExecutionModeTraceTotals {
    pub(super) fn add_record(&mut self, record: &Rem6O3TraceRecord) {
        if let Some(execution_mode) = record.execution_mode() {
            if let Some(index) = EXECUTION_MODE_STATS
                .iter()
                .position(|(mode, _suffix)| *mode == execution_mode)
            {
                self.counts[index] = self.counts[index].saturating_add(1);
            }
        }
    }

    pub(super) fn push_stats(self, stats: &mut Vec<Rem6O3TraceStat>) {
        for (index, (_mode, suffix)) in EXECUTION_MODE_STATS.iter().enumerate() {
            stats.push(Rem6O3TraceStat {
                suffix: *suffix,
                unit: "Count",
                value: self.counts[index],
            });
        }
    }
}
