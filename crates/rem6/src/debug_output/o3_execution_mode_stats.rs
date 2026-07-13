use std::collections::BTreeMap;

use super::{Rem6O3TraceRecord, Rem6O3TraceStat};

use crate::execution_mode_lanes::{
    execution_mode_lane_index, EXECUTION_MODE_LANES, EXECUTION_MODE_LANE_COUNT,
};
use crate::formatting::json_escape;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3ExecutionModeTraceTotals {
    counts: [u64; EXECUTION_MODE_LANE_COUNT],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Rem6O3ExecutionModeAuthorityTotals {
    targets: u64,
    modes: [u64; EXECUTION_MODE_LANE_COUNT],
    target_modes: BTreeMap<String, [u64; EXECUTION_MODE_LANE_COUNT]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6O3ExecutionModeAuthorityStat {
    path: String,
    unit: &'static str,
    value: u64,
}

impl Rem6O3ExecutionModeTraceTotals {
    pub(super) fn add_record(&mut self, record: &Rem6O3TraceRecord) {
        if let Some(execution_mode) = record.execution_mode() {
            if let Some(index) = execution_mode_lane_index(execution_mode) {
                self.counts[index] = self.counts[index].saturating_add(1);
            }
        }
    }

    pub(super) fn push_stats(self, stats: &mut Vec<Rem6O3TraceStat>) {
        for (index, lane) in EXECUTION_MODE_LANES.iter().enumerate() {
            stats.push(Rem6O3TraceStat {
                suffix: lane.o3_trace_stat_suffix(),
                unit: "Count",
                value: self.counts[index],
            });
        }
    }
}

impl Rem6O3ExecutionModeAuthorityTotals {
    fn add_record(
        &mut self,
        record: &Rem6O3TraceRecord,
        stat_path_segment: &impl Fn(&str) -> String,
    ) {
        let Some(mode) = record.execution_mode() else {
            return;
        };
        let Some(index) = execution_mode_lane_index(mode) else {
            return;
        };
        self.targets = self.targets.saturating_add(1);
        self.modes[index] = self.modes[index].saturating_add(1);
        let target = stat_path_segment(record.target());
        let counts = self.target_modes.entry(target).or_default();
        counts[index] = counts[index].saturating_add(1);
    }

    fn stats(self) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
        let mut stats = Vec::with_capacity(
            1 + EXECUTION_MODE_LANES.len() + self.target_modes.len() * EXECUTION_MODE_LANES.len(),
        );
        stats.push(Rem6O3ExecutionModeAuthorityStat::new(
            "execution_mode_authority.targets".to_string(),
            self.targets,
        ));
        for (index, lane) in EXECUTION_MODE_LANES.iter().enumerate() {
            stats.push(Rem6O3ExecutionModeAuthorityStat::new(
                format!("execution_mode_authority.mode.{}", lane.name()),
                self.modes[index],
            ));
        }
        for (target, counts) in self.target_modes {
            for (index, lane) in EXECUTION_MODE_LANES.iter().enumerate() {
                stats.push(Rem6O3ExecutionModeAuthorityStat::new(
                    format!(
                        "execution_mode_authority.target.{target}.mode.{}",
                        lane.name()
                    ),
                    counts[index],
                ));
            }
        }
        stats
    }
}

impl Rem6O3ExecutionModeAuthorityStat {
    pub(super) fn new(path: String, value: u64) -> Self {
        Self::with_unit(path, "Count", value)
    }

    pub(super) fn with_unit(path: String, unit: &'static str, value: u64) -> Self {
        Self { path, unit, value }
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

pub(super) fn o3_trace_execution_mode_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
    let mut totals = Rem6O3ExecutionModeAuthorityTotals::default();
    for record in records {
        totals.add_record(record, &stat_path_segment);
    }
    totals.stats()
}

pub(in crate::debug_output) fn o3_trace_cpu_execution_mode_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<(u32, Rem6O3ExecutionModeAuthorityStat)> {
    let mut cpu_totals = BTreeMap::<u32, Rem6O3ExecutionModeAuthorityTotals>::new();
    for record in records {
        if record.execution_mode().is_some() {
            cpu_totals
                .entry(record.cpu())
                .or_default()
                .add_record(record, &stat_path_segment);
        }
    }
    cpu_totals
        .into_iter()
        .flat_map(|(cpu, totals)| totals.stats().into_iter().map(move |stat| (cpu, stat)))
        .collect()
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

fn execution_mode_counts<'a>(
    modes: impl Iterator<Item = &'a str>,
) -> [u64; EXECUTION_MODE_LANE_COUNT] {
    let mut counts = [0_u64; EXECUTION_MODE_LANE_COUNT];
    for mode in modes {
        if let Some(index) = execution_mode_lane_index(mode) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

fn execution_mode_count_array_to_json(counts: [u64; EXECUTION_MODE_LANE_COUNT]) -> String {
    let fields = EXECUTION_MODE_LANES
        .iter()
        .zip(counts)
        .map(|(lane, count)| format!("\"{}\":{count}", lane.name()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}
