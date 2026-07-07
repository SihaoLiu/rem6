use super::{Rem6O3TraceRecord, Rem6O3TraceStat};

use crate::formatting::json_escape;

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

pub(super) fn o3_trace_execution_mode_authority_to_json(
    target: &str,
    mode: Option<&'static str>,
) -> String {
    let counts = execution_mode_counts(mode.into_iter());
    let target = mode.map_or_else(
        || "{}".to_string(),
        |_| {
            format!(
                "{{\"{}\":{{\"mode\":{}}}}}",
                json_escape(target),
                execution_mode_count_array_to_json(counts)
            )
        },
    );
    format!(
        "{{\"targets\":{},\"mode\":{},\"target\":{}}}",
        u64::from(mode.is_some()),
        execution_mode_count_array_to_json(counts),
        target
    )
}

fn execution_mode_counts<'a>(modes: impl Iterator<Item = &'a str>) -> [u64; 3] {
    let mut counts = [0_u64; 3];
    for mode in modes {
        if let Some(index) = EXECUTION_MODE_STATS
            .iter()
            .position(|(lane, _suffix)| *lane == mode)
        {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

fn execution_mode_count_array_to_json(counts: [u64; 3]) -> String {
    let fields = EXECUTION_MODE_STATS
        .iter()
        .zip(counts)
        .map(|((mode, _suffix), count)| format!("\"{mode}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}
