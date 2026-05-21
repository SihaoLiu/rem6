use std::sync::{Arc, Mutex};

use rem6_coherence::{
    MesiCpuResponseRecord, MoesiCpuResponseRecord, ParallelCoherenceRunSummary,
    PartitionedMesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarness,
};
use rem6_dram::DramTargetActivity;
use rem6_memory::{MemoryRequest, MemoryResponse, ResponseStatus};
use rem6_transport::{RequestDelivery, TargetOutcome};

use super::RiscvTopologySystemError;

#[derive(Clone)]
pub(super) struct RiscvTopologyMesiDataCache {
    harness: Arc<Mutex<PartitionedMesiDirectoryLineHarness>>,
    runs: Arc<Mutex<Vec<ParallelCoherenceRunSummary>>>,
}

impl RiscvTopologyMesiDataCache {
    pub(super) fn new(harness: PartitionedMesiDirectoryLineHarness) -> Self {
        Self {
            harness: Arc::new(Mutex::new(harness)),
            runs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn harness(&self) -> Arc<Mutex<PartitionedMesiDirectoryLineHarness>> {
        Arc::clone(&self.harness)
    }

    pub(super) fn runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.runs.lock().expect("MESI data cache run lock").clone()
    }

    pub(super) fn mark_runs(&self) -> usize {
        self.runs.lock().expect("MESI data cache run lock").len()
    }

    fn runs_since(&self, marker: usize) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("MESI data cache run lock")
            .get(marker..)
            .unwrap_or_default()
            .to_vec()
    }

    fn record_run(&self, run: ParallelCoherenceRunSummary) {
        self.runs
            .lock()
            .expect("MESI data cache run lock")
            .push(run);
    }
}

#[derive(Clone)]
pub(super) struct RiscvTopologyMoesiDataCache {
    harness: Arc<Mutex<PartitionedMoesiDirectoryLineHarness>>,
    runs: Arc<Mutex<Vec<ParallelCoherenceRunSummary>>>,
}

impl RiscvTopologyMoesiDataCache {
    pub(super) fn new(harness: PartitionedMoesiDirectoryLineHarness) -> Self {
        Self {
            harness: Arc::new(Mutex::new(harness)),
            runs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn harness(&self) -> Arc<Mutex<PartitionedMoesiDirectoryLineHarness>> {
        Arc::clone(&self.harness)
    }

    pub(super) fn runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.runs.lock().expect("MOESI data cache run lock").clone()
    }

    pub(super) fn mark_runs(&self) -> usize {
        self.runs.lock().expect("MOESI data cache run lock").len()
    }

    fn runs_since(&self, marker: usize) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("MOESI data cache run lock")
            .get(marker..)
            .unwrap_or_default()
            .to_vec()
    }

    fn record_run(&self, run: ParallelCoherenceRunSummary) {
        self.runs
            .lock()
            .expect("MOESI data cache run lock")
            .push(run);
    }
}

pub(super) fn topology_mesi_data_response(
    cache: &RiscvTopologyMesiDataCache,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_mesi_data_response_result(cache, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_mesi_data_response_result(
    cache: &RiscvTopologyMesiDataCache,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MESI data cache lock");
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvTopologySystemError::MesiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvTopologySystemError::MesiDataCache)?;
    let response_record = harness
        .cpu_responses()
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == delivery.request().id())
        .cloned()
        .ok_or(RiscvTopologySystemError::MissingMesiDataResponse {
            request: delivery.request().id(),
        })?;
    drop(harness);
    cache.record_run(run);

    let response = mesi_response_record_to_memory_response(delivery.request(), &response_record)?;
    let delay = response_record.tick().saturating_sub(start_tick);
    if delay == 0 {
        Ok(TargetOutcome::Respond(response))
    } else {
        Ok(TargetOutcome::RespondAfter { delay, response })
    }
}

pub(super) fn topology_moesi_data_response(
    cache: &RiscvTopologyMoesiDataCache,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_moesi_data_response_result(cache, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_moesi_data_response_result(
    cache: &RiscvTopologyMoesiDataCache,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MOESI data cache lock");
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvTopologySystemError::MoesiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvTopologySystemError::MoesiDataCache)?;
    let response_record = harness
        .cpu_responses()
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == delivery.request().id())
        .cloned()
        .ok_or(RiscvTopologySystemError::MissingMoesiDataResponse {
            request: delivery.request().id(),
        })?;
    drop(harness);
    cache.record_run(run);

    let response = moesi_response_record_to_memory_response(delivery.request(), &response_record)?;
    let delay = response_record.tick().saturating_sub(start_tick);
    if delay == 0 {
        Ok(TargetOutcome::Respond(response))
    } else {
        Ok(TargetOutcome::RespondAfter { delay, response })
    }
}

fn mesi_response_record_to_memory_response(
    request: &MemoryRequest,
    record: &MesiCpuResponseRecord,
) -> Result<MemoryResponse, RiscvTopologySystemError> {
    match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(RiscvTopologySystemError::Memory)
        }
        ResponseStatus::Retry => Ok(MemoryResponse::retry(request)),
    }
}

fn moesi_response_record_to_memory_response(
    request: &MemoryRequest,
    record: &MoesiCpuResponseRecord,
) -> Result<MemoryResponse, RiscvTopologySystemError> {
    match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(RiscvTopologySystemError::Memory)
        }
        ResponseStatus::Retry => Ok(MemoryResponse::retry(request)),
    }
}

pub(super) fn merge_mesi_data_cache_activity(
    mut fabric_activity: Vec<rem6_fabric::FabricLaneActivity>,
    mut dram_activity: Vec<DramTargetActivity>,
    cache: Option<&RiscvTopologyMesiDataCache>,
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

pub(super) fn merge_moesi_data_cache_activity(
    mut fabric_activity: Vec<rem6_fabric::FabricLaneActivity>,
    mut dram_activity: Vec<DramTargetActivity>,
    cache: Option<&RiscvTopologyMoesiDataCache>,
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

fn record_coherence_error(
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    error: RiscvTopologySystemError,
) {
    let mut guard = memory_error.lock().expect("coherence data error lock");
    if guard.is_none() {
        *guard = Some(error);
    }
}
