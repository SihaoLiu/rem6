use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_system::{traffic_gups_controller_transport_run, TrafficGupsTransportResponseStats};
use rem6_traffic::{
    GupsTrafficGenerator, TrafficController, TrafficControllerConfig, TrafficControllerState,
    TrafficGupsConfig, TrafficIdleConfig, TrafficIdleGenerator, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateProfileSummary, TrafficStateSpec,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport};

use crate::cli_output::emit_cli_output;
use crate::config::{Rem6GupsConfig, StatsFormat};
use crate::runtime_memory::{read_memory_dumps, CliMemoryRuntime};
use crate::stats_output::{gups_stats_output, Rem6GupsStatsInputs};
use crate::{
    execute_error, memory_transport_summary, transport_endpoint, Rem6CliError, Rem6MemoryDump,
    Rem6MemoryTransportSummary, DEFAULT_CACHE_LINE_BYTES,
};

const GUPS_MEMORY_TARGET: MemoryTargetId = MemoryTargetId::new(0);
const GUPS_AGENT: AgentId = AgentId::new(0);
const GUPS_SOURCE_PARTITION: PartitionId = PartitionId::new(0);
const GUPS_MEMORY_PARTITION: PartitionId = PartitionId::new(1);
const GUPS_STATE: TrafficStateId = TrafficStateId::new(0);
const GUPS_IDLE_STATE: TrafficStateId = TrafficStateId::new(1);
const GUPS_ELEMENT_BYTES: u64 = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6GupsArtifact {
    pub(crate) schema: &'static str,
    pub(crate) config: Rem6GupsConfig,
    pub(crate) execution: Rem6GupsExecutionSummary,
    pub(crate) memory_dumps: Vec<Rem6MemoryDump>,
    pub(crate) transport: Rem6MemoryTransportSummary,
    pub(crate) stats_json: String,
    pub(crate) stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6GupsExecutionSummary {
    pub(crate) final_tick: u64,
    pub(crate) scheduled_requests: u64,
    pub(crate) response_stats: TrafficGupsTransportResponseStats,
    pub(crate) profile_summaries: Vec<TrafficStateProfileSummary>,
}

pub(crate) fn run_gups_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6GupsConfig::parse_args(args)?;
    let artifact = run_gups_config(config)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
        None,
    )
}

pub fn run_gups_config(config: Rem6GupsConfig) -> Result<Rem6GupsArtifact, Rem6CliError> {
    let line_layout = CacheLineLayout::new(DEFAULT_CACHE_LINE_BYTES).map_err(execute_error)?;
    validate_gups_memory_start(&config)?;
    let gups_config = TrafficGupsConfig::new(
        GUPS_AGENT,
        line_layout,
        Address::new(config.memory_start()),
        config.memory_size(),
    )
    .map_err(execute_error)?
    .with_update_limit(config.updates())
    .map_err(execute_error)?
    .with_rng_state(config.rng_state());
    validate_gups_tick_budget(&config)?;
    let mut controller = gups_controller(gups_config)?;
    controller.start(0).map_err(execute_error)?;

    let memory = Arc::new(Mutex::new(build_gups_memory_store(&config, line_layout)?));
    let target_memory = Arc::clone(&memory);
    let target = Arc::new(move |delivery: &rem6_transport::RequestDelivery| {
        let outcome = target_memory
            .lock()
            .expect("GUPS CLI memory lock")
            .respond(delivery.request())
            .expect("GUPS CLI memory response");
        match outcome.response().cloned() {
            Some(response) => rem6_transport::TargetOutcome::Respond(response),
            None => rem6_transport::TargetOutcome::NoResponse,
        }
    });

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, config.min_remote_delay())
        .map_err(execute_error)?;
    let mut transport = MemoryTransport::new();
    let route = transport
        .add_route(
            MemoryRoute::new(
                transport_endpoint("gups".to_string())?,
                GUPS_SOURCE_PARTITION,
                transport_endpoint("memory".to_string())?,
                GUPS_MEMORY_PARTITION,
                config.memory_route_delay(),
                config.memory_route_delay(),
            )
            .map_err(execute_error)?,
        )
        .map_err(execute_error)?;
    let trace = MemoryTrace::new();
    let run = traffic_gups_controller_transport_run(
        &mut controller,
        GUPS_STATE,
        &mut scheduler,
        &transport,
        route,
        trace.clone(),
        target,
    )
    .map_err(execute_error)?;
    if run.final_tick() > config.max_tick() {
        return Err(Rem6CliError::Execute {
            error: format!(
                "GUPS final tick {} exceeded max tick {}",
                run.final_tick(),
                config.max_tick()
            ),
        });
    }

    let memory_runtime = CliMemoryRuntime::Store {
        store: memory,
        full_line_backing: Arc::new(Mutex::new(Vec::new())),
    };
    let memory_dumps = read_memory_dumps(&memory_runtime, line_layout, config.memory_dumps())?;
    let transport = memory_transport_summary(&trace);
    let execution = Rem6GupsExecutionSummary {
        final_tick: run.final_tick(),
        scheduled_requests: run.scheduled_count() as u64,
        response_stats: *run.response_stats(),
        profile_summaries: controller
            .snapshot()
            .generators()
            .iter()
            .map(|entry| entry.profile_summary())
            .collect(),
    };
    let stats = gups_stats_output(Rem6GupsStatsInputs {
        config: &config,
        execution: &execution,
        transport: &transport,
        memory_dumps: &memory_dumps,
    })?;

    Ok(Rem6GupsArtifact {
        schema: "rem6.cli.gups.v1",
        config,
        execution,
        memory_dumps,
        transport,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

fn validate_gups_memory_start(config: &Rem6GupsConfig) -> Result<(), Rem6CliError> {
    if config.memory_start().is_multiple_of(GUPS_ELEMENT_BYTES) {
        return Ok(());
    }
    Err(Rem6CliError::Execute {
        error: format!(
            "GUPS memory start 0x{:x} is not aligned to element size {}",
            config.memory_start(),
            GUPS_ELEMENT_BYTES
        ),
    })
}

fn validate_gups_tick_budget(config: &Rem6GupsConfig) -> Result<(), Rem6CliError> {
    let expected = expected_gups_final_tick(config)?;
    if expected <= config.max_tick() {
        return Ok(());
    }
    Err(Rem6CliError::Execute {
        error: format!(
            "GUPS expected final tick {} exceeds max tick {}",
            expected,
            config.max_tick()
        ),
    })
}

fn expected_gups_final_tick(config: &Rem6GupsConfig) -> Result<u64, Rem6CliError> {
    let updates = config.updates().min(gups_default_update_target(config)?);
    let scheduled_requests = updates
        .checked_mul(2)
        .ok_or_else(|| execute_error("GUPS scheduled request count overflow"))?;
    let request_cycle = config
        .memory_route_delay()
        .checked_mul(2)
        .and_then(|delay| delay.checked_add(1))
        .ok_or_else(|| execute_error("GUPS request cycle overflow"))?;
    scheduled_requests
        .checked_mul(request_cycle)
        .ok_or_else(|| execute_error("GUPS expected final tick overflow"))
}

fn gups_default_update_target(config: &Rem6GupsConfig) -> Result<u64, Rem6CliError> {
    config
        .memory_size()
        .checked_div(GUPS_ELEMENT_BYTES)
        .and_then(|table_size| table_size.checked_mul(4))
        .ok_or_else(|| execute_error("GUPS default update target overflow"))
}

fn gups_controller(config: TrafficGupsConfig) -> Result<TrafficController, Rem6CliError> {
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
                    .map_err(execute_error)?,
            ),
            TrafficTransition::new(
                GUPS_IDLE_STATE,
                GUPS_IDLE_STATE,
                TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
                    .map_err(execute_error)?,
            ),
        ],
    )
    .map_err(execute_error)?;
    let gups = TrafficControllerState::new(
        GUPS_STATE,
        TrafficStateGenerator::Gups(GupsTrafficGenerator::new(config)),
    );
    let idle = TrafficControllerState::new(
        GUPS_IDLE_STATE,
        TrafficStateGenerator::Idle(TrafficIdleGenerator::new(TrafficIdleConfig::new(u64::MAX))),
    );
    Ok(TrafficController::new(
        TrafficControllerConfig::new(graph, vec![gups, idle]).map_err(execute_error)?,
    ))
}

fn build_gups_memory_store(
    config: &Rem6GupsConfig,
    line_layout: CacheLineLayout,
) -> Result<PartitionedMemoryStore, Rem6CliError> {
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(GUPS_MEMORY_TARGET, line_layout)
        .map_err(execute_error)?;
    store
        .map_region(
            GUPS_MEMORY_TARGET,
            Address::new(config.memory_start()),
            AccessSize::new(config.memory_size()).map_err(execute_error)?,
        )
        .map_err(execute_error)?;
    insert_gups_zero_lines(&mut store, config, line_layout)?;
    Ok(store)
}

fn insert_gups_zero_lines(
    store: &mut PartitionedMemoryStore,
    config: &Rem6GupsConfig,
    line_layout: CacheLineLayout,
) -> Result<(), Rem6CliError> {
    let end = config
        .memory_start()
        .checked_add(config.memory_size())
        .ok_or_else(|| execute_error("GUPS memory range overflow"))?;
    let mut line = line_layout.line_address(Address::new(config.memory_start()));
    let final_line = line_layout.line_address(Address::new(end - 1));
    loop {
        store
            .insert_line(
                GUPS_MEMORY_TARGET,
                line,
                vec![0; line_layout.bytes() as usize],
            )
            .map_err(execute_error)?;
        if line == final_line {
            break;
        }
        line = Address::new(line.get() + line_layout.bytes());
    }
    Ok(())
}
