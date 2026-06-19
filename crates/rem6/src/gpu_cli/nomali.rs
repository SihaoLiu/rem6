use std::path::{Path, PathBuf};

use crate::formatting::json_escape;

use super::Rem6GpuRunExecutionSummary;

const NOMALI_API_VERSION: u32 = 0;
const NOMALI_REGISTER_WINDOW_BYTES: u64 = 0x4000;
const NOMALI_GPU_TYPE: &str = "T760";
const NOMALI_GPU_INT: u32 = 0;
const NOMALI_JOB_INT: u32 = 1;
const NOMALI_MMU_INT: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GpuNoMaliAdapterArtifact {
    output: PathBuf,
    contents: String,
}

impl Rem6GpuNoMaliAdapterArtifact {
    pub(crate) fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn contents(&self) -> &str {
        &self.contents
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"rem6.nomali.gpu-adapter.v1\",\"artifact\":\"{}\"}}",
            json_escape(&self.output.display().to_string())
        )
    }
}

pub(crate) fn gpu_run_nomali_adapter_artifact(
    output: PathBuf,
    execution: &Rem6GpuRunExecutionSummary,
) -> Rem6GpuNoMaliAdapterArtifact {
    let global_memory_reads = execution
        .compute_unit_activity()
        .iter()
        .map(|activity| activity.global_memory_reads())
        .sum::<u64>();
    let global_memory_writes = execution
        .compute_unit_activity()
        .iter()
        .map(|activity| activity.global_memory_writes())
        .sum::<u64>();
    let compute_units = execution.compute_unit_activity().len();
    let compute_unit_activity = execution
        .compute_unit_activity()
        .iter()
        .map(nomali_compute_unit_activity_json)
        .collect::<Vec<_>>()
        .join(",");
    let contents = format!(
        "{{\"schema\":\"rem6.nomali.gpu-adapter.v1\",\"source_schema\":\"rem6.cli.gpu-run.v1\",\"scope\":\"gpu-run-execution-summary-adapter\",\"gpu\":{},\"interface\":{},\"execution\":{{\"status\":\"completed\",\"final_tick\":{},\"compute_units\":{},\"workgroup_completions\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"memory_read_callback_observations\":{},\"memory_write_callback_observations\":{},\"job_event_observations\":{},\"compute_unit_activity\":[{}]}}}}\n",
        nomali_gpu_json(),
        nomali_interface_json(),
        execution.final_tick(),
        compute_units,
        execution.workgroup_completions(),
        execution.coalesced_memory_accesses(),
        global_memory_reads,
        global_memory_writes,
        global_memory_reads,
        global_memory_writes,
        execution.workgroup_completions(),
        compute_unit_activity,
    );
    Rem6GpuNoMaliAdapterArtifact { output, contents }
}

fn nomali_gpu_json() -> String {
    format!(
        "{{\"type\":\"{}\",\"api_version\":{},\"version\":{{\"major\":0,\"minor\":0,\"status\":0}},\"register_window_bytes\":{},\"config_registers\":{}}}",
        NOMALI_GPU_TYPE,
        NOMALI_API_VERSION,
        NOMALI_REGISTER_WINDOW_BYTES,
        nomali_t760_config_registers_json(),
    )
}

fn nomali_interface_json() -> String {
    format!(
        "{{\"callbacks\":[\"interrupt\",\"memread\",\"memwrite\",\"reset\"],\"interrupts\":{{\"gpu\":{{\"nomali_int\":{}}},\"job\":{{\"nomali_int\":{}}},\"mmu\":{{\"nomali_int\":{}}}}}}}",
        NOMALI_GPU_INT, NOMALI_JOB_INT, NOMALI_MMU_INT,
    )
}

fn nomali_t760_config_registers_json() -> &'static str {
    "{\"gpu_id\":\"0x07500000\",\"l2_features\":\"0x07130206\",\"tiler_features\":\"0x00000809\",\"mem_features\":\"0x00000001\",\"mmu_features\":\"0x00002830\",\"as_present\":\"0x000000ff\",\"js_present\":\"0x00000007\",\"thread_max_threads\":\"0x00000100\",\"thread_max_workgroup_size\":\"0x00000100\",\"thread_max_barrier_size\":\"0x00000100\",\"thread_features\":\"0x0a040400\",\"texture_features\":[\"0x00fe001e\",\"0x0000ffff\",\"0x9f81ffff\"],\"js_features\":[\"0x0000020e\",\"0x000001fe\",\"0x0000007e\"],\"shader_present\":\"0x0000000f\",\"tiler_present\":\"0x00000001\",\"l2_present\":\"0x00000001\"}"
}

fn nomali_compute_unit_activity_json(activity: &super::Rem6GpuComputeUnitActivity) -> String {
    format!(
        "{{\"compute_unit\":{},\"workgroup_completions\":{},\"busy_cycles\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"first_started_at\":{},\"last_completed_at\":{}}}",
        activity.compute_unit(),
        activity.workgroup_completions(),
        activity.busy_cycles(),
        activity.coalesced_memory_accesses(),
        activity.global_memory_reads(),
        activity.global_memory_writes(),
        optional_tick_json(activity.first_started_at()),
        optional_tick_json(activity.last_completed_at()),
    )
}

fn optional_tick_json(tick: Option<u64>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "null".to_string())
}
