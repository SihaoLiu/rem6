use std::sync::{Arc, Mutex};

use rem6_coherence::{
    CpuResponseRecord, MesiCpuResponseRecord, MoesiCpuResponseRecord, ParallelCoherenceRunSummary,
    PartitionedDirectoryLineHarness, PartitionedMesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarness,
};
use rem6_dram::DramTargetActivity;
use rem6_memory::{Address, MemoryRequest, MemoryResponse, ResponseStatus};
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::{RiscvDataCacheProtocol, RiscvDataCacheRunRecord};

use super::RiscvTopologySystemError;

#[derive(Clone)]
pub(super) struct RiscvTopologyMsiDataCache {
    harness: Arc<Mutex<PartitionedDirectoryLineHarness>>,
    runs: Arc<Mutex<Vec<ParallelCoherenceRunSummary>>>,
}

impl RiscvTopologyMsiDataCache {
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
        self.runs.lock().expect("MSI data cache run lock").clone()
    }

    pub(super) fn mark_runs(&self) -> usize {
        self.runs.lock().expect("MSI data cache run lock").len()
    }

    pub(super) fn line_address(&self) -> Address {
        self.harness
            .lock()
            .expect("MSI data cache lock")
            .line()
            .address()
    }

    fn runs_since(&self, marker: usize) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("MSI data cache run lock")
            .get(marker..)
            .unwrap_or_default()
            .to_vec()
    }

    fn record_run(&self, run: ParallelCoherenceRunSummary) {
        self.runs.lock().expect("MSI data cache run lock").push(run);
    }
}

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

    pub(super) fn line_address(&self) -> Address {
        self.harness
            .lock()
            .expect("MESI data cache lock")
            .line()
            .address()
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

    pub(super) fn line_address(&self) -> Address {
        self.harness
            .lock()
            .expect("MOESI data cache lock")
            .line()
            .address()
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

pub(super) fn topology_msi_data_response(
    cache: &RiscvTopologyMsiDataCache,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_msi_data_response_result(cache, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_msi_data_response_result(
    cache: &RiscvTopologyMsiDataCache,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MSI data cache lock");
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(RiscvTopologySystemError::MsiDataCache)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(RiscvTopologySystemError::MsiDataCache)?;
    let response_record = harness
        .cpu_responses()
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == delivery.request().id())
        .cloned()
        .ok_or(RiscvTopologySystemError::MissingMsiDataResponse {
            request: delivery.request().id(),
        })?;
    drop(harness);
    cache.record_run(run);

    data_cache_response_record_to_target_outcome(delivery.request(), start_tick, &response_record)
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

    data_cache_response_record_to_target_outcome(delivery.request(), start_tick, &response_record)
}

pub(super) fn topology_data_cache_response(
    msi_data_cache: Option<&RiscvTopologyMsiDataCache>,
    mesi_data_cache: Option<&RiscvTopologyMesiDataCache>,
    moesi_data_cache: Option<&RiscvTopologyMoesiDataCache>,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> Option<TargetOutcome> {
    let line_address = delivery.request().line_address();
    if let Some(cache) = moesi_data_cache.filter(|cache| cache.line_address() == line_address) {
        Some(topology_moesi_data_response(cache, memory_error, delivery))
    } else if let Some(cache) = mesi_data_cache.filter(|cache| cache.line_address() == line_address)
    {
        Some(topology_mesi_data_response(cache, memory_error, delivery))
    } else {
        msi_data_cache
            .filter(|cache| cache.line_address() == line_address)
            .map(|cache| topology_msi_data_response(cache, memory_error, delivery))
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

    data_cache_response_record_to_target_outcome(delivery.request(), start_tick, &response_record)
}

trait DataCacheCpuResponseRecord {
    fn status(&self) -> ResponseStatus;
    fn data(&self) -> Option<&[u8]>;
    fn tick(&self) -> u64;
}

impl DataCacheCpuResponseRecord for CpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }
}

impl DataCacheCpuResponseRecord for MesiCpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }
}

impl DataCacheCpuResponseRecord for MoesiCpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }
}

fn data_cache_response_record_to_target_outcome<R>(
    request: &MemoryRequest,
    start_tick: u64,
    record: &R,
) -> Result<TargetOutcome, RiscvTopologySystemError>
where
    R: DataCacheCpuResponseRecord,
{
    let response = data_cache_response_record_to_memory_response(request, record)?;
    let delay = record.tick().saturating_sub(start_tick);
    if delay == 0 {
        Ok(TargetOutcome::Respond(response))
    } else {
        Ok(TargetOutcome::RespondAfter { delay, response })
    }
}

fn data_cache_response_record_to_memory_response<R>(
    request: &MemoryRequest,
    record: &R,
) -> Result<MemoryResponse, RiscvTopologySystemError>
where
    R: DataCacheCpuResponseRecord,
{
    match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(RiscvTopologySystemError::Memory)
        }
        ResponseStatus::Retry => Ok(MemoryResponse::retry(request)),
    }
}

pub(super) fn merge_msi_data_cache_activity(
    mut fabric_activity: Vec<rem6_fabric::FabricLaneActivity>,
    mut dram_activity: Vec<DramTargetActivity>,
    cache: Option<&RiscvTopologyMsiDataCache>,
    marker: Option<usize>,
) -> (
    Vec<rem6_fabric::FabricLaneActivity>,
    Vec<DramTargetActivity>,
) {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return (fabric_activity, dram_activity);
    };

    merge_data_cache_run_activity(
        &mut fabric_activity,
        &mut dram_activity,
        cache.runs_since(marker),
    );

    (fabric_activity, dram_activity)
}

pub(super) fn msi_data_cache_run_records_since(
    cache: Option<&RiscvTopologyMsiDataCache>,
    marker: Option<usize>,
) -> Vec<RiscvDataCacheRunRecord> {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return Vec::new();
    };
    data_cache_run_records_since(RiscvDataCacheProtocol::Msi, cache.runs_since(marker))
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

    merge_data_cache_run_activity(
        &mut fabric_activity,
        &mut dram_activity,
        cache.runs_since(marker),
    );

    (fabric_activity, dram_activity)
}

pub(super) fn mesi_data_cache_run_records_since(
    cache: Option<&RiscvTopologyMesiDataCache>,
    marker: Option<usize>,
) -> Vec<RiscvDataCacheRunRecord> {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return Vec::new();
    };
    data_cache_run_records_since(RiscvDataCacheProtocol::Mesi, cache.runs_since(marker))
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

    merge_data_cache_run_activity(
        &mut fabric_activity,
        &mut dram_activity,
        cache.runs_since(marker),
    );

    (fabric_activity, dram_activity)
}

pub(super) fn moesi_data_cache_run_records_since(
    cache: Option<&RiscvTopologyMoesiDataCache>,
    marker: Option<usize>,
) -> Vec<RiscvDataCacheRunRecord> {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return Vec::new();
    };
    data_cache_run_records_since(RiscvDataCacheProtocol::Moesi, cache.runs_since(marker))
}

fn merge_data_cache_run_activity(
    fabric_activity: &mut Vec<rem6_fabric::FabricLaneActivity>,
    dram_activity: &mut Vec<DramTargetActivity>,
    runs: Vec<ParallelCoherenceRunSummary>,
) {
    for run in runs {
        fabric_activity.extend(run.fabric_activities().into_values());
        dram_activity.extend(run.dram_target_activities().into_values());
    }
}

fn data_cache_run_records_since(
    protocol: RiscvDataCacheProtocol,
    runs: Vec<ParallelCoherenceRunSummary>,
) -> Vec<RiscvDataCacheRunRecord> {
    runs.into_iter()
        .map(|run| RiscvDataCacheRunRecord::new(protocol, run))
        .collect()
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
