use std::collections::BTreeMap;

use rem6_kernel::{PartitionEventId, PartitionedScheduler, SchedulerContext};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{CpuClusterError, CpuCore, CpuId};

#[derive(Clone, Debug)]
pub struct CpuCluster {
    cores: BTreeMap<CpuId, CpuCore>,
}

impl CpuCluster {
    pub fn new<I>(cores: I) -> Result<Self, CpuClusterError>
    where
        I: IntoIterator<Item = CpuCore>,
    {
        let mut by_cpu = BTreeMap::new();
        let mut by_agent = BTreeMap::new();
        let mut by_endpoint = BTreeMap::new();

        for core in cores {
            let cpu = core.id();
            if by_cpu.contains_key(&cpu) {
                return Err(CpuClusterError::DuplicateCpu { cpu });
            }

            let agent = core.agent();
            if let Some(existing) = by_agent.insert(agent, cpu) {
                return Err(CpuClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                });
            }

            let endpoint = core.fetch_endpoint();
            if let Some(existing) = by_endpoint.insert(endpoint.clone(), cpu) {
                return Err(CpuClusterError::DuplicateFetchEndpoint {
                    endpoint,
                    existing,
                    duplicate: cpu,
                });
            }

            by_cpu.insert(cpu, core);
        }

        Ok(Self { cores: by_cpu })
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }

    pub fn core_ids(&self) -> Vec<CpuId> {
        self.cores.keys().copied().collect()
    }

    pub fn core(&self, cpu: CpuId) -> Result<CpuCore, CpuClusterError> {
        self.cores
            .get(&cpu)
            .cloned()
            .ok_or(CpuClusterError::UnknownCpu { cpu })
    }

    pub fn issue_next_fetch<F>(
        &self,
        cpu: CpuId,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuClusterError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.core(cpu)?
            .issue_next_fetch(scheduler, transport, trace, responder)
            .map_err(CpuClusterError::Cpu)
    }
}
