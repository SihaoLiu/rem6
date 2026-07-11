use std::collections::BTreeSet;

use rem6_cpu::{
    InOrderPipelineAdvance, InOrderPipelineInstruction, InOrderPipelineRedirectCause,
    InOrderPipelineStage, RiscvCluster,
};

mod correlation;

pub(super) use correlation::PipelineStallBacklogFlushSummary;

const PIPELINE_STAGE_NAMES: [&str; 5] = ["fetch1", "fetch2", "decode", "execute", "commit"];
const PIPELINE_STALL_CAUSES: [&str; 3] = ["fetch_wait", "data_wait", "execute_wait"];
const PIPELINE_REDIRECT_CAUSES: [&str; 3] =
    ["branch_prediction", "trap_redirect", "interrupt_redirect"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6PipelineTraceRecord {
    pub(super) cpu: u32,
    pub(super) cycle: u64,
    pub(super) stall_cycles: u64,
    pub(super) stall_cause: Option<&'static str>,
    pub(super) flush_cause: Option<&'static str>,
    pub(super) redirect_cause: Option<&'static str>,
    pub(super) state_changed: bool,
    pub(super) before_in_flight: Vec<Rem6PipelineTraceInstruction>,
    pub(super) after_in_flight: Vec<Rem6PipelineTraceInstruction>,
    pub(super) advanced: Vec<Rem6PipelineTraceAdvance>,
    pub(super) resource_blocked: Vec<Rem6PipelineTraceInstruction>,
    pub(super) ordering_blocked: Vec<Rem6PipelineTraceInstruction>,
    pub(super) flushed: Vec<Rem6PipelineTraceInstruction>,
    pub(super) branch_predictions: u64,
    pub(super) branch_mispredictions: u64,
    pub(super) branch_prediction_flushed: u64,
    pub(super) redirect_target_pc: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rem6PipelineTraceInstruction {
    sequence: u64,
    stage: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rem6PipelineTraceAdvance {
    sequence: u64,
    source_stage: &'static str,
    destination_stage: Option<&'static str>,
    pub(super) retires: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6PipelineTraceSummary {
    totals: Rem6PipelineTraceRecordTotals,
    stages: [Rem6PipelineTraceStageTotals; PIPELINE_STAGE_NAMES.len()],
    stall_causes: [Rem6PipelineTraceStallCauseTotals; PIPELINE_STALL_CAUSES.len()],
    flush_causes: [Rem6PipelineTraceFlushCauseTotals; PIPELINE_REDIRECT_CAUSES.len()],
    redirect_causes: [Rem6PipelineTraceFlushCauseTotals; PIPELINE_REDIRECT_CAUSES.len()],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6PipelineTraceRecordTotals {
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
struct Rem6PipelineTraceStageTotals {
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
struct Rem6PipelineTraceResourceTotals {
    records: u64,
    resource_blocked: u64,
    resource_blocked_cycles: u64,
    ordering_blocked: u64,
    ordering_blocked_cycles: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6PipelineTraceFlushTotals {
    records: u64,
    flushed: u64,
    flushed_cycles: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6PipelineTraceStallCauseTotals {
    totals: Rem6PipelineTraceRecordTotals,
    stages: [Rem6PipelineTraceResourceTotals; PIPELINE_STAGE_NAMES.len()],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6PipelineTraceFlushCauseTotals {
    totals: Rem6PipelineTraceRecordTotals,
    stages: [Rem6PipelineTraceFlushTotals; PIPELINE_STAGE_NAMES.len()],
}

pub(super) fn pipeline_trace_records(
    cluster: &RiscvCluster,
    core_count: u32,
) -> Vec<Rem6PipelineTraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = rem6_cpu::CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        records.extend(
            core.in_order_pipeline_cycle_records()
                .into_iter()
                .map(|cycle| {
                    let summary = cycle.summary();
                    let branch_prediction_flushed =
                        summary.branch_prediction_flushed_count() as u64;
                    let branch_predictions = summary.branch_prediction_count() as u64;
                    Rem6PipelineTraceRecord {
                        cpu: cpu.get(),
                        cycle: cycle.cycle(),
                        stall_cycles: cycle.stall_cycle_count(),
                        stall_cause: cycle.stall_cause().map(|cause| cause.as_str()),
                        flush_cause: pipeline_redirect_cause(summary.flush_cause()),
                        redirect_cause: pipeline_redirect_cause(summary.redirect_cause()),
                        state_changed: summary.state_changed(),
                        before_in_flight: cycle
                            .before()
                            .in_flight()
                            .iter()
                            .copied()
                            .map(pipeline_trace_instruction)
                            .collect(),
                        after_in_flight: cycle
                            .after()
                            .in_flight()
                            .iter()
                            .copied()
                            .map(pipeline_trace_instruction)
                            .collect(),
                        advanced: cycle
                            .plan()
                            .advanced()
                            .iter()
                            .copied()
                            .map(pipeline_trace_advance)
                            .collect(),
                        resource_blocked: cycle
                            .plan()
                            .resource_blocked()
                            .iter()
                            .copied()
                            .map(pipeline_trace_instruction)
                            .collect(),
                        ordering_blocked: cycle
                            .plan()
                            .ordering_blocked()
                            .iter()
                            .copied()
                            .map(pipeline_trace_instruction)
                            .collect(),
                        flushed: cycle
                            .plan()
                            .flushed()
                            .iter()
                            .copied()
                            .map(pipeline_trace_instruction)
                            .collect(),
                        branch_predictions,
                        branch_mispredictions: summary.branch_misprediction_count() as u64,
                        branch_prediction_flushed,
                        redirect_target_pc: summary.redirect_target_pc(),
                    }
                }),
        );
    }
    records.sort_by_key(|record| (record.cycle, record.cpu));
    records
}

impl Rem6PipelineTraceRecord {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"cycle\":{},\"stall_cycles\":{},\"stall_cause\":{},\"flush_cause\":{},\"redirect_cause\":{},\"state_changed\":{},\"before_in_flight\":[{}],\"after_in_flight\":[{}],\"advanced\":[{}],\"resource_blocked\":[{}],\"ordering_blocked\":[{}],\"flushed\":[{}],\"branch_predictions\":{},\"branch_mispredictions\":{},\"branch_prediction_flushed\":{},\"redirect_target\":{}}}",
            self.cpu,
            self.cycle,
            self.stall_cycles,
            optional_str_json(self.stall_cause),
            optional_str_json(self.flush_cause),
            optional_str_json(self.redirect_cause),
            self.state_changed,
            pipeline_trace_instructions_json(&self.before_in_flight),
            pipeline_trace_instructions_json(&self.after_in_flight),
            pipeline_trace_advances_json(&self.advanced),
            pipeline_trace_instructions_json(&self.resource_blocked),
            pipeline_trace_instructions_json(&self.ordering_blocked),
            pipeline_trace_instructions_json(&self.flushed),
            self.branch_predictions,
            self.branch_mispredictions,
            self.branch_prediction_flushed,
            optional_hex_json(self.redirect_target_pc),
        )
    }
}

pub(super) fn pipeline_trace_summary_to_json(records: &[Rem6PipelineTraceRecord]) -> String {
    let mut summary = Rem6PipelineTraceSummary::default();
    for record in records {
        summary.add_record(record);
    }
    let stall_backlog_flush = PipelineStallBacklogFlushSummary::from_records(records);
    summary.to_json(&stall_backlog_flush.to_json())
}

impl Rem6PipelineTraceSummary {
    fn add_record(&mut self, record: &Rem6PipelineTraceRecord) {
        self.totals.add_record(record);

        let before_in_flight = pipeline_stage_instruction_counts(&record.before_in_flight);
        let after_in_flight = pipeline_stage_instruction_counts(&record.after_in_flight);
        let (advanced, retired) = pipeline_stage_advance_counts(&record.advanced);
        let resource_blocked = pipeline_stage_instruction_counts(&record.resource_blocked);
        let ordering_blocked = pipeline_stage_instruction_counts(&record.ordering_blocked);
        let flushed = pipeline_stage_instruction_counts(&record.flushed);

        for index in 0..PIPELINE_STAGE_NAMES.len() {
            if before_in_flight[index] > 0 {
                self.stages[index].before_in_flight = self.stages[index]
                    .before_in_flight
                    .saturating_add(before_in_flight[index]);
                self.stages[index].before_in_flight_cycles =
                    self.stages[index].before_in_flight_cycles.saturating_add(1);
            }
            if after_in_flight[index] > 0 {
                self.stages[index].after_in_flight = self.stages[index]
                    .after_in_flight
                    .saturating_add(after_in_flight[index]);
                self.stages[index].after_in_flight_cycles =
                    self.stages[index].after_in_flight_cycles.saturating_add(1);
            }
            if advanced[index] > 0 {
                self.stages[index].advanced =
                    self.stages[index].advanced.saturating_add(advanced[index]);
                self.stages[index].advanced_cycles =
                    self.stages[index].advanced_cycles.saturating_add(1);
            }
            if retired[index] > 0 {
                self.stages[index].retired =
                    self.stages[index].retired.saturating_add(retired[index]);
                self.stages[index].retired_cycles =
                    self.stages[index].retired_cycles.saturating_add(1);
            }
            if resource_blocked[index] > 0 {
                self.stages[index].resource_blocked = self.stages[index]
                    .resource_blocked
                    .saturating_add(resource_blocked[index]);
                self.stages[index].resource_blocked_cycles =
                    self.stages[index].resource_blocked_cycles.saturating_add(1);
            }
            if ordering_blocked[index] > 0 {
                self.stages[index].ordering_blocked = self.stages[index]
                    .ordering_blocked
                    .saturating_add(ordering_blocked[index]);
                self.stages[index].ordering_blocked_cycles =
                    self.stages[index].ordering_blocked_cycles.saturating_add(1);
            }
            if flushed[index] > 0 {
                self.stages[index].flushed =
                    self.stages[index].flushed.saturating_add(flushed[index]);
                self.stages[index].flushed_cycles =
                    self.stages[index].flushed_cycles.saturating_add(1);
            }
        }

        match record.flush_cause {
            Some("branch_prediction") => {
                for (index, flushed) in flushed.iter().copied().enumerate() {
                    if flushed > 0 {
                        self.stages[index].add_branch_prediction_flushed(flushed, 1);
                    }
                }
            }
            Some("trap_redirect") => {
                for (index, flushed) in flushed.iter().copied().enumerate() {
                    if flushed > 0 {
                        self.stages[index].add_trap_redirect_flushed(flushed, 1);
                    }
                }
            }
            Some("interrupt_redirect") => {
                for (index, flushed) in flushed.iter().copied().enumerate() {
                    if flushed > 0 {
                        self.stages[index].add_interrupt_redirect_flushed(flushed, 1);
                    }
                }
            }
            Some(_) | None => {}
        }

        if let Some(index) = record.stall_cause.and_then(pipeline_stall_cause_index) {
            self.stall_causes[index].totals.add_record(record);
            let mut stall_stages = BTreeSet::new();
            for stage in 0..PIPELINE_STAGE_NAMES.len() {
                if resource_blocked[stage] > 0 || ordering_blocked[stage] > 0 {
                    stall_stages.insert(stage);
                }
            }
            for stage in stall_stages {
                self.stall_causes[index].stages[stage].add_record(
                    resource_blocked[stage],
                    u64::from(resource_blocked[stage] > 0).saturating_mul(record.stall_cycles),
                    ordering_blocked[stage],
                    u64::from(ordering_blocked[stage] > 0).saturating_mul(record.stall_cycles),
                );
            }
        }

        if let Some(index) = record.flush_cause.and_then(pipeline_redirect_cause_index) {
            self.flush_causes[index].totals.add_record(record);
            for stage in 0..PIPELINE_STAGE_NAMES.len() {
                if flushed[stage] > 0 {
                    self.flush_causes[index].stages[stage].add_record(flushed[stage], 1);
                }
            }
        }
        if let Some(index) = record
            .redirect_cause
            .and_then(pipeline_redirect_cause_index)
        {
            self.redirect_causes[index].totals.add_record(record);
            for stage in 0..PIPELINE_STAGE_NAMES.len() {
                if flushed[stage] > 0 {
                    self.redirect_causes[index].stages[stage].add_record(flushed[stage], 1);
                }
            }
        }
    }

    fn to_json(self, stall_backlog_flush: &str) -> String {
        format!(
            "{{{},\"stage\":{},\"stall_cause\":{},\"flush_cause\":{},\"redirect_cause\":{},\"stall_backlog_flush\":{}}}",
            self.totals.json_fields(),
            pipeline_stage_totals_json(&self.stages),
            pipeline_stall_cause_totals_json(&self.stall_causes),
            pipeline_flush_cause_totals_json(&self.flush_causes),
            pipeline_flush_cause_totals_json(&self.redirect_causes),
            stall_backlog_flush,
        )
    }
}

impl Rem6PipelineTraceRecordTotals {
    fn add_record(&mut self, record: &Rem6PipelineTraceRecord) {
        self.records = self.records.saturating_add(1);
        self.stall_cycles = self.stall_cycles.saturating_add(record.stall_cycles);
        self.state_changed = self
            .state_changed
            .saturating_add(u64::from(record.state_changed));
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
        self.redirects = self
            .redirects
            .saturating_add(u64::from(record.redirect_target_pc.is_some()));
        self.before_in_flight = self
            .before_in_flight
            .saturating_add(record.before_in_flight.len() as u64);
        self.after_in_flight = self
            .after_in_flight
            .saturating_add(record.after_in_flight.len() as u64);
    }

    fn json_fields(self) -> String {
        format!(
            "\"records\":{},\"stall_cycles\":{},\"state_changed\":{},\"advanced\":{},\"retired\":{},\"flushed\":{},\"resource_blocked\":{},\"ordering_blocked\":{},\"branch_predictions\":{},\"branch_mispredictions\":{},\"branch_prediction_flushed\":{},\"redirects\":{},\"before_in_flight\":{},\"after_in_flight\":{}",
            self.records,
            self.stall_cycles,
            self.state_changed,
            self.advanced,
            self.retired,
            self.flushed,
            self.resource_blocked,
            self.ordering_blocked,
            self.branch_predictions,
            self.branch_mispredictions,
            self.branch_prediction_flushed,
            self.redirects,
            self.before_in_flight,
            self.after_in_flight,
        )
    }
}

impl Rem6PipelineTraceStageTotals {
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

    fn to_json(self) -> String {
        format!(
            "{{\"before_in_flight\":{},\"before_in_flight_cycles\":{},\"after_in_flight\":{},\"after_in_flight_cycles\":{},\"advanced\":{},\"advanced_cycles\":{},\"retired\":{},\"retired_cycles\":{},\"flushed\":{},\"flushed_cycles\":{},\"branch_prediction_flushed\":{},\"branch_prediction_flushed_cycles\":{},\"trap_redirect_flushed\":{},\"trap_redirect_flushed_cycles\":{},\"interrupt_redirect_flushed\":{},\"interrupt_redirect_flushed_cycles\":{},\"resource_blocked\":{},\"resource_blocked_cycles\":{},\"ordering_blocked\":{},\"ordering_blocked_cycles\":{}}}",
            self.before_in_flight,
            self.before_in_flight_cycles,
            self.after_in_flight,
            self.after_in_flight_cycles,
            self.advanced,
            self.advanced_cycles,
            self.retired,
            self.retired_cycles,
            self.flushed,
            self.flushed_cycles,
            self.branch_prediction_flushed,
            self.branch_prediction_flushed_cycles,
            self.trap_redirect_flushed,
            self.trap_redirect_flushed_cycles,
            self.interrupt_redirect_flushed,
            self.interrupt_redirect_flushed_cycles,
            self.resource_blocked,
            self.resource_blocked_cycles,
            self.ordering_blocked,
            self.ordering_blocked_cycles,
        )
    }
}

impl Rem6PipelineTraceResourceTotals {
    fn add_record(
        &mut self,
        resource_blocked: u64,
        resource_blocked_cycles: u64,
        ordering_blocked: u64,
        ordering_blocked_cycles: u64,
    ) {
        self.records = self.records.saturating_add(1);
        self.resource_blocked = self.resource_blocked.saturating_add(resource_blocked);
        self.resource_blocked_cycles = self
            .resource_blocked_cycles
            .saturating_add(resource_blocked_cycles);
        self.ordering_blocked = self.ordering_blocked.saturating_add(ordering_blocked);
        self.ordering_blocked_cycles = self
            .ordering_blocked_cycles
            .saturating_add(ordering_blocked_cycles);
    }

    fn to_json(self) -> String {
        format!(
            "{{\"records\":{},\"resource_blocked\":{},\"resource_blocked_cycles\":{},\"ordering_blocked\":{},\"ordering_blocked_cycles\":{}}}",
            self.records,
            self.resource_blocked,
            self.resource_blocked_cycles,
            self.ordering_blocked,
            self.ordering_blocked_cycles,
        )
    }
}

impl Rem6PipelineTraceFlushTotals {
    fn add_record(&mut self, flushed: u64, flushed_cycles: u64) {
        self.records = self.records.saturating_add(1);
        self.flushed = self.flushed.saturating_add(flushed);
        self.flushed_cycles = self.flushed_cycles.saturating_add(flushed_cycles);
    }

    fn to_json(self) -> String {
        format!(
            "{{\"records\":{},\"flushed\":{},\"flushed_cycles\":{}}}",
            self.records, self.flushed, self.flushed_cycles
        )
    }
}

impl Rem6PipelineTraceStallCauseTotals {
    fn to_json(self) -> String {
        format!(
            "{{{},\"stage\":{}}}",
            self.totals.json_fields(),
            pipeline_resource_stage_totals_json(&self.stages),
        )
    }
}

impl Rem6PipelineTraceFlushCauseTotals {
    fn to_json(self) -> String {
        format!(
            "{{{},\"stage\":{}}}",
            self.totals.json_fields(),
            pipeline_flush_stage_totals_json(&self.stages),
        )
    }
}

const fn pipeline_redirect_cause(
    cause: Option<InOrderPipelineRedirectCause>,
) -> Option<&'static str> {
    match cause {
        None => None,
        Some(cause) => Some(cause.as_str()),
    }
}

impl Rem6PipelineTraceInstruction {
    pub(super) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(super) const fn stage(self) -> &'static str {
        self.stage
    }

    fn to_json(self) -> String {
        format!(
            "{{\"sequence\":{},\"stage\":\"{}\"}}",
            self.sequence, self.stage
        )
    }
}

impl Rem6PipelineTraceAdvance {
    pub(super) const fn source_stage(self) -> &'static str {
        self.source_stage
    }

    fn to_json(self) -> String {
        let destination_stage = self
            .destination_stage
            .map(|stage| format!("\"{stage}\""))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"sequence\":{},\"source_stage\":\"{}\",\"destination_stage\":{},\"retires\":{}}}",
            self.sequence, self.source_stage, destination_stage, self.retires
        )
    }
}

fn pipeline_trace_instruction(
    instruction: InOrderPipelineInstruction,
) -> Rem6PipelineTraceInstruction {
    Rem6PipelineTraceInstruction {
        sequence: instruction.sequence(),
        stage: in_order_pipeline_stage_name(instruction.stage()),
    }
}

fn pipeline_trace_advance(advance: InOrderPipelineAdvance) -> Rem6PipelineTraceAdvance {
    Rem6PipelineTraceAdvance {
        sequence: advance.sequence(),
        source_stage: in_order_pipeline_stage_name(advance.source_stage()),
        destination_stage: advance
            .destination_stage()
            .map(in_order_pipeline_stage_name),
        retires: advance.retires(),
    }
}

fn pipeline_trace_instructions_json(records: &[Rem6PipelineTraceInstruction]) -> String {
    records
        .iter()
        .copied()
        .map(Rem6PipelineTraceInstruction::to_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn pipeline_trace_advances_json(records: &[Rem6PipelineTraceAdvance]) -> String {
    records
        .iter()
        .copied()
        .map(Rem6PipelineTraceAdvance::to_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn pipeline_stage_instruction_counts(
    records: &[Rem6PipelineTraceInstruction],
) -> [u64; PIPELINE_STAGE_NAMES.len()] {
    let mut counts = [0u64; PIPELINE_STAGE_NAMES.len()];
    for record in records {
        if let Some(index) = pipeline_stage_index(record.stage()) {
            counts[index] = counts[index].saturating_add(1);
        }
    }
    counts
}

fn pipeline_stage_advance_counts(
    records: &[Rem6PipelineTraceAdvance],
) -> (
    [u64; PIPELINE_STAGE_NAMES.len()],
    [u64; PIPELINE_STAGE_NAMES.len()],
) {
    let mut advanced = [0u64; PIPELINE_STAGE_NAMES.len()];
    let mut retired = [0u64; PIPELINE_STAGE_NAMES.len()];
    for record in records {
        if let Some(index) = pipeline_stage_index(record.source_stage()) {
            advanced[index] = advanced[index].saturating_add(1);
            if record.retires {
                retired[index] = retired[index].saturating_add(1);
            }
        }
    }
    (advanced, retired)
}

fn pipeline_stage_index(stage: &str) -> Option<usize> {
    PIPELINE_STAGE_NAMES
        .iter()
        .position(|candidate| *candidate == stage)
}

fn pipeline_stall_cause_index(cause: &str) -> Option<usize> {
    PIPELINE_STALL_CAUSES
        .iter()
        .position(|candidate| *candidate == cause)
}

fn pipeline_redirect_cause_index(cause: &str) -> Option<usize> {
    PIPELINE_REDIRECT_CAUSES
        .iter()
        .position(|candidate| *candidate == cause)
}

fn pipeline_stage_totals_json(
    stages: &[Rem6PipelineTraceStageTotals; PIPELINE_STAGE_NAMES.len()],
) -> String {
    let fields = PIPELINE_STAGE_NAMES
        .iter()
        .enumerate()
        .map(|(index, stage)| format!("\"{stage}\":{}", stages[index].to_json()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn pipeline_resource_stage_totals_json(
    stages: &[Rem6PipelineTraceResourceTotals; PIPELINE_STAGE_NAMES.len()],
) -> String {
    let fields = PIPELINE_STAGE_NAMES
        .iter()
        .enumerate()
        .map(|(index, stage)| format!("\"{stage}\":{}", stages[index].to_json()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn pipeline_flush_stage_totals_json(
    stages: &[Rem6PipelineTraceFlushTotals; PIPELINE_STAGE_NAMES.len()],
) -> String {
    let fields = PIPELINE_STAGE_NAMES
        .iter()
        .enumerate()
        .map(|(index, stage)| format!("\"{stage}\":{}", stages[index].to_json()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn pipeline_stall_cause_totals_json(
    causes: &[Rem6PipelineTraceStallCauseTotals; PIPELINE_STALL_CAUSES.len()],
) -> String {
    let fields = PIPELINE_STALL_CAUSES
        .iter()
        .enumerate()
        .map(|(index, cause)| format!("\"{cause}\":{}", causes[index].to_json()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn pipeline_flush_cause_totals_json(
    causes: &[Rem6PipelineTraceFlushCauseTotals; PIPELINE_REDIRECT_CAUSES.len()],
) -> String {
    let fields = PIPELINE_REDIRECT_CAUSES
        .iter()
        .enumerate()
        .map(|(index, cause)| format!("\"{cause}\":{}", causes[index].to_json()))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn optional_hex_json(value: Option<u64>) -> String {
    value
        .map(|value| format!("\"0x{value:x}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_str_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_string())
}

const fn in_order_pipeline_stage_name(stage: InOrderPipelineStage) -> &'static str {
    match stage {
        InOrderPipelineStage::Fetch1 => "fetch1",
        InOrderPipelineStage::Fetch2 => "fetch2",
        InOrderPipelineStage::Decode => "decode",
        InOrderPipelineStage::Execute => "execute",
        InOrderPipelineStage::Commit => "commit",
    }
}
