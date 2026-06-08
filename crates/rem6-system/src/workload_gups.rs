use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler, Tick};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_traffic::{
    GupsTrafficGenerator, TrafficController, TrafficControllerConfig, TrafficControllerState,
    TrafficGupsConfig, TrafficIdleConfig, TrafficIdleGenerator, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTransition,
    TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryTrace, MemoryTransport, TargetOutcome, TransportEndpointId,
    TransportError,
};
use rem6_workload::{
    WorkloadError, WorkloadGupsRun, WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadReplayPlan,
    WorkloadResult, WorkloadRouteId, WorkloadTopology,
};

use crate::{traffic_gups_controller_transport_run, TrafficGupsTransportError};

const GUPS_STATE: TrafficStateId = TrafficStateId::new(0);
const GUPS_IDLE_STATE: TrafficStateId = TrafficStateId::new(1);
const GUPS_ELEMENT_BYTES: u64 = 8;

pub fn run_workload_gups_plan(
    plan: &WorkloadReplayPlan,
) -> Result<WorkloadResult, WorkloadGupsRunError> {
    let topology = plan
        .topology()
        .ok_or(WorkloadGupsRunError::MissingTopology)?;
    let mut summaries = Vec::new();
    let mut final_tick = 0;
    for run in plan.gups_runs() {
        let summary = execute_gups_run(topology, run)?;
        final_tick = final_tick.max(summary.final_tick());
        summaries.push(summary);
    }

    let result = WorkloadResult::new(plan.manifest_identity(), final_tick)
        .with_gups_run_summaries(summaries);
    plan.verify_result(&result)
        .map_err(WorkloadGupsRunError::Workload)?;
    Ok(result)
}

fn execute_gups_run(
    topology: &WorkloadTopology,
    run: &WorkloadGupsRun,
) -> Result<rem6_workload::WorkloadGupsRunSummary, WorkloadGupsRunError> {
    validate_memory_start(run)?;
    let target = topology
        .memory_targets()
        .iter()
        .find(|target| target.target() == run.memory_target())
        .ok_or(WorkloadGupsRunError::MissingMemoryTarget {
            route: run.route().clone(),
            target: run.memory_target(),
        })?;
    validate_target_range(run, target)?;

    let route = topology
        .memory_routes()
        .iter()
        .find(|route| route.id() == run.route())
        .ok_or_else(|| WorkloadGupsRunError::MissingRoute {
            route: run.route().clone(),
        })?;
    if route.hops().iter().any(|hop| hop.fabric().is_some()) {
        return Err(WorkloadGupsRunError::UnsupportedFabricRoute {
            route: run.route().clone(),
        });
    }

    let line_layout =
        CacheLineLayout::new(target.line_bytes()).map_err(WorkloadGupsRunError::Memory)?;
    let gups_config = TrafficGupsConfig::new(
        AgentId::new(run.agent()),
        line_layout,
        run.memory_start(),
        run.memory_size(),
    )
    .map_err(TrafficGupsTransportError::from)?
    .with_update_limit(run.updates())
    .map_err(TrafficGupsTransportError::from)?
    .with_rng_state(run.rng_state());
    let mut controller = gups_controller(gups_config)?;
    controller
        .start(0)
        .map_err(TrafficGupsTransportError::from)?;

    let memory = Arc::new(Mutex::new(build_gups_memory_store(
        run,
        target,
        line_layout,
    )?));
    let target_memory = Arc::clone(&memory);
    let responder = Arc::new(move |delivery: &rem6_transport::RequestDelivery| {
        let outcome = target_memory
            .lock()
            .expect("workload GUPS memory lock")
            .respond(delivery.request())
            .expect("workload GUPS memory response");
        match outcome.response().cloned() {
            Some(response) => TargetOutcome::Respond(response),
            None => TargetOutcome::NoResponse,
        }
    });

    let worker_limit = u32::try_from(topology.parallel_worker_limit()).map_err(|_| {
        WorkloadGupsRunError::WorkerLimitOverflow {
            workers: topology.parallel_worker_limit(),
        }
    })?;
    let mut scheduler =
        PartitionedScheduler::with_min_remote_delay(worker_limit, topology.min_remote_delay())
            .map_err(TrafficGupsTransportError::from)?;
    let mut transport = MemoryTransport::new();
    let route_id = transport
        .add_route(workload_gups_memory_route(route)?)
        .map_err(WorkloadGupsRunError::Transport)?;
    let trace = MemoryTrace::new();
    let transport_run = traffic_gups_controller_transport_run(
        &mut controller,
        GUPS_STATE,
        &mut scheduler,
        &transport,
        route_id,
        trace,
        responder,
    )
    .map_err(WorkloadGupsRunError::Traffic)?;
    if let Some(maximum_final_tick) = run.maximum_final_tick() {
        if transport_run.final_tick() > maximum_final_tick {
            return Err(WorkloadGupsRunError::FinalTickExceeded {
                route: run.route().clone(),
                final_tick: transport_run.final_tick(),
                maximum_final_tick,
            });
        }
    }

    Ok(transport_run.workload_gups_run_summary(run.route().clone()))
}

fn validate_memory_start(run: &WorkloadGupsRun) -> Result<(), WorkloadGupsRunError> {
    if run.memory_start().get().is_multiple_of(GUPS_ELEMENT_BYTES) {
        return Ok(());
    }
    Err(WorkloadGupsRunError::UnalignedMemoryStart {
        route: run.route().clone(),
        memory_start: run.memory_start(),
    })
}

fn validate_target_range(
    run: &WorkloadGupsRun,
    target: &WorkloadMemoryTarget,
) -> Result<(), WorkloadGupsRunError> {
    let range = run.memory_range().map_err(WorkloadGupsRunError::Workload)?;
    if target.range().contains_range(range) {
        return Ok(());
    }
    Err(WorkloadGupsRunError::MemoryRangeOutsideTarget {
        route: run.route().clone(),
        target: target.target(),
        memory_start: run.memory_start(),
        memory_size: run.memory_size(),
    })
}

fn gups_controller(config: TrafficGupsConfig) -> Result<TrafficController, WorkloadGupsRunError> {
    let graph = TrafficStateGraphConfig::new(
        vec![
            TrafficStateSpec::new(GUPS_STATE, 1),
            TrafficStateSpec::new(GUPS_IDLE_STATE, u64::MAX),
        ],
        GUPS_STATE,
        vec![
            TrafficTransition::new(
                GUPS_STATE,
                GUPS_IDLE_STATE,
                TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                    .map_err(TrafficGupsTransportError::from)?,
            ),
            TrafficTransition::new(
                GUPS_IDLE_STATE,
                GUPS_IDLE_STATE,
                TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                    .map_err(TrafficGupsTransportError::from)?,
            ),
        ],
    )
    .map_err(TrafficGupsTransportError::from)?;
    let gups = TrafficControllerState::new(
        GUPS_STATE,
        TrafficStateGenerator::Gups(GupsTrafficGenerator::new(config)),
    );
    let idle = TrafficControllerState::new(
        GUPS_IDLE_STATE,
        TrafficStateGenerator::Idle(TrafficIdleGenerator::new(TrafficIdleConfig::new(u64::MAX))),
    );
    let config = TrafficControllerConfig::new(graph, vec![gups, idle])
        .map_err(TrafficGupsTransportError::from)?;
    Ok(TrafficController::new(config))
}

fn build_gups_memory_store(
    run: &WorkloadGupsRun,
    target: &WorkloadMemoryTarget,
    line_layout: CacheLineLayout,
) -> Result<PartitionedMemoryStore, WorkloadGupsRunError> {
    let target_id = MemoryTargetId::new(target.target());
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(target_id, line_layout)
        .map_err(WorkloadGupsRunError::Memory)?;
    store
        .map_region(
            target_id,
            target.range().start(),
            AccessSize::new(target.range().size().bytes()).map_err(WorkloadGupsRunError::Memory)?,
        )
        .map_err(WorkloadGupsRunError::Memory)?;
    insert_gups_zero_lines(&mut store, target_id, run, line_layout)?;
    Ok(store)
}

fn insert_gups_zero_lines(
    store: &mut PartitionedMemoryStore,
    target: MemoryTargetId,
    run: &WorkloadGupsRun,
    line_layout: CacheLineLayout,
) -> Result<(), WorkloadGupsRunError> {
    let end = run
        .memory_start()
        .get()
        .checked_add(run.memory_size())
        .ok_or_else(|| {
            WorkloadGupsRunError::Workload(WorkloadError::Memory(MemoryError::AddressOverflow {
                start: run.memory_start(),
                size: AccessSize::new(run.memory_size()).expect("GUPS run size was validated"),
            }))
        })?;
    let mut line = line_layout.line_address(run.memory_start());
    let final_line = line_layout.line_address(Address::new(end - 1));
    loop {
        store
            .insert_line(target, line, vec![0; line_layout.bytes() as usize])
            .map_err(WorkloadGupsRunError::Memory)?;
        if line == final_line {
            break;
        }
        line = Address::new(line.get() + line_layout.bytes());
    }
    Ok(())
}

fn workload_gups_memory_route(
    route: &WorkloadMemoryRoute,
) -> Result<MemoryRoute, WorkloadGupsRunError> {
    let source = TransportEndpointId::new(route.source_endpoint())
        .map_err(WorkloadGupsRunError::Transport)?;
    let hops = route
        .hops()
        .iter()
        .map(workload_gups_memory_route_hop)
        .collect::<Result<Vec<_>, WorkloadGupsRunError>>()?;
    MemoryRoute::new_path(source, PartitionId::new(route.source_partition()), hops)
        .map_err(WorkloadGupsRunError::Transport)
}

fn workload_gups_memory_route_hop(
    hop: &rem6_workload::WorkloadRouteHop,
) -> Result<MemoryRouteHop, WorkloadGupsRunError> {
    MemoryRouteHop::new(
        TransportEndpointId::new(hop.endpoint()).map_err(WorkloadGupsRunError::Transport)?,
        PartitionId::new(hop.partition()),
        hop.request_latency(),
        hop.response_latency(),
    )
    .map_err(WorkloadGupsRunError::Transport)
}

#[derive(Debug)]
pub enum WorkloadGupsRunError {
    Workload(WorkloadError),
    Memory(MemoryError),
    Transport(TransportError),
    Traffic(TrafficGupsTransportError),
    MissingTopology,
    MissingRoute {
        route: WorkloadRouteId,
    },
    MissingMemoryTarget {
        route: WorkloadRouteId,
        target: u32,
    },
    MemoryRangeOutsideTarget {
        route: WorkloadRouteId,
        target: u32,
        memory_start: Address,
        memory_size: u64,
    },
    UnalignedMemoryStart {
        route: WorkloadRouteId,
        memory_start: Address,
    },
    UnsupportedFabricRoute {
        route: WorkloadRouteId,
    },
    WorkerLimitOverflow {
        workers: usize,
    },
    FinalTickExceeded {
        route: WorkloadRouteId,
        final_tick: Tick,
        maximum_final_tick: Tick,
    },
}

impl fmt::Display for WorkloadGupsRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workload(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Traffic(error) => write!(formatter, "{error}"),
            Self::MissingTopology => write!(formatter, "workload GUPS run requires a topology"),
            Self::MissingRoute { route } => {
                write!(formatter, "workload GUPS route {} is not defined", route.as_str())
            }
            Self::MissingMemoryTarget { route, target } => write!(
                formatter,
                "workload GUPS route {} uses missing memory target {target}",
                route.as_str()
            ),
            Self::MemoryRangeOutsideTarget {
                route,
                target,
                memory_start,
                memory_size,
            } => write!(
                formatter,
                "workload GUPS route {} range {:#x}+{} is outside memory target {target}",
                route.as_str(),
                memory_start.get(),
                memory_size
            ),
            Self::UnalignedMemoryStart {
                route,
                memory_start,
            } => write!(
                formatter,
                "workload GUPS route {} starts at unaligned address {:#x}",
                route.as_str(),
                memory_start.get()
            ),
            Self::UnsupportedFabricRoute { route } => write!(
                formatter,
                "workload GUPS route {} uses fabric timing",
                route.as_str()
            ),
            Self::WorkerLimitOverflow { workers } => write!(
                formatter,
                "workload GUPS worker limit {workers} does not fit the scheduler"
            ),
            Self::FinalTickExceeded {
                route,
                final_tick,
                maximum_final_tick,
            } => write!(
                formatter,
                "workload GUPS route {} finished at tick {final_tick}, above maximum {maximum_final_tick}",
                route.as_str()
            ),
        }
    }
}

impl Error for WorkloadGupsRunError {}

impl From<TrafficGupsTransportError> for WorkloadGupsRunError {
    fn from(error: TrafficGupsTransportError) -> Self {
        Self::Traffic(error)
    }
}
