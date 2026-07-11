use rem6_cpu::{InOrderPipelineRedirectCause, InOrderPipelineStage, RiscvCluster};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6BranchTraceRecord {
    pub(super) cpu: u32,
    cycle: u64,
    sequence: u64,
    resolved_stage: &'static str,
    pc: u64,
    pub(super) conditional: bool,
    pub(super) predicted_taken: bool,
    predicted_target_pc: Option<u64>,
    pub(super) resolved_taken: bool,
    resolved_target_pc: Option<u64>,
    pub(super) mispredicted: bool,
    pub(super) repair_target_pc: Option<u64>,
    pub(super) flushed_sequences: Vec<u64>,
}

pub(super) fn branch_trace_records(
    cluster: &RiscvCluster,
    core_count: u32,
) -> Vec<Rem6BranchTraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = rem6_cpu::CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        for cycle in core.in_order_pipeline_cycle_records() {
            records.extend(cycle.branch_predictions().iter().map(|prediction| {
                let flushed = cycle
                    .plan()
                    .redirect()
                    .filter(|redirect| {
                        redirect.cause() == InOrderPipelineRedirectCause::BranchPrediction
                            && redirect.sequence() == prediction.sequence()
                    })
                    .map_or(&[] as &[_], |_| cycle.plan().flushed());
                Rem6BranchTraceRecord {
                    cpu: cpu.get(),
                    cycle: cycle.cycle(),
                    sequence: prediction.sequence(),
                    resolved_stage: in_order_pipeline_stage_name(prediction.resolved_stage()),
                    pc: prediction.fetch_pc(),
                    conditional: prediction.is_conditional(),
                    predicted_taken: prediction.predicted_taken(),
                    predicted_target_pc: prediction.predicted_target_pc(),
                    resolved_taken: prediction.resolved_taken(),
                    resolved_target_pc: prediction.resolved_target_pc(),
                    mispredicted: prediction.mispredicted(),
                    repair_target_pc: prediction.repair_target_pc(),
                    flushed_sequences: flushed
                        .iter()
                        .map(|instruction| instruction.sequence())
                        .collect(),
                }
            }));
        }
    }
    records.sort_by_key(|record| (record.cycle, record.cpu, record.sequence, record.pc));
    records
}

impl Rem6BranchTraceRecord {
    pub(super) fn kind(&self) -> &'static str {
        match self.conditional {
            true => "conditional",
            false => "unconditional",
        }
    }

    pub(super) fn to_json(&self) -> String {
        let flushed_sequences = self
            .flushed_sequences
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"cpu\":{},\"cycle\":{},\"sequence\":{},\"resolved_stage\":\"{}\",\"pc\":\"0x{:x}\",\"kind\":\"{}\",\"conditional\":{},\"predicted_taken\":{},\"predicted_target\":{},\"resolved_taken\":{},\"resolved_target\":{},\"mispredicted\":{},\"repair_target\":{},\"flushed_count\":{},\"flushed_sequences\":[{}]}}",
            self.cpu,
            self.cycle,
            self.sequence,
            self.resolved_stage,
            self.pc,
            self.kind(),
            self.conditional,
            self.predicted_taken,
            optional_hex_json(self.predicted_target_pc),
            self.resolved_taken,
            optional_hex_json(self.resolved_target_pc),
            self.mispredicted,
            optional_hex_json(self.repair_target_pc),
            self.flushed_sequences.len(),
            flushed_sequences,
        )
    }
}

fn optional_hex_json(value: Option<u64>) -> String {
    value
        .map(|value| format!("\"0x{value:x}\""))
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
