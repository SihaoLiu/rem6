use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_coherence::{
    ChiCpuResponseRecord, ChiHarnessError, CpuResponseRecord, HarnessError, MesiCpuResponseRecord,
    MesiHarnessError, MoesiCpuResponseRecord, MoesiHarnessError, ParallelCoherenceRunSummary,
    PartitionedCacheAgentConfig, PartitionedChiDirectoryLineHarness,
    PartitionedDirectoryLineHarness, PartitionedMesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarness,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AgentId, MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_transport::{RequestDelivery, TargetBatchOutcome, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadMemoryRoute, WorkloadRouteId, WorkloadTopology};

use super::data_cache_backend::WorkloadDataCacheBackend;
use super::{memory_response, RiscvWorkloadReplayError, WorkloadMemoryBackend};
use crate::{RiscvDataCacheProtocol, RiscvDataCacheRunRecord};

pub(super) fn data_cache_agents(
    topology: &WorkloadTopology,
) -> Result<Vec<PartitionedCacheAgentConfig>, RiscvWorkloadReplayError> {
    let mut agents = BTreeMap::new();
    for config in topology
        .riscv_cores()
        .iter()
        .filter_map(|core| {
            let data_endpoint = core.data_endpoint()?;
            let data_route = core.data_route()?;
            Some((core, data_endpoint, data_route))
        })
        .map(|(core, data_endpoint, data_route)| {
            let route = workload_route(topology, data_route)?;
            data_cache_agent_config(core.agent(), data_endpoint, route)
        })
    {
        let config = config?;
        agents.entry(config.agent()).or_insert(config);
    }

    for copy in topology.gpu_dma_copies() {
        let route = workload_route(topology, copy.route())?;
        let config = data_cache_agent_config(copy.agent(), route.source_endpoint(), route)?;
        agents.entry(config.agent()).or_insert(config);
    }

    for copy in topology.accelerator_dma_copies() {
        let route = workload_route(topology, copy.route())?;
        let config = data_cache_agent_config(copy.agent(), route.source_endpoint(), route)?;
        agents.entry(config.agent()).or_insert(config);
    }

    Ok(agents.into_values().collect())
}

pub(super) fn cached_memory_response(
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    memory: &WorkloadMemoryBackend,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    if let Some(data_cache) = data_cache {
        if let Some(outcome) = data_cache
            .lock()
            .expect("workload data cache lock")
            .respond(delivery)
        {
            return outcome;
        }
    }
    memory_response(memory, delivery)
}

pub(super) fn cached_target_batch_response(
    data_cache: Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
    memory: &WorkloadMemoryBackend,
    deliveries: Vec<RequestDelivery>,
) -> Option<Vec<TargetBatchOutcome>> {
    let Some(data_cache) = data_cache else {
        return memory.target_batch_response(deliveries);
    };

    let mut outcomes = Vec::with_capacity(deliveries.len());
    let mut memory_deliveries = Vec::new();
    for delivery in deliveries {
        let cache_outcome = data_cache
            .lock()
            .expect("workload data cache lock")
            .respond(&delivery);
        match cache_outcome {
            Some(outcome) => {
                outcomes.push(TargetBatchOutcome::new(delivery.request().id(), outcome))
            }
            None => memory_deliveries.push(delivery),
        }
    }

    if let Some(mut memory_outcomes) = memory.target_batch_response(memory_deliveries.clone()) {
        outcomes.append(&mut memory_outcomes);
    } else {
        outcomes.extend(memory_deliveries.iter().map(|delivery| {
            TargetBatchOutcome::new(delivery.request().id(), memory_response(memory, delivery))
        }));
    }

    Some(outcomes)
}

pub(super) trait WorkloadDataCacheResponseHarness {
    type Error;
    type Response: WorkloadDataCacheResponseRecord;

    fn now(&self) -> u64;
    fn cpu_responses(&self) -> Vec<Self::Response>;
    fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<(), Self::Error>;
    fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, Self::Error>;
}

pub(super) fn data_cache_response_result<H, M>(
    protocol: RiscvDataCacheProtocol,
    records: &mut Vec<RiscvDataCacheRunRecord>,
    harness: &mut H,
    delivery: &RequestDelivery,
    map_error: M,
) -> Result<TargetOutcome, RiscvWorkloadReplayError>
where
    H: WorkloadDataCacheResponseHarness,
    M: Fn(H::Error) -> RiscvWorkloadReplayError,
{
    let start_tick = harness.now();
    let responses_before = harness.cpu_responses().len();
    harness
        .submit_cpu_request_parallel(delivery.request().id().agent(), delivery.request().clone())
        .map_err(&map_error)?;
    let run = harness
        .run_until_idle_parallel_recorded()
        .map_err(&map_error)?;
    let responses = harness.cpu_responses();
    let response_record =
        data_cache_response_record(&responses, responses_before, delivery.request().id())?;
    records.push(RiscvDataCacheRunRecord::new(protocol, run));

    data_cache_response_record_to_target_outcome(delivery.request(), start_tick, &response_record)
}

impl WorkloadDataCacheResponseHarness for PartitionedDirectoryLineHarness {
    type Error = HarnessError;
    type Response = CpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedDirectoryLineHarness::cpu_responses(self)
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
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
}

impl WorkloadDataCacheResponseHarness for PartitionedMesiDirectoryLineHarness {
    type Error = MesiHarnessError;
    type Response = MesiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedMesiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedMesiDirectoryLineHarness::cpu_responses(self)
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
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
}

impl WorkloadDataCacheResponseHarness for PartitionedMoesiDirectoryLineHarness {
    type Error = MoesiHarnessError;
    type Response = MoesiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedMoesiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedMoesiDirectoryLineHarness::cpu_responses(self)
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
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
}

impl WorkloadDataCacheResponseHarness for PartitionedChiDirectoryLineHarness {
    type Error = ChiHarnessError;
    type Response = ChiCpuResponseRecord;

    fn now(&self) -> u64 {
        PartitionedChiDirectoryLineHarness::now(self)
    }

    fn cpu_responses(&self) -> Vec<Self::Response> {
        PartitionedChiDirectoryLineHarness::cpu_responses(self)
    }

    fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
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
}

fn workload_route<'a>(
    topology: &'a WorkloadTopology,
    route: &WorkloadRouteId,
) -> Result<&'a WorkloadMemoryRoute, RiscvWorkloadReplayError> {
    topology
        .memory_routes()
        .iter()
        .find(|candidate| candidate.id() == route)
        .ok_or_else(|| RiscvWorkloadReplayError::MissingRoute {
            route: route.clone(),
        })
}

fn data_cache_agent_config(
    agent: u32,
    endpoint: &str,
    route: &WorkloadMemoryRoute,
) -> Result<PartitionedCacheAgentConfig, RiscvWorkloadReplayError> {
    Ok(PartitionedCacheAgentConfig::new(
        AgentId::new(agent),
        PartitionId::new(route.source_partition()),
        TransportEndpointId::new(endpoint).map_err(RiscvWorkloadReplayError::Transport)?,
        route.request_latency(),
        route.response_latency(),
    ))
}

pub(super) trait WorkloadDataCacheResponseRecord: Clone {
    fn status(&self) -> ResponseStatus;
    fn data(&self) -> Option<&[u8]>;
    fn tick(&self) -> u64;
    fn request(&self) -> MemoryRequestId;
}

impl WorkloadDataCacheResponseRecord for CpuResponseRecord {
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

impl WorkloadDataCacheResponseRecord for MesiCpuResponseRecord {
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

impl WorkloadDataCacheResponseRecord for MoesiCpuResponseRecord {
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

impl WorkloadDataCacheResponseRecord for ChiCpuResponseRecord {
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

fn data_cache_response_record<R>(
    responses: &[R],
    responses_before: usize,
    request: MemoryRequestId,
) -> Result<R, RiscvWorkloadReplayError>
where
    R: WorkloadDataCacheResponseRecord,
{
    responses
        .get(responses_before..)
        .unwrap_or_default()
        .iter()
        .find(|record| record.request() == request)
        .cloned()
        .ok_or(RiscvWorkloadReplayError::MissingDataCacheResponse { request })
}

fn data_cache_response_record_to_target_outcome<R>(
    request: &MemoryRequest,
    start_tick: u64,
    record: &R,
) -> Result<TargetOutcome, RiscvWorkloadReplayError>
where
    R: WorkloadDataCacheResponseRecord,
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
) -> Result<MemoryResponse, RiscvWorkloadReplayError>
where
    R: WorkloadDataCacheResponseRecord,
{
    match record.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(request, record.data().map(<[u8]>::to_vec))
                .map_err(RiscvWorkloadReplayError::Memory)
        }
        ResponseStatus::Retry => Ok(MemoryResponse::retry(request)),
        ResponseStatus::StoreConditionalFailed => MemoryResponse::store_conditional_failed(request)
            .map_err(RiscvWorkloadReplayError::Memory),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_transport::TargetBatchOutcome;

    type CachedTargetBatchAdapter = fn(
        Option<&Arc<Mutex<WorkloadDataCacheBackend>>>,
        &WorkloadMemoryBackend,
        Vec<RequestDelivery>,
    ) -> Option<Vec<TargetBatchOutcome>>;

    #[test]
    fn cached_target_batch_response_entrypoint_exists() {
        let _adapter: CachedTargetBatchAdapter = cached_target_batch_response;
    }
}
