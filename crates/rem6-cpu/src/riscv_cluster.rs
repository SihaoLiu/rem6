use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionedScheduler, RunSummary, SchedulerContext, Tick};
use rem6_memory::AgentId;
use rem6_transport::{
    MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportEndpointId,
};

use crate::{CpuId, RiscvCore, RiscvCoreDriveAction, RiscvCpuError};

#[derive(Clone, Debug)]
pub struct RiscvCluster {
    cores: BTreeMap<CpuId, RiscvCore>,
}

impl RiscvCluster {
    pub fn new<I>(cores: I) -> Result<Self, RiscvClusterError>
    where
        I: IntoIterator<Item = RiscvCore>,
    {
        let mut by_cpu = BTreeMap::new();
        let mut by_agent = BTreeMap::new();
        let mut by_fetch_endpoint = BTreeMap::new();
        let mut by_data_endpoint = BTreeMap::new();

        for core in cores {
            let cpu = core.id();
            if by_cpu.contains_key(&cpu) {
                return Err(RiscvClusterError::DuplicateCpu { cpu });
            }

            let agent = core.agent();
            if let Some(existing) = by_agent.insert(agent, cpu) {
                return Err(RiscvClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                });
            }

            let fetch_endpoint = core.fetch_endpoint();
            if let Some(existing) = by_fetch_endpoint.insert(fetch_endpoint.clone(), cpu) {
                return Err(RiscvClusterError::DuplicateFetchEndpoint {
                    endpoint: fetch_endpoint,
                    existing,
                    duplicate: cpu,
                });
            }

            if let Some(data_endpoint) = core.data_endpoint() {
                if let Some(existing) = by_data_endpoint.insert(data_endpoint.clone(), cpu) {
                    return Err(RiscvClusterError::DuplicateDataEndpoint {
                        endpoint: data_endpoint,
                        existing,
                        duplicate: cpu,
                    });
                }
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

    pub fn core(&self, cpu: CpuId) -> Result<RiscvCore, RiscvClusterError> {
        self.cores
            .get(&cpu)
            .cloned()
            .ok_or(RiscvClusterError::UnknownCpu { cpu })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_core_next_action<F, D>(
        &self,
        cpu: CpuId,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvClusterError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.core(cpu)?
            .drive_next_action(
                scheduler,
                transport,
                fetch_trace,
                data_trace,
                fetch_responder,
                data_responder,
            )
            .map_err(|error| RiscvClusterError::Core { cpu, error })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let mut actions = Vec::new();
        for (cpu, core) in &self.cores {
            if let Some(action) = core
                .drive_next_action(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    fetch_responder(*cpu),
                    data_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                actions.push(RiscvClusterDriveEvent::new(*cpu, action));
            }
        }

        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<RiscvClusterTurn, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let core_events = self.drive_ready_cores(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
            fetch_responder,
            data_responder,
        )?;
        if !core_events.is_empty() {
            return Ok(RiscvClusterTurn::core(core_events));
        }

        if scheduler.is_idle() {
            return Ok(RiscvClusterTurn::idle(scheduler.now()));
        }

        Ok(RiscvClusterTurn::scheduler(scheduler.run_next_epoch()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterDriveEvent {
    cpu: CpuId,
    action: RiscvCoreDriveAction,
}

impl RiscvClusterDriveEvent {
    pub const fn new(cpu: CpuId, action: RiscvCoreDriveAction) -> Self {
        Self { cpu, action }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn action(&self) -> &RiscvCoreDriveAction {
        &self.action
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterTurn {
    core_events: Vec<RiscvClusterDriveEvent>,
    scheduler: Option<RunSummary>,
    idle_tick: Option<Tick>,
}

impl RiscvClusterTurn {
    pub fn core(core_events: Vec<RiscvClusterDriveEvent>) -> Self {
        Self {
            core_events,
            scheduler: None,
            idle_tick: None,
        }
    }

    pub const fn scheduler(summary: RunSummary) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: Some(summary),
            idle_tick: None,
        }
    }

    pub const fn idle(tick: Tick) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: None,
            idle_tick: Some(tick),
        }
    }

    pub fn core_events(&self) -> &[RiscvClusterDriveEvent] {
        &self.core_events
    }

    pub const fn scheduler_summary(&self) -> Option<RunSummary> {
        self.scheduler
    }

    pub const fn idle_tick(&self) -> Option<Tick> {
        self.idle_tick
    }

    pub const fn is_idle(&self) -> bool {
        self.idle_tick.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvClusterError {
    DuplicateCpu {
        cpu: CpuId,
    },
    DuplicateAgent {
        agent: AgentId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateFetchEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateDataEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    UnknownCpu {
        cpu: CpuId,
    },
    Core {
        cpu: CpuId,
        error: RiscvCpuError,
    },
}

impl fmt::Display for RiscvClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCpu { cpu } => {
                write!(formatter, "CPU {} is already registered", cpu.get())
            }
            Self::DuplicateAgent {
                agent,
                existing,
                duplicate,
            } => write!(
                formatter,
                "agent {} is assigned to CPU {} and CPU {}",
                agent.get(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateFetchEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "fetch endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateDataEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "data endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::UnknownCpu { cpu } => write!(formatter, "CPU {} is not registered", cpu.get()),
            Self::Core { cpu, error } => {
                write!(formatter, "CPU {} action failed: {error}", cpu.get())
            }
        }
    }
}

impl Error for RiscvClusterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core { error, .. } => Some(error),
            _ => None,
        }
    }
}
