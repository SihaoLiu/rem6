use rem6_cpu::{
    InOrderPipelineAdvance, InOrderPipelineInstruction, InOrderPipelineStage, RiscvCluster,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6PipelineTraceRecord {
    pub(super) cpu: u32,
    pub(super) cycle: u64,
    pub(super) stall_cycles: u64,
    pub(super) stall_cause: Option<&'static str>,
    pub(super) flush_cause: Option<&'static str>,
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
                    Rem6PipelineTraceRecord {
                        cpu: cpu.get(),
                        cycle: cycle.cycle(),
                        stall_cycles: cycle.stall_cycle_count(),
                        stall_cause: cycle.stall_cause().map(|cause| cause.as_str()),
                        flush_cause: pipeline_flush_cause(branch_prediction_flushed),
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
                        branch_predictions: summary.branch_prediction_count() as u64,
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
            "{{\"cpu\":{},\"cycle\":{},\"stall_cycles\":{},\"stall_cause\":{},\"flush_cause\":{},\"state_changed\":{},\"before_in_flight\":[{}],\"after_in_flight\":[{}],\"advanced\":[{}],\"resource_blocked\":[{}],\"ordering_blocked\":[{}],\"flushed\":[{}],\"branch_predictions\":{},\"branch_mispredictions\":{},\"branch_prediction_flushed\":{},\"redirect_target\":{}}}",
            self.cpu,
            self.cycle,
            self.stall_cycles,
            optional_str_json(self.stall_cause),
            optional_str_json(self.flush_cause),
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

const fn pipeline_flush_cause(branch_prediction_flushed: u64) -> Option<&'static str> {
    match branch_prediction_flushed {
        0 => None,
        _ => Some("branch_prediction"),
    }
}

impl Rem6PipelineTraceInstruction {
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
