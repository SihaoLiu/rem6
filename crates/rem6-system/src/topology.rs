use std::error::Error;
use std::fmt;

use rem6_cpu::{CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_kernel::{PartitionedScheduler, SchedulerError, Tick};
use rem6_topology::Topology;
use rem6_transport::MemoryTransport;

pub struct RiscvTopologySystem {
    topology: Topology,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    cluster: RiscvCluster,
}

impl RiscvTopologySystem {
    pub fn with_min_remote_delay(
        topology: Topology,
        cluster_config: RiscvClusterTopologyConfig,
        min_remote_delay: Tick,
    ) -> Result<Self, RiscvTopologySystemError> {
        let scheduler = PartitionedScheduler::with_min_remote_delay(
            topology.partition_count(),
            min_remote_delay,
        )
        .map_err(RiscvTopologySystemError::Scheduler)?;
        let mut transport = MemoryTransport::new();
        let cluster = RiscvCluster::from_topology(&topology, &mut transport, cluster_config)
            .map_err(RiscvTopologySystemError::CpuTopology)?;

        Ok(Self {
            topology,
            scheduler,
            transport,
            cluster,
        })
    }

    pub const fn topology(&self) -> &Topology {
        &self.topology
    }

    pub const fn scheduler(&self) -> &PartitionedScheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut PartitionedScheduler {
        &mut self.scheduler
    }

    pub const fn transport(&self) -> &MemoryTransport {
        &self.transport
    }

    pub const fn cluster(&self) -> &RiscvCluster {
        &self.cluster
    }

    pub fn execution_parts_mut(
        &mut self,
    ) -> (&RiscvCluster, &mut PartitionedScheduler, &MemoryTransport) {
        (&self.cluster, &mut self.scheduler, &self.transport)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologySystemError {
    Scheduler(SchedulerError),
    CpuTopology(CpuTopologyError),
}

impl fmt::Display for RiscvTopologySystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::CpuTopology(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvTopologySystemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::CpuTopology(error) => Some(error),
        }
    }
}
