use std::sync::{Arc, Mutex};

use rem6_coherence::{ParallelCoherenceRunSummary, PartitionedDirectoryLineHarness};
use rem6_cpu::RiscvCluster;
use rem6_dram::DramTargetActivity;
use rem6_memory::{Address, MemoryResponse};
use rem6_transport::{RequestDelivery, TargetOutcome};

use super::coherence_data::topology_cache_response_result;
use super::RiscvTopologySystemError;

#[derive(Clone)]
pub(super) struct RiscvTopologyMsiInstructionCache {
    harness: Arc<Mutex<PartitionedDirectoryLineHarness>>,
    runs: Arc<Mutex<Vec<ParallelCoherenceRunSummary>>>,
}

impl RiscvTopologyMsiInstructionCache {
    pub(super) fn new(harness: PartitionedDirectoryLineHarness) -> Self {
        Self {
            harness: Arc::new(Mutex::new(harness)),
            runs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn harness(&self) -> Arc<Mutex<PartitionedDirectoryLineHarness>> {
        Arc::clone(&self.harness)
    }

    pub(super) fn runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("MSI instruction cache run lock")
            .clone()
    }

    pub(super) fn mark_runs(&self) -> usize {
        self.runs
            .lock()
            .expect("MSI instruction cache run lock")
            .len()
    }

    fn runs_since(&self, marker: usize) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("MSI instruction cache run lock")
            .get(marker..)
            .unwrap_or_default()
            .to_vec()
    }

    fn line_address(&self) -> Address {
        self.harness
            .lock()
            .expect("MSI instruction cache lock")
            .line()
            .address()
    }

    fn record_run(&self, run: ParallelCoherenceRunSummary) {
        self.runs
            .lock()
            .expect("MSI instruction cache run lock")
            .push(run);
    }
}

pub(super) fn topology_msi_instruction_cache_response(
    cache: Option<&RiscvTopologyMsiInstructionCache>,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> Option<TargetOutcome> {
    let cache = cache.filter(|cache| cache.line_address() == delivery.request().line_address())?;
    Some(topology_msi_instruction_response(
        cache,
        cluster,
        memory_error,
        delivery,
    ))
}

fn topology_msi_instruction_response(
    cache: &RiscvTopologyMsiInstructionCache,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_msi_instruction_response_result(cache, cluster, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_instruction_cache_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_msi_instruction_response_result(
    cache: &RiscvTopologyMsiInstructionCache,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MSI instruction cache lock");
    let (run, outcome) = topology_cache_response_result(
        &mut *harness,
        cluster,
        delivery,
        RiscvTopologySystemError::MsiInstructionCache,
        |request| RiscvTopologySystemError::MissingMsiInstructionResponse { request },
    )?;
    drop(harness);
    cache.record_run(run);
    Ok(outcome)
}

pub(super) fn merge_msi_instruction_cache_activity(
    mut fabric_activity: Vec<rem6_fabric::FabricLaneActivity>,
    mut dram_activity: Vec<DramTargetActivity>,
    cache: Option<&RiscvTopologyMsiInstructionCache>,
    marker: Option<usize>,
) -> (
    Vec<rem6_fabric::FabricLaneActivity>,
    Vec<DramTargetActivity>,
) {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return (fabric_activity, dram_activity);
    };

    for run in cache.runs_since(marker) {
        fabric_activity.extend(run.fabric_activities().into_values());
        dram_activity.extend(run.dram_target_activities().into_values());
    }

    (fabric_activity, dram_activity)
}

fn record_instruction_cache_error(
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    error: RiscvTopologySystemError,
) {
    let mut guard = memory_error
        .lock()
        .expect("MSI instruction cache error lock");
    if guard.is_none() {
        *guard = Some(error);
    }
}
