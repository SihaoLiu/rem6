use std::collections::BTreeMap;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6O3ExecutionModeAuthorityStat {
    path: String,
    value: u64,
}

impl Rem6O3ExecutionModeTraceTotals {
    pub(super) fn add_record(&mut self, record: &Rem6O3TraceRecord) {
        if let Some(execution_mode) = record.execution_mode() {
            if let Some(index) = execution_mode_index(execution_mode) {
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

impl Rem6O3ExecutionModeAuthorityStat {
    fn new(path: String, value: u64) -> Self {
        Self { path, value }
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

pub(super) fn o3_trace_execution_mode_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
    let mut targets = 0_u64;
    let mut modes = [0_u64; 3];
    let mut target_modes = BTreeMap::<(String, &'static str), u64>::new();

    for record in records {
        let Some(mode) = record.execution_mode() else {
            continue;
        };
        let Some(index) = execution_mode_index(mode) else {
            continue;
        };
        targets = targets.saturating_add(1);
        modes[index] = modes[index].saturating_add(1);
        let target = stat_path_segment(record.target());
        *target_modes.entry((target, mode)).or_default() += 1;
    }

    let mut stats = Vec::with_capacity(1 + EXECUTION_MODE_STATS.len() + target_modes.len());
    stats.push(Rem6O3ExecutionModeAuthorityStat::new(
        "execution_mode_authority.targets".to_string(),
        targets,
    ));
    for (index, (mode, _suffix)) in EXECUTION_MODE_STATS.iter().enumerate() {
        stats.push(Rem6O3ExecutionModeAuthorityStat::new(
            format!("execution_mode_authority.mode.{mode}"),
            modes[index],
        ));
    }
    for ((target, mode), value) in target_modes {
        stats.push(Rem6O3ExecutionModeAuthorityStat::new(
            format!("execution_mode_authority.target.{target}.mode.{mode}"),
            value,
        ));
    }
    stats
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
        if let Some(index) = execution_mode_index(mode) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

fn execution_mode_index(mode: &str) -> Option<usize> {
    EXECUTION_MODE_STATS
        .iter()
        .position(|(lane, _suffix)| *lane == mode)
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
