use std::cmp;
use std::collections::BTreeMap;
use std::path::Path;

use rem6_boot::BootImage;
use rem6_coherence::ParallelCoherenceRunSummary;
use rem6_dram::{DramMemoryActivityProfile, DramTargetActivity};
use rem6_memory::{AccessSize, Address, AddressRange, CacheLineLayout, MemoryTargetId};
use rem6_system::RiscvWorkloadReplay;
use rem6_traffic::TrafficTrace;
use rem6_workload::{
    WorkloadAcquiredResource, WorkloadAcquiredSuiteResource, WorkloadDataCacheProtocol,
    WorkloadHostPlacement, WorkloadId, WorkloadManifest, WorkloadMemoryRoute, WorkloadMemoryTarget,
    WorkloadParallelExecutionSummary, WorkloadReplayPlan, WorkloadResolvedResources,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResourcePayload,
    WorkloadRiscvDataCache, WorkloadRouteFabric, WorkloadRouteId, WorkloadTopology,
    WorkloadTrafficTraceReplayRun,
};
use sha2::{Digest, Sha256};

use crate::cli_output::emit_cli_output;
use crate::config::{Rem6TraceReplayConfig, StatsFormat};
use crate::formatting::bytes_to_hex;
use crate::guest_memory::build_cli_dram_profile;
use crate::resource_acquire_cli::{
    acquire_manifest_required_resources, acquire_suite_required_resources,
    reject_runtime_remote_uri_resources,
};
use crate::resource_acquire_config::Rem6ResourceAcquireConfig;
use crate::stats_output::{trace_replay_stats_output, Rem6TraceReplayStatsInputs};
use crate::{execute_error, Rem6CliError};

const TRACE_RESOURCE_ID: &str = "trace";
const TRACE_SOURCE_ENDPOINT: &str = "trace.source";
const TRACE_TARGET_ENDPOINT: &str = "memory";
const TRACE_DATA_CACHE_ENDPOINT: &str = "trace.dcache";
const TRACE_SOURCE_PARTITION: u32 = 0;
const TRACE_TARGET_PARTITION: u32 = 1;
const TRACE_HOST_SOURCE: u32 = 51;
const TRACE_HOST_LATENCY: u64 = 2;
const TRACE_PARALLEL_WORKERS: usize = 2;
const TRACE_REPLAY_MAX_TURNS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6TraceReplayArtifact {
    pub(crate) schema: &'static str,
    pub(crate) config: Rem6TraceReplayConfig,
    pub(crate) trace_digest: String,
    pub(crate) execution: Rem6TraceReplayExecutionSummary,
    pub(crate) stats_json: String,
    pub(crate) stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6TraceReplayExecutionSummary {
    pub(crate) final_tick: u64,
    pub(crate) summary: rem6_workload::WorkloadTrafficTraceReplaySummary,
    pub(crate) parallel_summary: WorkloadParallelExecutionSummary,
    pub(crate) data_cache_dram_summary: WorkloadParallelExecutionSummary,
    pub(crate) data_cache_dram_accesses: usize,
}

pub(crate) fn run_trace_replay_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6TraceReplayConfig::parse_args(args)?;
    let artifact = run_trace_replay_config(config)?;
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
        &[],
    )
}

pub fn run_trace_replay_config(
    config: Rem6TraceReplayConfig,
) -> Result<Rem6TraceReplayArtifact, Rem6CliError> {
    let trace_resource = WorkloadResourceId::new(TRACE_RESOURCE_ID).map_err(execute_error)?;
    let trace_payload = trace_replay_payload(&config, &trace_resource)?;
    let trace = TrafficTrace::from_gem5_packet_trace(&trace_payload, config.tick_frequency())
        .map_err(execute_error)?;
    let trace_max_tick = trace.max_tick().unwrap_or(0);
    if trace_max_tick > config.max_tick() {
        return Err(Rem6CliError::Execute {
            error: format!(
                "trace replay final tick {} exceeds max tick {}",
                trace_max_tick,
                config.max_tick()
            ),
        });
    }
    let trace_digest = trace_payload_digest(&trace_payload);
    let route = WorkloadRouteId::new(config.route()).map_err(execute_error)?;
    let manifest = trace_replay_manifest(
        &config,
        &route,
        &trace,
        &trace_resource,
        &trace_digest,
        trace_max_tick.saturating_add(1),
    )?;
    let resolved = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(trace_resource, trace_digest.clone(), trace_payload)
                .map_err(execute_error)?,
        ],
    )
    .map_err(execute_error)?;
    let plan = WorkloadReplayPlan::from_manifest(&manifest).map_err(execute_error)?;
    let outcome = RiscvWorkloadReplay::new(plan)
        .with_resolved_resources(resolved)
        .with_max_turns(TRACE_REPLAY_MAX_TURNS)
        .run_parallel()
        .map_err(execute_error)?;
    let summary = outcome
        .result()
        .traffic_trace_replay_summary(&route)
        .cloned()
        .ok_or_else(|| execute_error("trace replay summary missing"))?;
    let parallel_summary = outcome
        .result()
        .parallel_execution_summary()
        .cloned()
        .ok_or_else(|| execute_error("trace replay parallel summary missing"))?;
    let final_tick = outcome
        .run()
        .final_tick()
        .ok_or_else(|| execute_error("trace replay final tick missing"))?;
    if final_tick > config.max_tick() {
        return Err(Rem6CliError::Execute {
            error: format!(
                "trace replay final tick {} exceeds max tick {}",
                final_tick,
                config.max_tick()
            ),
        });
    }
    let data_cache_runs = outcome.run().data_cache_runs();
    let data_cache_dram_summary = data_cache_dram_summary(data_cache_runs);
    let execution = Rem6TraceReplayExecutionSummary {
        final_tick,
        summary,
        parallel_summary,
        data_cache_dram_accesses: data_cache_runs
            .iter()
            .map(|run| run.dram_access_count())
            .sum(),
        data_cache_dram_summary,
    };
    let stats = trace_replay_stats_output(Rem6TraceReplayStatsInputs {
        config: &config,
        execution: &execution,
    })?;

    Ok(Rem6TraceReplayArtifact {
        schema: "rem6.cli.trace_replay.v1",
        config,
        trace_digest,
        execution,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

fn trace_replay_payload(
    config: &Rem6TraceReplayConfig,
    trace_resource: &WorkloadResourceId,
) -> Result<Vec<u8>, Rem6CliError> {
    if let Some(resource_config) = config.resource_config() {
        let acquire_config = Rem6ResourceAcquireConfig::parse_args([
            "resource-acquire".to_string(),
            "--config".to_string(),
            resource_config.display().to_string(),
        ])?;
        reject_runtime_remote_uri_resources("trace-replay", resource_config, &acquire_config)?;
        if acquire_config.suite_id().is_some() {
            let (_plan, acquired) = acquire_suite_required_resources(&acquire_config)?;
            let mut trace_resources = if let Some(selector) = config.trace_resource() {
                acquired
                    .into_iter()
                    .filter(|resource| {
                        resource.workload_id().as_str() == selector.workload_id()
                            && resource.acquired().resource().as_str() == selector.resource_id()
                    })
                    .collect::<Vec<_>>()
            } else {
                acquired
                    .into_iter()
                    .filter(|resource| resource.acquired().resource() == trace_resource)
                    .collect::<Vec<_>>()
            };
            if trace_resources.len() != 1 {
                return Err(Rem6CliError::Execute {
                    error: format!(
                        "trace replay suite resource config {} acquired {} required trace resources; expected exactly one",
                        resource_config.display(),
                        trace_resources.len(),
                    ),
                });
            }
            return trace_payload_from_suite_resource(resource_config, trace_resources.remove(0));
        }
        if let Some(selector) = config.trace_resource() {
            return Err(Rem6CliError::Execute {
                error: format!(
                    "trace replay suite resource {} requires suite resource_config",
                    selector.qualified_id()
                ),
            });
        }
        let (_manifest, acquired) = acquire_manifest_required_resources(&acquire_config)?;
        let resource = acquired
            .into_iter()
            .find(|resource| resource.resource() == trace_resource)
            .ok_or_else(|| Rem6CliError::Execute {
                error: format!(
                    "trace replay resource config {} did not acquire required trace resource",
                    resource_config.display()
                ),
            })?;
        return trace_payload_from_resource(resource_config, resource);
    }

    std::fs::read(config.trace()).map_err(|error| Rem6CliError::ReadBinary {
        path: config.trace().to_path_buf(),
        error: error.to_string(),
    })
}

fn trace_payload_from_resource(
    resource_config: &Path,
    resource: WorkloadAcquiredResource,
) -> Result<Vec<u8>, Rem6CliError> {
    let resource_id = resource.resource().as_str().to_string();
    let kind = resource.kind();
    if kind != WorkloadResourceKind::Input {
        return Err(Rem6CliError::Execute {
            error: format!(
                "trace resource {resource_id} in trace replay resource config {} has kind {}; expected input",
                resource_config.display(),
                kind.as_str(),
            ),
        });
    }
    Ok(resource.into_payload().data().to_vec())
}

fn trace_payload_from_suite_resource(
    resource_config: &Path,
    resource: WorkloadAcquiredSuiteResource,
) -> Result<Vec<u8>, Rem6CliError> {
    let workload_id = resource.workload_id().as_str().to_string();
    let resource_id = resource.acquired().resource().as_str().to_string();
    let kind = resource.acquired().kind();
    if kind != WorkloadResourceKind::Input {
        return Err(Rem6CliError::Execute {
            error: format!(
                "trace suite resource {workload_id}/{resource_id} in trace replay resource config {} has kind {}; expected input",
                resource_config.display(),
                kind.as_str(),
            ),
        });
    }
    Ok(resource.into_acquired().into_payload().data().to_vec())
}

fn trace_replay_manifest(
    config: &Rem6TraceReplayConfig,
    route: &WorkloadRouteId,
    trace: &TrafficTrace,
    trace_resource: &WorkloadResourceId,
    trace_digest: &str,
    duration: u64,
) -> Result<WorkloadManifest, Rem6CliError> {
    let memory_range = AddressRange::new(
        Address::new(config.memory_start()),
        AccessSize::new(config.memory_size()).map_err(execute_error)?,
    )
    .map_err(execute_error)?;
    let line_layout = CacheLineLayout::new(config.line_bytes()).map_err(execute_error)?;
    let memory_target = trace_replay_memory_target(config, memory_range, line_layout)?;
    let topology = WorkloadTopology::new(
        trace_partition_count(config.control_partition()),
        config.min_remote_delay(),
        TRACE_PARALLEL_WORKERS,
        WorkloadHostPlacement::new(
            trace_host_partition(config.control_partition()),
            TRACE_HOST_LATENCY,
            TRACE_HOST_SOURCE,
        )
        .map_err(execute_error)?,
    )
    .map_err(execute_error)?
    .add_memory_target(memory_target)
    .map_err(execute_error)?;
    let topology = match config.data_cache_protocol() {
        Some(protocol) => {
            trace_replay_data_cache_topology(config, route, trace, topology, protocol)?
        }
        None => trace_replay_memory_topology(config, route, topology)?,
    };
    let mut trace_replay = WorkloadTrafficTraceReplayRun::new(
        route.clone(),
        trace_resource.clone(),
        config.tick_frequency(),
        config.agent(),
        config.line_bytes(),
        duration,
        config.control_partition(),
    );
    if config.data_cache_protocol().is_some() {
        trace_replay = trace_replay.with_data_cache();
    }

    WorkloadManifest::builder(
        WorkloadId::new("cli-trace-replay").map_err(execute_error)?,
        trace_replay_boot_image(config, trace)?,
    )
    .add_resource(
        WorkloadResource::new(
            trace_resource.clone(),
            WorkloadResourceKind::Input,
            trace_digest,
            config.trace_input(),
        )
        .map_err(execute_error)?,
    )
    .map_err(execute_error)?
    .with_topology(topology)
    .with_expected_stop_reason("idle")
    .add_traffic_trace_replay(trace_replay)
    .map_err(execute_error)?
    .build()
    .map_err(execute_error)
}

fn trace_replay_memory_target(
    config: &Rem6TraceReplayConfig,
    memory_range: AddressRange,
    line_layout: CacheLineLayout,
) -> Result<WorkloadMemoryTarget, Rem6CliError> {
    let target =
        WorkloadMemoryTarget::new(0, config.line_bytes(), memory_range).map_err(execute_error)?;
    match config.data_cache_dram_memory_profile() {
        Some(profile) => target
            .with_external_memory_profile(build_cli_dram_profile(line_layout, profile)?)
            .map_err(execute_error),
        None => Ok(target),
    }
}

fn trace_replay_memory_topology(
    config: &Rem6TraceReplayConfig,
    route: &WorkloadRouteId,
    topology: WorkloadTopology,
) -> Result<WorkloadTopology, Rem6CliError> {
    topology
        .add_memory_route(trace_replay_memory_route(
            route.clone(),
            TRACE_SOURCE_ENDPOINT,
            TRACE_SOURCE_PARTITION,
            config,
        )?)
        .map_err(execute_error)
}

fn trace_replay_data_cache_topology(
    config: &Rem6TraceReplayConfig,
    route: &WorkloadRouteId,
    trace: &TrafficTrace,
    topology: WorkloadTopology,
    protocol: WorkloadDataCacheProtocol,
) -> Result<WorkloadTopology, Rem6CliError> {
    let backing_route = trace_replay_backing_route(route)?;
    let line_layout = CacheLineLayout::new(config.line_bytes()).map_err(execute_error)?;
    let mut line_addresses = trace.line_addresses(line_layout);
    if line_addresses.is_empty() {
        line_addresses.push(Address::new(config.memory_start()));
    }
    let first_line = line_addresses[0];
    let data_cache = line_addresses.iter().copied().skip(1).fold(
        WorkloadRiscvDataCache::new(
            protocol,
            0,
            first_line,
            TRACE_TARGET_PARTITION,
            TRACE_DATA_CACHE_ENDPOINT,
            backing_route.clone(),
        )
        .map_err(execute_error)?,
        WorkloadRiscvDataCache::with_line_address,
    );

    topology
        .add_memory_route(trace_replay_memory_route(
            route.clone(),
            TRACE_SOURCE_ENDPOINT,
            TRACE_SOURCE_PARTITION,
            config,
        )?)
        .map_err(execute_error)?
        .add_memory_route(trace_replay_memory_route(
            backing_route,
            TRACE_DATA_CACHE_ENDPOINT,
            TRACE_TARGET_PARTITION,
            config,
        )?)
        .map_err(execute_error)?
        .with_riscv_data_cache(data_cache)
        .map_err(execute_error)
}

fn trace_replay_memory_route(
    route: WorkloadRouteId,
    source_endpoint: &str,
    source_partition: u32,
    config: &Rem6TraceReplayConfig,
) -> Result<WorkloadMemoryRoute, Rem6CliError> {
    let route = WorkloadMemoryRoute::new(
        route,
        source_endpoint,
        source_partition,
        TRACE_TARGET_ENDPOINT,
        TRACE_TARGET_PARTITION,
        config.memory_route_delay(),
        config.memory_route_delay(),
    )
    .map_err(execute_error)?;
    Ok(match trace_replay_fabric_route(config)? {
        Some(fabric) => route.with_fabric(fabric),
        None => route,
    })
}

fn trace_replay_fabric_route(
    config: &Rem6TraceReplayConfig,
) -> Result<Option<WorkloadRouteFabric>, Rem6CliError> {
    let Some(link) = config.fabric_link() else {
        return Ok(None);
    };
    let bandwidth =
        config
            .fabric_bandwidth_bytes_per_tick()
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--fabric-bandwidth-bytes-per-tick",
            })?;
    let fabric = WorkloadRouteFabric::new(link, bandwidth)
        .map_err(execute_error)?
        .with_virtual_networks(
            config.fabric_request_virtual_network(),
            config.fabric_response_virtual_network(),
        );
    match config.fabric_credit_depth() {
        Some(credit_depth) => fabric
            .with_credit_depth(credit_depth)
            .map(Some)
            .map_err(execute_error),
        None => Ok(Some(fabric)),
    }
}

fn trace_replay_backing_route(route: &WorkloadRouteId) -> Result<WorkloadRouteId, Rem6CliError> {
    WorkloadRouteId::new(format!("{}.dcache.backing", route.as_str())).map_err(execute_error)
}

fn trace_replay_boot_image(
    config: &Rem6TraceReplayConfig,
    trace: &TrafficTrace,
) -> Result<BootImage, Rem6CliError> {
    let entry = Address::new(config.memory_start());
    let mut boot = BootImage::new(entry);
    if config.data_cache_protocol().is_none() {
        return Ok(boot);
    }

    let line_layout = CacheLineLayout::new(config.line_bytes()).map_err(execute_error)?;
    let line_bytes = usize::try_from(config.line_bytes()).map_err(|_| Rem6CliError::Execute {
        error: format!(
            "trace replay line bytes {} exceed host size",
            config.line_bytes()
        ),
    })?;
    let boot_line = line_layout.line_address(entry);
    let mut line_addresses = trace.line_addresses(line_layout);
    if !line_addresses.contains(&boot_line) {
        line_addresses.push(boot_line);
        line_addresses.sort_by_key(|address| address.get());
    }
    for line in line_addresses {
        let data = vec![0; line_bytes];
        boot = boot.add_segment(line, data).map_err(execute_error)?;
    }
    Ok(boot)
}

fn trace_partition_count(control_partition: u32) -> u32 {
    cmp::max(4, control_partition.saturating_add(2))
}

fn trace_host_partition(control_partition: u32) -> u32 {
    trace_partition_count(control_partition) - 1
}

fn trace_payload_digest(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    format!("sha256:{}", bytes_to_hex(&digest))
}

fn data_cache_dram_summary(
    data_cache_runs: &[ParallelCoherenceRunSummary],
) -> WorkloadParallelExecutionSummary {
    let mut activities = BTreeMap::<MemoryTargetId, DramTargetActivity>::new();
    for run in data_cache_runs {
        for (target, activity) in run.dram_target_activities() {
            activities
                .entry(target)
                .and_modify(|stored| {
                    *stored = stored.clone().merge_window(activity.clone());
                })
                .or_insert(activity);
        }
    }
    dram_profile_summary(&DramMemoryActivityProfile::from_target_activities(
        activities.values(),
    ))
}

fn dram_profile_summary(profile: &DramMemoryActivityProfile) -> WorkloadParallelExecutionSummary {
    WorkloadParallelExecutionSummary::default().with_dram_activity(
        profile.active_target_count(),
        profile.active_port_count(),
        profile.active_bank_count(),
        profile.access_count(),
        profile.read_count(),
        profile.write_count(),
        profile.row_hit_count(),
        profile.row_miss_count(),
        profile.command_count(),
        profile.turnaround_count(),
        profile.total_ready_latency_cycles(),
        profile.max_ready_latency_cycles(),
    )
}

impl Rem6TraceReplayExecutionSummary {
    pub(crate) const fn final_tick(&self) -> u64 {
        self.final_tick
    }

    pub(crate) const fn summary(&self) -> &rem6_workload::WorkloadTrafficTraceReplaySummary {
        &self.summary
    }

    pub(crate) const fn parallel_summary(&self) -> &WorkloadParallelExecutionSummary {
        &self.parallel_summary
    }

    pub(crate) const fn data_cache_dram_summary(&self) -> &WorkloadParallelExecutionSummary {
        &self.data_cache_dram_summary
    }

    pub(crate) const fn data_cache_dram_accesses(&self) -> usize {
        self.data_cache_dram_accesses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_replay_fabric_route_preserves_virtual_networks_and_credit_depth() {
        let config = Rem6TraceReplayConfig::parse_args([
            "trace-replay",
            "--trace",
            "trace.pb",
            "--route",
            "cpu0.fetch",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "4",
            "--fabric-request-virtual-network",
            "3",
            "--fabric-response-virtual-network",
            "4",
            "--fabric-credit-depth",
            "2",
        ])
        .unwrap();

        let fabric = trace_replay_fabric_route(&config).unwrap().unwrap();

        assert_eq!(fabric.link(), "cpu_mem");
        assert_eq!(fabric.bandwidth_bytes_per_tick(), 4);
        assert_eq!(fabric.request_virtual_network(), 3);
        assert_eq!(fabric.response_virtual_network(), 4);
        assert_eq!(fabric.credit_depth(), Some(2));
    }
}
