use std::collections::BTreeSet;

use rem6_kernel::PartitionId;
use rem6_memory::{AgentId, CacheLineLayout};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace, TrafficTraceConfig,
    TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_workload::{WorkloadError, WorkloadTrafficTraceReplayRun};

use super::{RiscvWorkloadReplay, RiscvWorkloadReplayError, RiscvWorkloadTrafficTraceReplay};

pub(super) fn build_traffic_trace_replays(
    workload_replay: &RiscvWorkloadReplay,
) -> Result<Vec<RiscvWorkloadTrafficTraceReplay>, RiscvWorkloadReplayError> {
    let mut replays = workload_replay.traffic_trace_replays.clone();
    let mut routes = BTreeSet::new();
    for replay in &replays {
        if !routes.insert(replay.route().clone()) {
            return Err(RiscvWorkloadReplayError::Workload(
                WorkloadError::DuplicateRoute {
                    route: replay.route().clone(),
                },
            ));
        }
    }
    for replay in workload_replay.plan.traffic_trace_replays() {
        if !routes.insert(replay.route().clone()) {
            return Err(RiscvWorkloadReplayError::Workload(
                WorkloadError::DuplicateRoute {
                    route: replay.route().clone(),
                },
            ));
        }
        replays.push(build_manifest_traffic_trace_replay(
            workload_replay,
            replay,
        )?);
    }
    Ok(replays)
}

fn build_manifest_traffic_trace_replay(
    workload_replay: &RiscvWorkloadReplay,
    replay: &WorkloadTrafficTraceReplayRun,
) -> Result<RiscvWorkloadTrafficTraceReplay, RiscvWorkloadReplayError> {
    let trace_payload = workload_replay
        .resolved_resources
        .as_ref()
        .and_then(|resources| resources.payload_data(replay.resource()))
        .ok_or_else(|| {
            RiscvWorkloadReplayError::Workload(WorkloadError::MissingResourcePayload {
                resource: replay.resource().clone(),
            })
        })?;
    let trace = TrafficTrace::from_gem5_packet_trace(trace_payload, replay.tick_frequency())
        .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
    let line_layout =
        CacheLineLayout::new(replay.line_bytes()).map_err(RiscvWorkloadReplayError::Memory)?;
    let trace_config = TrafficTraceConfig::new(
        AgentId::new(replay.agent()),
        line_layout,
        replay.duration(),
        trace,
    )
    .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
    let controller = manifest_traffic_trace_controller(trace_config)?;
    Ok(RiscvWorkloadTrafficTraceReplay::new(
        controller,
        replay.route().clone(),
        PartitionId::new(replay.control_partition()),
    )
    .with_retry_delay(replay.retry_delay()))
}

fn manifest_traffic_trace_controller(
    config: TrafficTraceConfig,
) -> Result<TrafficController, RiscvWorkloadReplayError> {
    let state = TrafficStateId::new(0);
    let duration = config.duration();
    let transition = TrafficTransition::new(
        state,
        state,
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
            .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?,
    );
    let graph = TrafficStateGraphConfig::new(
        vec![TrafficStateSpec::new(state, duration)],
        state,
        vec![transition],
    )
    .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
    let controller = TrafficControllerConfig::new(
        graph,
        vec![TrafficControllerState::new(
            state,
            TrafficStateGenerator::Trace(TrafficTraceGenerator::new(config)),
        )],
    )
    .map_err(|error| RiscvWorkloadReplayError::TrafficTraceReplay(error.into()))?;
    Ok(TrafficController::new(controller))
}
