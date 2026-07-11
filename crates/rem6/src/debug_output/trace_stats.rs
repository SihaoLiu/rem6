use std::collections::{BTreeMap, BTreeSet};

use super::{
    branch::Rem6BranchTraceRecord,
    pipeline::{PipelineStallBacklogFlushSummary, Rem6PipelineTraceRecord},
    Rem6DataTraceRecord, Rem6ExecTraceRecord, Rem6FetchTraceRecord,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6PipelineTraceStat {
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineTraceStatSummary {
    records: u64,
    stall_cycles: u64,
    state_changed: u64,
    advanced: u64,
    retired: u64,
    flushed: u64,
    resource_blocked: u64,
    ordering_blocked: u64,
    branch_predictions: u64,
    branch_mispredictions: u64,
    branch_prediction_flushed: u64,
    redirects: u64,
    before_in_flight: u64,
    after_in_flight: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineStageTraceStatSummary {
    before_in_flight: u64,
    before_in_flight_cycles: u64,
    after_in_flight: u64,
    after_in_flight_cycles: u64,
    advanced: u64,
    advanced_cycles: u64,
    retired: u64,
    retired_cycles: u64,
    flushed: u64,
    flushed_cycles: u64,
    branch_prediction_flushed: u64,
    branch_prediction_flushed_cycles: u64,
    trap_redirect_flushed: u64,
    trap_redirect_flushed_cycles: u64,
    interrupt_redirect_flushed: u64,
    interrupt_redirect_flushed_cycles: u64,
    resource_blocked: u64,
    resource_blocked_cycles: u64,
    ordering_blocked: u64,
    ordering_blocked_cycles: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineStageResourceTraceStatSummary {
    resource_blocked: u64,
    resource_blocked_cycles: u64,
    ordering_blocked: u64,
    ordering_blocked_cycles: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineStageFlushTraceStatSummary {
    flushed: u64,
    flushed_cycles: u64,
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

impl Rem6PipelineTraceStat {
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

impl PipelineTraceStatSummary {
    fn add_record(&mut self, record: &Rem6PipelineTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.stall_cycles = self.stall_cycles.saturating_add(record.stall_cycles);
        if record.state_changed {
            self.state_changed = self.state_changed.saturating_add(1);
        }
        self.advanced = self.advanced.saturating_add(record.advanced.len() as u64);
        self.retired = self.retired.saturating_add(
            record
                .advanced
                .iter()
                .filter(|advance| advance.retires)
                .count() as u64,
        );
        self.flushed = self.flushed.saturating_add(record.flushed.len() as u64);
        self.resource_blocked = self
            .resource_blocked
            .saturating_add(record.resource_blocked.len() as u64);
        self.ordering_blocked = self
            .ordering_blocked
            .saturating_add(record.ordering_blocked.len() as u64);
        self.branch_predictions = self
            .branch_predictions
            .saturating_add(record.branch_predictions);
        self.branch_mispredictions = self
            .branch_mispredictions
            .saturating_add(record.branch_mispredictions);
        self.branch_prediction_flushed = self
            .branch_prediction_flushed
            .saturating_add(record.branch_prediction_flushed);
        if record.redirect_target_pc.is_some() {
            self.redirects = self.redirects.saturating_add(1);
        }
        self.before_in_flight = self
            .before_in_flight
            .saturating_add(record.before_in_flight.len() as u64);
        self.after_in_flight = self
            .after_in_flight
            .saturating_add(record.after_in_flight.len() as u64);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6PipelineTraceStat>, prefix: &str) {
        for (suffix, value) in [
            ("records", self.records),
            ("stall_cycles", self.stall_cycles),
            ("state_changed", self.state_changed),
            ("advanced", self.advanced),
            ("retired", self.retired),
            ("flushed", self.flushed),
            ("resource_blocked", self.resource_blocked),
            ("ordering_blocked", self.ordering_blocked),
            ("branch_predictions", self.branch_predictions),
            ("branch_mispredictions", self.branch_mispredictions),
            ("branch_prediction_flushed", self.branch_prediction_flushed),
            ("redirects", self.redirects),
            ("before_in_flight", self.before_in_flight),
            ("after_in_flight", self.after_in_flight),
        ] {
            stats.push(Rem6PipelineTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit: "Count",
                value,
            });
        }
    }
}

impl PipelineStageTraceStatSummary {
    fn add_before_in_flight(&mut self, before_in_flight: u64, before_in_flight_cycles: u64) {
        self.before_in_flight = self.before_in_flight.saturating_add(before_in_flight);
        self.before_in_flight_cycles = self
            .before_in_flight_cycles
            .saturating_add(before_in_flight_cycles);
    }

    fn add_after_in_flight(&mut self, after_in_flight: u64, after_in_flight_cycles: u64) {
        self.after_in_flight = self.after_in_flight.saturating_add(after_in_flight);
        self.after_in_flight_cycles = self
            .after_in_flight_cycles
            .saturating_add(after_in_flight_cycles);
    }

    fn add_advanced(
        &mut self,
        advanced: u64,
        advanced_cycles: u64,
        retired: u64,
        retired_cycles: u64,
    ) {
        self.advanced = self.advanced.saturating_add(advanced);
        self.advanced_cycles = self.advanced_cycles.saturating_add(advanced_cycles);
        self.retired = self.retired.saturating_add(retired);
        self.retired_cycles = self.retired_cycles.saturating_add(retired_cycles);
    }

    fn add_resource_blocked(&mut self, resource_blocked: u64, resource_blocked_cycles: u64) {
        self.resource_blocked = self.resource_blocked.saturating_add(resource_blocked);
        self.resource_blocked_cycles = self
            .resource_blocked_cycles
            .saturating_add(resource_blocked_cycles);
    }

    fn add_ordering_blocked(&mut self, ordering_blocked: u64, ordering_blocked_cycles: u64) {
        self.ordering_blocked = self.ordering_blocked.saturating_add(ordering_blocked);
        self.ordering_blocked_cycles = self
            .ordering_blocked_cycles
            .saturating_add(ordering_blocked_cycles);
    }

    fn add_flushed(&mut self, flushed: u64, flushed_cycles: u64) {
        self.flushed = self.flushed.saturating_add(flushed);
        self.flushed_cycles = self.flushed_cycles.saturating_add(flushed_cycles);
    }

    fn add_branch_prediction_flushed(&mut self, flushed: u64, flushed_cycles: u64) {
        self.branch_prediction_flushed = self.branch_prediction_flushed.saturating_add(flushed);
        self.branch_prediction_flushed_cycles = self
            .branch_prediction_flushed_cycles
            .saturating_add(flushed_cycles);
    }

    fn add_trap_redirect_flushed(&mut self, flushed: u64, flushed_cycles: u64) {
        self.trap_redirect_flushed = self.trap_redirect_flushed.saturating_add(flushed);
        self.trap_redirect_flushed_cycles = self
            .trap_redirect_flushed_cycles
            .saturating_add(flushed_cycles);
    }

    fn add_interrupt_redirect_flushed(&mut self, flushed: u64, flushed_cycles: u64) {
        self.interrupt_redirect_flushed = self.interrupt_redirect_flushed.saturating_add(flushed);
        self.interrupt_redirect_flushed_cycles = self
            .interrupt_redirect_flushed_cycles
            .saturating_add(flushed_cycles);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6PipelineTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("before_in_flight", "Count", self.before_in_flight),
            (
                "before_in_flight_cycles",
                "Cycle",
                self.before_in_flight_cycles,
            ),
            ("after_in_flight", "Count", self.after_in_flight),
            (
                "after_in_flight_cycles",
                "Cycle",
                self.after_in_flight_cycles,
            ),
            ("advanced", "Count", self.advanced),
            ("advanced_cycles", "Cycle", self.advanced_cycles),
            ("retired", "Count", self.retired),
            ("retired_cycles", "Cycle", self.retired_cycles),
            ("flushed", "Count", self.flushed),
            ("flushed_cycles", "Cycle", self.flushed_cycles),
            (
                "branch_prediction_flushed",
                "Count",
                self.branch_prediction_flushed,
            ),
            (
                "branch_prediction_flushed_cycles",
                "Cycle",
                self.branch_prediction_flushed_cycles,
            ),
            ("trap_redirect_flushed", "Count", self.trap_redirect_flushed),
            (
                "trap_redirect_flushed_cycles",
                "Cycle",
                self.trap_redirect_flushed_cycles,
            ),
            (
                "interrupt_redirect_flushed",
                "Count",
                self.interrupt_redirect_flushed,
            ),
            (
                "interrupt_redirect_flushed_cycles",
                "Cycle",
                self.interrupt_redirect_flushed_cycles,
            ),
            ("resource_blocked", "Count", self.resource_blocked),
            (
                "resource_blocked_cycles",
                "Cycle",
                self.resource_blocked_cycles,
            ),
            ("ordering_blocked", "Count", self.ordering_blocked),
            (
                "ordering_blocked_cycles",
                "Cycle",
                self.ordering_blocked_cycles,
            ),
        ] {
            stats.push(Rem6PipelineTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

impl PipelineStageResourceTraceStatSummary {
    fn add_record(
        &mut self,
        resource_blocked: u64,
        resource_blocked_cycles: u64,
        ordering_blocked: u64,
        ordering_blocked_cycles: u64,
    ) {
        self.resource_blocked = self.resource_blocked.saturating_add(resource_blocked);
        self.resource_blocked_cycles = self
            .resource_blocked_cycles
            .saturating_add(resource_blocked_cycles);
        self.ordering_blocked = self.ordering_blocked.saturating_add(ordering_blocked);
        self.ordering_blocked_cycles = self
            .ordering_blocked_cycles
            .saturating_add(ordering_blocked_cycles);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6PipelineTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("resource_blocked", "Count", self.resource_blocked),
            (
                "resource_blocked_cycles",
                "Cycle",
                self.resource_blocked_cycles,
            ),
            ("ordering_blocked", "Count", self.ordering_blocked),
            (
                "ordering_blocked_cycles",
                "Cycle",
                self.ordering_blocked_cycles,
            ),
        ] {
            stats.push(Rem6PipelineTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

impl PipelineStageFlushTraceStatSummary {
    fn add_record(&mut self, flushed: u64, flushed_cycles: u64) {
        self.flushed = self.flushed.saturating_add(flushed);
        self.flushed_cycles = self.flushed_cycles.saturating_add(flushed_cycles);
    }

    fn push_stats(&self, stats: &mut Vec<Rem6PipelineTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            ("flushed", "Count", self.flushed),
            ("flushed_cycles", "Cycle", self.flushed_cycles),
        ] {
            stats.push(Rem6PipelineTraceStat {
                path: format!("{prefix}.{suffix}"),
                unit,
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

pub(super) fn pipeline_trace_stats(
    records: &[Rem6PipelineTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6PipelineTraceStat> {
    let mut cpus = BTreeMap::<u32, PipelineTraceStatSummary>::new();
    let mut states = BTreeMap::<&str, PipelineTraceStatSummary>::new();
    let mut stages = BTreeMap::<String, PipelineStageTraceStatSummary>::new();
    let mut stall_causes = BTreeMap::<&str, PipelineTraceStatSummary>::new();
    let mut stall_cause_stages =
        BTreeMap::<(&str, String), PipelineStageResourceTraceStatSummary>::new();
    let mut flush_causes = BTreeMap::<&str, PipelineTraceStatSummary>::new();
    let mut flush_cause_stages =
        BTreeMap::<(&str, String), PipelineStageFlushTraceStatSummary>::new();
    let mut redirect_causes = BTreeMap::<&str, PipelineTraceStatSummary>::new();
    let mut redirect_cause_stages =
        BTreeMap::<(&str, String), PipelineStageFlushTraceStatSummary>::new();
    let mut cpu_stages = BTreeMap::<(u32, String), PipelineStageTraceStatSummary>::new();
    let mut cpu_stall_causes = BTreeMap::<(u32, &str), PipelineTraceStatSummary>::new();
    let mut cpu_stall_cause_stages =
        BTreeMap::<(u32, &str, String), PipelineStageResourceTraceStatSummary>::new();
    let mut cpu_flush_causes = BTreeMap::<(u32, &str), PipelineTraceStatSummary>::new();
    let mut cpu_flush_cause_stages =
        BTreeMap::<(u32, &str, String), PipelineStageFlushTraceStatSummary>::new();
    let mut cpu_redirect_causes = BTreeMap::<(u32, &str), PipelineTraceStatSummary>::new();
    let mut cpu_redirect_cause_stages =
        BTreeMap::<(u32, &str, String), PipelineStageFlushTraceStatSummary>::new();
    for record in records {
        cpus.entry(record.cpu).or_default().add_record(record);
        states
            .entry(pipeline_state_path(record.state_changed))
            .or_default()
            .add_record(record);
        let mut stage_before_in_flight = BTreeMap::<String, u64>::new();
        for instruction in &record.before_in_flight {
            *stage_before_in_flight
                .entry(stat_path_segment(instruction.stage()))
                .or_default() += 1;
        }
        for (stage, before_in_flight) in &stage_before_in_flight {
            stages
                .entry(stage.clone())
                .or_default()
                .add_before_in_flight(*before_in_flight, 1);
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_before_in_flight(*before_in_flight, 1);
        }
        let mut stage_after_in_flight = BTreeMap::<String, u64>::new();
        for instruction in &record.after_in_flight {
            *stage_after_in_flight
                .entry(stat_path_segment(instruction.stage()))
                .or_default() += 1;
        }
        for (stage, after_in_flight) in &stage_after_in_flight {
            stages
                .entry(stage.clone())
                .or_default()
                .add_after_in_flight(*after_in_flight, 1);
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_after_in_flight(*after_in_flight, 1);
        }
        let mut stage_advanced = BTreeMap::<String, (u64, u64)>::new();
        for advance in &record.advanced {
            let entry = stage_advanced
                .entry(stat_path_segment(advance.source_stage()))
                .or_default();
            entry.0 += 1;
            if advance.retires {
                entry.1 += 1;
            }
        }
        for (stage, (advanced, retired)) in &stage_advanced {
            stages.entry(stage.clone()).or_default().add_advanced(
                *advanced,
                1,
                *retired,
                u64::from(*retired > 0),
            );
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_advanced(*advanced, 1, *retired, u64::from(*retired > 0));
        }
        let mut stage_resource_blocked = BTreeMap::<String, u64>::new();
        for instruction in &record.resource_blocked {
            *stage_resource_blocked
                .entry(stat_path_segment(instruction.stage()))
                .or_default() += 1;
        }
        for (stage, resource_blocked) in &stage_resource_blocked {
            stages
                .entry(stage.clone())
                .or_default()
                .add_resource_blocked(*resource_blocked, 1);
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_resource_blocked(*resource_blocked, 1);
        }
        let mut stage_ordering_blocked = BTreeMap::<String, u64>::new();
        for instruction in &record.ordering_blocked {
            *stage_ordering_blocked
                .entry(stat_path_segment(instruction.stage()))
                .or_default() += 1;
        }
        for (stage, ordering_blocked) in &stage_ordering_blocked {
            stages
                .entry(stage.clone())
                .or_default()
                .add_ordering_blocked(*ordering_blocked, 1);
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_ordering_blocked(*ordering_blocked, 1);
        }
        let mut stage_flushed = BTreeMap::<String, u64>::new();
        for instruction in &record.flushed {
            *stage_flushed
                .entry(stat_path_segment(instruction.stage()))
                .or_default() += 1;
        }
        for (stage, flushed) in &stage_flushed {
            stages
                .entry(stage.clone())
                .or_default()
                .add_flushed(*flushed, 1);
            cpu_stages
                .entry((record.cpu, stage.clone()))
                .or_default()
                .add_flushed(*flushed, 1);
        }
        match record.flush_cause {
            Some("branch_prediction") => {
                for (stage, flushed) in &stage_flushed {
                    stages
                        .entry(stage.clone())
                        .or_default()
                        .add_branch_prediction_flushed(*flushed, 1);
                    cpu_stages
                        .entry((record.cpu, stage.clone()))
                        .or_default()
                        .add_branch_prediction_flushed(*flushed, 1);
                }
            }
            Some("trap_redirect") => {
                for (stage, flushed) in &stage_flushed {
                    stages
                        .entry(stage.clone())
                        .or_default()
                        .add_trap_redirect_flushed(*flushed, 1);
                    cpu_stages
                        .entry((record.cpu, stage.clone()))
                        .or_default()
                        .add_trap_redirect_flushed(*flushed, 1);
                }
            }
            Some("interrupt_redirect") => {
                for (stage, flushed) in &stage_flushed {
                    stages
                        .entry(stage.clone())
                        .or_default()
                        .add_interrupt_redirect_flushed(*flushed, 1);
                    cpu_stages
                        .entry((record.cpu, stage.clone()))
                        .or_default()
                        .add_interrupt_redirect_flushed(*flushed, 1);
                }
            }
            Some(_) | None => {}
        }
        if let Some(cause) = record.stall_cause {
            stall_causes.entry(cause).or_default().add_record(record);
            cpu_stall_causes
                .entry((record.cpu, cause))
                .or_default()
                .add_record(record);
            let stall_stages = stage_resource_blocked
                .keys()
                .chain(stage_ordering_blocked.keys())
                .cloned()
                .collect::<BTreeSet<_>>();
            for stage in stall_stages {
                stall_cause_stages
                    .entry((cause, stage.clone()))
                    .or_default()
                    .add_record(
                        stage_resource_blocked
                            .get(&stage)
                            .copied()
                            .unwrap_or_default(),
                        u64::from(stage_resource_blocked.contains_key(&stage))
                            .saturating_mul(record.stall_cycles),
                        stage_ordering_blocked
                            .get(&stage)
                            .copied()
                            .unwrap_or_default(),
                        u64::from(stage_ordering_blocked.contains_key(&stage))
                            .saturating_mul(record.stall_cycles),
                    );
                cpu_stall_cause_stages
                    .entry((record.cpu, cause, stage.clone()))
                    .or_default()
                    .add_record(
                        stage_resource_blocked
                            .get(&stage)
                            .copied()
                            .unwrap_or_default(),
                        u64::from(stage_resource_blocked.contains_key(&stage))
                            .saturating_mul(record.stall_cycles),
                        stage_ordering_blocked
                            .get(&stage)
                            .copied()
                            .unwrap_or_default(),
                        u64::from(stage_ordering_blocked.contains_key(&stage))
                            .saturating_mul(record.stall_cycles),
                    );
            }
        }
        if let Some(cause) = record.flush_cause {
            flush_causes.entry(cause).or_default().add_record(record);
            cpu_flush_causes
                .entry((record.cpu, cause))
                .or_default()
                .add_record(record);
            for (stage, flushed) in &stage_flushed {
                flush_cause_stages
                    .entry((cause, stage.clone()))
                    .or_default()
                    .add_record(*flushed, 1);
                cpu_flush_cause_stages
                    .entry((record.cpu, cause, stage.clone()))
                    .or_default()
                    .add_record(*flushed, 1);
            }
        }
        if let Some(cause) = record.redirect_cause {
            redirect_causes.entry(cause).or_default().add_record(record);
            cpu_redirect_causes
                .entry((record.cpu, cause))
                .or_default()
                .add_record(record);
            for (stage, flushed) in &stage_flushed {
                redirect_cause_stages
                    .entry((cause, stage.clone()))
                    .or_default()
                    .add_record(*flushed, 1);
                cpu_redirect_cause_stages
                    .entry((record.cpu, cause, stage.clone()))
                    .or_default()
                    .add_record(*flushed, 1);
            }
        }
    }

    let mut stats = Vec::new();
    for (cpu, summary) in cpus {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}"));
    }
    for (state, summary) in states {
        summary.push_stats(&mut stats, &format!("state.{}", stat_path_segment(state)));
    }
    for (stage, summary) in stages {
        summary.push_stats(&mut stats, &format!("stage.{stage}"));
    }
    for (cause, summary) in stall_causes {
        summary.push_stats(
            &mut stats,
            &format!("stall_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cause, stage), summary) in stall_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!("stall_cause.{}.stage.{stage}", stat_path_segment(cause)),
        );
    }
    for (cause, summary) in flush_causes {
        summary.push_stats(
            &mut stats,
            &format!("flush_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cause, stage), summary) in flush_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!("flush_cause.{}.stage.{stage}", stat_path_segment(cause)),
        );
    }
    for (cause, summary) in redirect_causes {
        summary.push_stats(
            &mut stats,
            &format!("redirect_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cause, stage), summary) in redirect_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!("redirect_cause.{}.stage.{stage}", stat_path_segment(cause)),
        );
    }
    for ((cpu, stage), summary) in cpu_stages {
        summary.push_stats(&mut stats, &format!("cpu.cpu{cpu}.stage.{stage}"));
    }
    for ((cpu, cause), summary) in cpu_stall_causes {
        summary.push_stats(
            &mut stats,
            &format!("cpu.cpu{cpu}.stall_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cpu, cause, stage), summary) in cpu_stall_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!(
                "cpu.cpu{cpu}.stall_cause.{}.stage.{stage}",
                stat_path_segment(cause)
            ),
        );
    }
    for ((cpu, cause), summary) in cpu_flush_causes {
        summary.push_stats(
            &mut stats,
            &format!("cpu.cpu{cpu}.flush_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cpu, cause, stage), summary) in cpu_flush_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!(
                "cpu.cpu{cpu}.flush_cause.{}.stage.{stage}",
                stat_path_segment(cause)
            ),
        );
    }
    for ((cpu, cause), summary) in cpu_redirect_causes {
        summary.push_stats(
            &mut stats,
            &format!("cpu.cpu{cpu}.redirect_cause.{}", stat_path_segment(cause)),
        );
    }
    for ((cpu, cause, stage), summary) in cpu_redirect_cause_stages {
        summary.push_stats(
            &mut stats,
            &format!(
                "cpu.cpu{cpu}.redirect_cause.{}.stage.{stage}",
                stat_path_segment(cause)
            ),
        );
    }
    for metric in PipelineStallBacklogFlushSummary::from_records(records).metrics() {
        stats.push(Rem6PipelineTraceStat {
            path: metric.path,
            unit: metric.unit,
            value: metric.value,
        });
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

const fn pipeline_state_path(state_changed: bool) -> &'static str {
    match state_changed {
        true => "changed",
        false => "unchanged",
    }
}
