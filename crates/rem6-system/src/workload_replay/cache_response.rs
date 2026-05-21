use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_coherence::PartitionedCacheAgentConfig;
use rem6_kernel::PartitionId;
use rem6_memory::AgentId;
use rem6_transport::{RequestDelivery, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadMemoryRoute, WorkloadRouteId, WorkloadTopology};

use super::{
    memory_response, RiscvWorkloadReplayError, WorkloadDataCacheBackend, WorkloadMemoryBackend,
};

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
