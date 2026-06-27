use std::collections::BTreeMap;

use super::{
    Rem6BranchTraceRecord, Rem6DataTraceRecord, Rem6ExecTraceRecord, Rem6FetchTraceRecord,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6ExecTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6FetchTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6DataTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6BranchTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ExecTraceStatSummary {
    records: u64,
    retired: u64,
    bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FetchTraceStatSummary {
    records: u64,
    bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DataTraceStatSummary {
    records: u64,
    loads: u64,
    stores: u64,
    atomics: u64,
    bytes: u64,
    load_bytes: u64,
    store_bytes: u64,
    atomic_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct BranchTraceStatSummary {
    records: u64,
    conditional: u64,
    unconditional: u64,
    predicted_taken: u64,
    resolved_taken: u64,
    mispredictions: u64,
    repairs: u64,
    flushed: u64,
}

impl Rem6ExecTraceStat {
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

impl Rem6FetchTraceStat {
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

impl Rem6DataTraceStat {
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

impl Rem6BranchTraceStat {
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

impl ExecTraceStatSummary {
    fn add_record(&mut self, record: &Rem6ExecTraceRecord) {
        self.records = self.records.saturating_add(1);
        if record.retired {
            self.retired = self.retired.saturating_add(1);
        }
        self.bytes = self.bytes.saturating_add(record.bytes.len() as u64);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6ExecTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("retired", "Count", self.retired),
            ("bytes", "Byte", self.bytes),
        ] {
            stats.push(Rem6ExecTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

impl FetchTraceStatSummary {
    fn add_record(&mut self, record: &Rem6FetchTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(record.size);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6FetchTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("bytes", "Byte", self.bytes),
        ] {
            stats.push(Rem6FetchTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

impl DataTraceStatSummary {
    fn add_record(&mut self, record: &Rem6DataTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.bytes = self.bytes.saturating_add(record.size);
        match record.kind {
            "load" => {
                self.loads = self.loads.saturating_add(1);
                self.load_bytes = self.load_bytes.saturating_add(record.size);
            }
            "store" => {
                self.stores = self.stores.saturating_add(1);
                self.store_bytes = self.store_bytes.saturating_add(record.size);
            }
            "atomic" => {
                self.atomics = self.atomics.saturating_add(1);
                self.atomic_bytes = self.atomic_bytes.saturating_add(record.size);
            }
            other => unreachable!("unexpected data trace kind {other}"),
        }
    }

    fn push_stats(&self, stats: &mut Vec<Rem6DataTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("records", "Count", self.records),
            ("loads", "Count", self.loads),
            ("stores", "Count", self.stores),
            ("atomics", "Count", self.atomics),
            ("bytes", "Byte", self.bytes),
            ("load_bytes", "Byte", self.load_bytes),
            ("store_bytes", "Byte", self.store_bytes),
            ("atomic_bytes", "Byte", self.atomic_bytes),
        ] {
            stats.push(Rem6DataTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

impl BranchTraceStatSummary {
    fn add_record(&mut self, record: &Rem6BranchTraceRecord) {
        self.records = self.records.saturating_add(1);
        if record.conditional {
            self.conditional = self.conditional.saturating_add(1);
        } else {
            self.unconditional = self.unconditional.saturating_add(1);
        }
        if record.predicted_taken {
            self.predicted_taken = self.predicted_taken.saturating_add(1);
        }
        if record.resolved_taken {
            self.resolved_taken = self.resolved_taken.saturating_add(1);
        }
        if record.mispredicted {
            self.mispredictions = self.mispredictions.saturating_add(1);
        }
        if record.repair_target_pc.is_some() {
            self.repairs = self.repairs.saturating_add(1);
        }
        self.flushed = self
            .flushed
            .saturating_add(record.flushed_sequences.len() as u64);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6BranchTraceStat>, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("conditional", self.conditional),
            ("unconditional", self.unconditional),
            ("predicted_taken", self.predicted_taken),
            ("resolved_taken", self.resolved_taken),
            ("mispredictions", self.mispredictions),
            ("repairs", self.repairs),
            ("flushed", self.flushed),
        ] {
            stats.push(Rem6BranchTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit: "Count",
                value,
            });
        }
    }
}

pub(super) fn exec_trace_stats(records: &[Rem6ExecTraceRecord]) -> Vec<Rem6ExecTraceStat> {
    let mut cpus = BTreeMap::<u32, ExecTraceStatSummary>::new();
    let mut retirement = BTreeMap::<&str, ExecTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        retirement
            .entry(exec_retirement_path(record.retired))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (retirement, summary) in retirement {
        summary.push_stats(&mut stats, &format!("retirement.{retirement}"));
    }
    stats
}

pub(super) fn fetch_trace_stats(
    records: &[Rem6FetchTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6FetchTraceStat> {
    let mut cpus = BTreeMap::<u32, FetchTraceStatSummary>::new();
    let mut endpoints = BTreeMap::<&str, FetchTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        endpoints
            .entry(record.endpoint.as_str())
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (endpoint, summary) in endpoints {
        summary.push_stats(
            &mut stats,
            &format!("endpoint.{}", stat_path_segment(endpoint)),
        );
    }
    stats
}

pub(super) fn data_trace_stats(
    records: &[Rem6DataTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6DataTraceStat> {
    let mut cpus = BTreeMap::<u32, DataTraceStatSummary>::new();
    let mut kinds = BTreeMap::<&str, DataTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        kinds.entry(record.kind).or_default().add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (kind, summary) in kinds {
        summary.push_stats(&mut stats, &format!("kind.{}", stat_path_segment(kind)));
    }
    stats
}

pub(super) fn branch_trace_stats(
    records: &[Rem6BranchTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6BranchTraceStat> {
    let mut cpus = BTreeMap::<u32, BranchTraceStatSummary>::new();
    let mut kinds = BTreeMap::<&str, BranchTraceStatSummary>::new();
    let mut outcomes = BTreeMap::<&str, BranchTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        kinds.entry(record.kind()).or_default().add_record(record);
        outcomes
            .entry(branch_outcome_path(record.mispredicted))
            .or_default()
            .add_record(record);
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (kind, summary) in kinds {
        summary.push_stats(&mut stats, &format!("kind.{}", stat_path_segment(kind)));
    }
    for (outcome, summary) in outcomes {
        summary.push_stats(
            &mut stats,
            &format!("outcome.{}", stat_path_segment(outcome)),
        );
    }
    stats
}

const fn exec_retirement_path(retired: bool) -> &'static str {
    match retired {
        true => "retired",
        false => "not_retired",
    }
}

const fn branch_outcome_path(mispredicted: bool) -> &'static str {
    match mispredicted {
        true => "mispredicted",
        false => "correct",
    }
}
