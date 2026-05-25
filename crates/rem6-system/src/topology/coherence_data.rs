use std::sync::{Arc, Mutex};

use rem6_coherence::{
    ChiCpuResponseRecord, ChiHarnessError, CpuResponseRecord, HarnessError, MesiCpuResponseRecord,
    MesiHarnessError, MoesiCpuResponseRecord, MoesiHarnessError, ParallelCoherenceRunSummary,
    PartitionedChiDirectoryLineHarness, PartitionedDirectoryLineHarness,
    PartitionedMesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarness,
};
use rem6_cpu::RiscvCluster;
use rem6_dram::DramTargetActivity;
use rem6_memory::{Address, MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_protocol_chi::ChiEvent;
use rem6_protocol_mesi::MesiEvent;
use rem6_protocol_moesi::MoesiEvent;
use rem6_protocol_msi::MsiEvent;
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

#[derive(Clone)]
pub(super) struct RiscvTopologyChiDataCache {
    harness: Arc<Mutex<PartitionedChiDirectoryLineHarness>>,
    runs: Arc<Mutex<Vec<ParallelCoherenceRunSummary>>>,
}

impl RiscvTopologyChiDataCache {
    pub(super) fn new(harness: PartitionedChiDirectoryLineHarness) -> Self {
        Self {
            harness: Arc::new(Mutex::new(harness)),
            runs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) fn harness(&self) -> Arc<Mutex<PartitionedChiDirectoryLineHarness>> {
        Arc::clone(&self.harness)
    }

    pub(super) fn runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.runs.lock().expect("CHI data cache run lock").clone()
    }

    pub(super) fn mark_runs(&self) -> usize {
        self.runs.lock().expect("CHI data cache run lock").len()
    }

    pub(super) fn line_address(&self) -> Address {
        self.harness
            .lock()
            .expect("CHI data cache lock")
            .line()
            .address()
    }

    fn runs_since(&self, marker: usize) -> Vec<ParallelCoherenceRunSummary> {
        self.runs
            .lock()
            .expect("CHI data cache run lock")
            .get(marker..)
            .unwrap_or_default()
            .to_vec()
    }

    fn record_run(&self, run: ParallelCoherenceRunSummary) {
        self.runs.lock().expect("CHI data cache run lock").push(run);
    }
}

pub(super) fn topology_msi_data_response(
    cache: &RiscvTopologyMsiDataCache,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_msi_data_response_result(cache, cluster, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_msi_data_response_result(
    cache: &RiscvTopologyMsiDataCache,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MSI data cache lock");
    let (run, outcome) = topology_data_cache_response_result(
        &mut *harness,
        cluster,
        delivery,
        RiscvTopologySystemError::MsiDataCache,
        |request| RiscvTopologySystemError::MissingMsiDataResponse { request },
    )?;
    drop(harness);
    cache.record_run(run);
    Ok(outcome)
}

pub(super) fn topology_mesi_data_response(
    cache: &RiscvTopologyMesiDataCache,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_mesi_data_response_result(cache, cluster, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_mesi_data_response_result(
    cache: &RiscvTopologyMesiDataCache,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MESI data cache lock");
    let (run, outcome) = topology_data_cache_response_result(
        &mut *harness,
        cluster,
        delivery,
        RiscvTopologySystemError::MesiDataCache,
        |request| RiscvTopologySystemError::MissingMesiDataResponse { request },
    )?;
    drop(harness);
    cache.record_run(run);
    Ok(outcome)
}

pub(super) fn topology_data_cache_response(
    msi_data_cache: Option<&RiscvTopologyMsiDataCache>,
    mesi_data_cache: Option<&RiscvTopologyMesiDataCache>,
    moesi_data_cache: Option<&RiscvTopologyMoesiDataCache>,
    chi_data_cache: Option<&RiscvTopologyChiDataCache>,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> Option<TargetOutcome> {
    let line_address = delivery.request().line_address();
    if let Some(cache) = chi_data_cache.filter(|cache| cache.line_address() == line_address) {
        Some(topology_chi_data_response(
            cache,
            cluster,
            memory_error,
            delivery,
        ))
    } else if let Some(cache) =
        moesi_data_cache.filter(|cache| cache.line_address() == line_address)
    {
        Some(topology_moesi_data_response(
            cache,
            cluster,
            memory_error,
            delivery,
        ))
    } else if let Some(cache) = mesi_data_cache.filter(|cache| cache.line_address() == line_address)
    {
        Some(topology_mesi_data_response(
            cache,
            cluster,
            memory_error,
            delivery,
        ))
    } else {
        msi_data_cache
            .filter(|cache| cache.line_address() == line_address)
            .map(|cache| topology_msi_data_response(cache, cluster, memory_error, delivery))
    }
}

pub(super) fn topology_chi_data_response(
    cache: &RiscvTopologyChiDataCache,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_chi_data_response_result(cache, cluster, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_chi_data_response_result(
    cache: &RiscvTopologyChiDataCache,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("CHI data cache lock");
    let (run, outcome) = topology_data_cache_response_result(
        &mut *harness,
        cluster,
        delivery,
        RiscvTopologySystemError::ChiDataCache,
        |request| RiscvTopologySystemError::MissingChiDataResponse { request },
    )?;
    drop(harness);
    cache.record_run(run);
    Ok(outcome)
}

pub(super) fn topology_moesi_data_response(
    cache: &RiscvTopologyMoesiDataCache,
    cluster: &RiscvCluster,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match topology_moesi_data_response_result(cache, cluster, delivery) {
        Ok(outcome) => outcome,
        Err(error) => {
            record_coherence_error(memory_error, error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn topology_moesi_data_response_result(
    cache: &RiscvTopologyMoesiDataCache,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, RiscvTopologySystemError> {
    let mut harness = cache.harness.lock().expect("MOESI data cache lock");
    let (run, outcome) = topology_data_cache_response_result(
        &mut *harness,
        cluster,
        delivery,
        RiscvTopologySystemError::MoesiDataCache,
        |request| RiscvTopologySystemError::MissingMoesiDataResponse { request },
    )?;
    drop(harness);
    cache.record_run(run);
    Ok(outcome)
}

fn invalidate_snooped_reservation(
    cluster: &RiscvCluster,
    target: rem6_memory::AgentId,
    request: &MemoryRequest,
) {
    cluster.invalidate_load_reservation_for_agent_if_overlaps(
        target,
        request.range().start(),
        request.size(),
    );
}

trait DataCacheCpuResponseRecord {
    fn status(&self) -> ResponseStatus;
    fn data(&self) -> Option<&[u8]>;
    fn tick(&self) -> u64;
    fn request(&self) -> MemoryRequestId;
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

    fn request(&self) -> MemoryRequestId {
        self.request()
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

    fn request(&self) -> MemoryRequestId {
        self.request()
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

    fn request(&self) -> MemoryRequestId {
        self.request()
    }
}

impl DataCacheCpuResponseRecord for ChiCpuResponseRecord {
    fn status(&self) -> ResponseStatus {
        self.status()
    }

    fn data(&self) -> Option<&[u8]> {
        self.data()
    }

    fn tick(&self) -> u64 {
        self.tick()
    }

    fn request(&self) -> MemoryRequestId {
        self.request()
    }
}

trait TopologyDataCacheResponseHarness {
    type Error;
    type Response: Clone + DataCacheCpuResponseRecord;

    fn now(&self) -> u64;
    fn cpu_responses(&self) -> Vec<Self::Response>;
    fn directory_decision_count(&self) -> usize;
    fn submit_cpu_request_parallel(
        &mut self,
        agent: rem6_memory::AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error>;
    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error>;
    fn invalidate_snooped_reservations_since(
        &self,
        decisions_before: usize,
        cluster: &RiscvCluster,
        request: &MemoryRequest,
    );
}

impl TopologyDataCacheResponseHarness for PartitionedDirectoryLineHarness {
    type Error = HarnessError;
    type Response = CpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedDirectoryLineHarness::cpu_responses(self)
    }

    fn directory_decision_count(&self) -> usize {
        self.directory_decisions().len()
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: rem6_memory::AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error> {
        PartitionedDirectoryLineHarness::submit_cpu_request_parallel(self, agent, request)
            .map(|_| ())
    }

    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error> {
        PartitionedDirectoryLineHarness::run_until_idle_parallel_recorded(self)
    }

    fn invalidate_snooped_reservations_since(
        &self,
        decisions_before: usize,
        cluster: &RiscvCluster,
        request: &MemoryRequest,
    ) {
        for record in self
            .directory_decisions()
            .get(decisions_before..)
            .unwrap_or_default()
        {
            for snoop in record.decision().snoops() {
                if snoop.event() == MsiEvent::SnoopWrite {
                    invalidate_snooped_reservation(cluster, snoop.target(), request);
                }
            }
        }
    }
}

impl TopologyDataCacheResponseHarness for PartitionedMesiDirectoryLineHarness {
    type Error = MesiHarnessError;
    type Response = MesiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedMesiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedMesiDirectoryLineHarness::cpu_responses(self)
    }

    fn directory_decision_count(&self) -> usize {
        self.directory_decisions().len()
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: rem6_memory::AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error> {
        PartitionedMesiDirectoryLineHarness::submit_cpu_request_parallel(self, agent, request)
            .map(|_| ())
    }

    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error> {
        PartitionedMesiDirectoryLineHarness::run_until_idle_parallel_recorded(self)
    }

    fn invalidate_snooped_reservations_since(
        &self,
        decisions_before: usize,
        cluster: &RiscvCluster,
        request: &MemoryRequest,
    ) {
        for record in self
            .directory_decisions()
            .get(decisions_before..)
            .unwrap_or_default()
        {
            for snoop in record.decision().snoops() {
                if snoop.event() == MesiEvent::SnoopWrite {
                    invalidate_snooped_reservation(cluster, snoop.target(), request);
                }
            }
        }
    }
}

impl TopologyDataCacheResponseHarness for PartitionedMoesiDirectoryLineHarness {
    type Error = MoesiHarnessError;
    type Response = MoesiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedMoesiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedMoesiDirectoryLineHarness::cpu_responses(self)
    }

    fn directory_decision_count(&self) -> usize {
        self.directory_decisions().len()
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: rem6_memory::AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error> {
        PartitionedMoesiDirectoryLineHarness::submit_cpu_request_parallel(self, agent, request)
            .map(|_| ())
    }

    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error> {
        PartitionedMoesiDirectoryLineHarness::run_until_idle_parallel_recorded(self)
    }

    fn invalidate_snooped_reservations_since(
        &self,
        decisions_before: usize,
        cluster: &RiscvCluster,
        request: &MemoryRequest,
    ) {
        for record in self
            .directory_decisions()
            .get(decisions_before..)
            .unwrap_or_default()
        {
            for snoop in record.decision().snoops() {
                if snoop.event() == MoesiEvent::SnoopWrite {
                    invalidate_snooped_reservation(cluster, snoop.target(), request);
                }
            }
        }
    }
}

impl TopologyDataCacheResponseHarness for PartitionedChiDirectoryLineHarness {
    type Error = ChiHarnessError;
    type Response = ChiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedChiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedChiDirectoryLineHarness::cpu_responses(self)
    }

    fn directory_decision_count(&self) -> usize {
        self.directory_decisions().len()
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: rem6_memory::AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error> {
        PartitionedChiDirectoryLineHarness::submit_cpu_request_parallel(self, agent, request)
            .map(|_| ())
    }

    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error> {
        PartitionedChiDirectoryLineHarness::run_until_idle_parallel_recorded(self)
    }

    fn invalidate_snooped_reservations_since(
        &self,
        decisions_before: usize,
        cluster: &RiscvCluster,
        request: &MemoryRequest,
    ) {
        for record in self
            .directory_decisions()
            .get(decisions_before..)
            .unwrap_or_default()
        {
            for snoop in record.decision().snoops() {
                if snoop.event() == ChiEvent::SnoopUnique {
                    invalidate_snooped_reservation(cluster, snoop.target(), request);
                }
            }
        }
    }
}

fn topology_data_cache_response_result<H, M, N>(
    harness: &mut H,
    cluster: &RiscvCluster,
    delivery: &RequestDelivery,
    map_error: M,
    missing_response: N,
) -> Result<(ParallelCoherenceRunSummary, TargetOutcome), RiscvTopologySystemError>
where
    H: TopologyDataCacheResponseHarness,
    M: Fn(H::Error) -> RiscvTopologySystemError,
    N: Fn(MemoryRequestId) -> RiscvTopologySystemError,
{
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    let decisions_before = harness.directory_decision_count();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(&map_error)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(&map_error)?;
    harness.invalidate_snooped_reservations_since(decisions_before, cluster, delivery.request());
    let responses = harness.cpu_responses();
    let response_record =
        data_cache_response_record(&responses, responses_before, delivery.request().id())
            .ok_or_else(|| missing_response(delivery.request().id()))?;
    let outcome = data_cache_response_record_to_target_outcome(
        delivery.request(),
        start_tick,
        &response_record,
    )?;
    Ok((run, outcome))
}

fn data_cache_response_record<R>(
    responses: &[R],
    responses_before: usize,
    request: MemoryRequestId,
) -> Option<R>
where
    R: Clone + DataCacheCpuResponseRecord,
{
    responses
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == request)
        .cloned()
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

pub(super) fn merge_chi_data_cache_activity(
    mut fabric_activity: Vec<rem6_fabric::FabricLaneActivity>,
    mut dram_activity: Vec<DramTargetActivity>,
    cache: Option<&RiscvTopologyChiDataCache>,
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

pub(super) fn chi_data_cache_run_records_since(
    cache: Option<&RiscvTopologyChiDataCache>,
    marker: Option<usize>,
) -> Vec<RiscvDataCacheRunRecord> {
    let (Some(cache), Some(marker)) = (cache, marker) else {
        return Vec::new();
    };
    data_cache_run_records_since(RiscvDataCacheProtocol::Chi, cache.runs_since(marker))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_topology_data_cache_response_harness<H>()
    where
        H: TopologyDataCacheResponseHarness,
        H::Response: DataCacheCpuResponseRecord,
    {
    }

    #[test]
    fn topology_data_cache_response_harness_is_protocol_neutral() {
        assert_topology_data_cache_response_harness::<PartitionedDirectoryLineHarness>();
        assert_topology_data_cache_response_harness::<PartitionedMesiDirectoryLineHarness>();
        assert_topology_data_cache_response_harness::<PartitionedMoesiDirectoryLineHarness>();
        assert_topology_data_cache_response_harness::<PartitionedChiDirectoryLineHarness>();
    }
}
