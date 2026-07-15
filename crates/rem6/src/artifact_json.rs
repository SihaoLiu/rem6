use super::formatting::{bytes_to_hex, json_escape};
use super::{
    CliDataCacheSummary, Rem6CoreSummary, Rem6DataAccessProbeSummary,
    Rem6ExecutionModeQuiescenceGateSummary, Rem6ExecutionModeStateTransferSummary,
    Rem6ExecutionStop, Rem6ExecutionSummary, Rem6GuestHostCallSummary, Rem6GupsArtifact,
    Rem6GupsExecutionSummary, Rem6HostActionSummary, Rem6HostCheckpointComponentSummary,
    Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary, Rem6HostExecutionModeSwitchSummary,
    Rem6HostInjectedCommandSummary, Rem6HostStatsDumpSampleSummary, Rem6HostStatsDumpSummary,
    Rem6HostStatsResetSummary, Rem6HostStopActionSummary, Rem6HostWorkMarkerSummary,
    Rem6InstructionProbeSummary, Rem6MemoryDump, Rem6ParallelFrontierSummary,
    Rem6ParallelPartitionSummary, Rem6ParallelReadyPartitionSummary, Rem6PcCountPairSummary,
    Rem6PcCountTrackerSummary, Rem6RiscvGuestWriteSummary, Rem6RiscvSbiConsoleSummary,
    Rem6RiscvSbiHsmStatusSummary, Rem6RiscvSbiHsmSummary, Rem6RiscvSbiHsmWakeSummary,
    Rem6RiscvSbiIpiSummary, Rem6RiscvSbiResetSummary, Rem6RiscvSbiRfenceCompletionSummary,
    Rem6RiscvSbiRfenceSummary, Rem6RiscvSbiTimerSummary, Rem6RiscvUnknownSyscallSummary,
    Rem6TraceReplayArtifact, Rem6TraceReplayExecutionSummary,
    Rem6TraceReplayExternalAdapterSummary, RunMemorySystem,
};

use gups::gups_response_stats_json;

use self::fabric::{
    fabric_hop_activities_json, fabric_lane_activities_json, fabric_link_activities_json,
    fabric_wait_for_json_fields,
};

mod checkpoint;
mod dram;
mod fabric;
mod gups;
mod parallel;
mod resources;
mod run;
#[cfg(test)]
mod tests;
mod transport;

pub(super) use self::dram::dram_targets_json;

impl Rem6GupsArtifact {
    pub fn to_json(&self) -> String {
        let memory_dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let profiles = gups_profile_summaries_json(&self.execution.profile_summaries);
        format!(
            "{{\"schema\":\"{}\",\"generator\":\"gups\",\"memory_start\":\"0x{:x}\",\"memory_size\":{},\"updates\":{},\"rng_state\":\"0x{:x}\",\"profiles\":[{}],\"simulation\":{},\"memory\":[{}],\"transport\":{{\"gups\":{}}},\"stats\":{}}}\n",
            self.schema,
            self.config.memory_start(),
            self.config.memory_size(),
            self.config.updates(),
            self.config.rng_state(),
            profiles,
            self.execution.to_json(self.config.max_tick()),
            memory_dumps,
            self.transport.to_json(),
            self.stats_json,
        )
    }
}

fn gups_profile_summaries_json(profiles: &[rem6_traffic::TrafficStateProfileSummary]) -> String {
    profiles
        .iter()
        .map(gups_profile_summary_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn gups_profile_summary_json(summary: &rem6_traffic::TrafficStateProfileSummary) -> String {
    let profile = summary.profile();
    let generator_summary = profile.summary();
    format!(
        "{{\"state\":{},\"generator_class\":\"{}\",\"memory_profile\":\"{}\",\"summary\":{{\"packet_count\":{},\"read_count\":{},\"write_count\":{},\"bytes_read\":{},\"bytes_written\":{},\"first_tick\":{},\"last_tick\":{}}}}}",
        summary.state().get(),
        profile.generator_class().as_str(),
        profile.memory_profile().as_str(),
        generator_summary.packet_count(),
        generator_summary.read_count(),
        generator_summary.write_count(),
        generator_summary.bytes_read(),
        generator_summary.bytes_written(),
        optional_tick_json(generator_summary.first_tick()),
        optional_tick_json(generator_summary.last_tick()),
    )
}

impl Rem6TraceReplayArtifact {
    pub fn to_json(&self) -> String {
        let data_cache_protocol = self
            .config
            .data_cache_protocol()
            .map(|protocol| format!("\"{}\"", protocol.as_str()))
            .unwrap_or_else(|| "null".to_string());
        let data_cache_dram_memory_profile = self
            .config
            .data_cache_dram_memory_profile()
            .map(|profile| format!("\"{}\"", profile.as_str()))
            .unwrap_or_else(|| "null".to_string());
        let fabric_link = optional_string_json(self.config.fabric_link());
        let fabric_bandwidth = optional_count_json(self.config.fabric_bandwidth_bytes_per_tick());
        let fabric_request_virtual_network = optional_count_json(
            self.config
                .fabric_link()
                .map(|_| u64::from(self.config.fabric_request_virtual_network())),
        );
        let fabric_response_virtual_network = optional_count_json(
            self.config
                .fabric_link()
                .map(|_| u64::from(self.config.fabric_response_virtual_network())),
        );
        let fabric_credit_depth =
            optional_count_json(self.config.fabric_credit_depth().map(u64::from));
        let fabric_router_stage = self
            .config
            .fabric_router_stage()
            .map(|stage| {
                format!(
                    "{{\"router\":\"{}\",\"input_port\":{},\"output_port\":{},\"virtual_channel\":{},\"latency_ticks\":{}}}",
                    json_escape(stage.router()),
                    stage.input_port(),
                    stage.output_port(),
                    stage.virtual_channel(),
                    stage.latency(),
                )
            })
            .unwrap_or_else(|| "null".to_string());
        let trace_resource = self
            .config
            .trace_resource()
            .map(|selector| format!("\"{}\"", json_escape(&selector.source_name())))
            .unwrap_or_else(|| "null".to_string());
        let external_adapter = self
            .external_adapter
            .as_ref()
            .map(Rem6TraceReplayExternalAdapterSummary::to_json)
            .unwrap_or_else(|| "null".to_string());
        let power_analysis = self
            .power_analysis
            .as_ref()
            .map(crate::power_output::Rem6PowerAnalysisArtifact::to_json)
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"schema\":\"{}\",\"generator\":\"trace-replay\",\"trace\":\"{}\",\"trace_resource\":{},\"trace_digest\":\"{}\",\"route\":\"{}\",\"memory_start\":\"0x{:x}\",\"memory_size\":{},\"tick_frequency\":{},\"line_bytes\":{},\"agent\":{},\"control_partition\":{},\"data_cache_protocol\":{},\"data_cache_dram_memory_profile\":{},\"fabric_link\":{},\"fabric_bandwidth_bytes_per_tick\":{},\"fabric_request_virtual_network\":{},\"fabric_response_virtual_network\":{},\"fabric_credit_depth\":{},\"fabric_router_stage\":{},\"external_adapter\":{},\"simulation\":{},\"summary\":{},\"power_analysis\":{},\"stats\":{}}}\n",
            self.schema,
            json_escape(&self.config.trace_input()),
            trace_resource,
            json_escape(&self.trace_digest),
            json_escape(self.config.route()),
            self.config.memory_start(),
            self.config.memory_size(),
            self.config.tick_frequency(),
            self.config.line_bytes(),
            self.config.agent(),
            self.config.control_partition(),
            data_cache_protocol,
            data_cache_dram_memory_profile,
            fabric_link,
            fabric_bandwidth,
            fabric_request_virtual_network,
            fabric_response_virtual_network,
            fabric_credit_depth,
            fabric_router_stage,
            external_adapter,
            self.execution.to_json(self.config.max_tick()),
            traffic_trace_summary_json(
                self.execution.summary(),
                self.execution.parallel_summary(),
                self.execution.data_cache(),
                self.execution.data_cache_dram_summary(),
                self.execution.data_cache_dram_accesses(),
            ),
            power_analysis,
            self.stats_json,
        )
    }
}

impl Rem6TraceReplayExternalAdapterSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"{}\",\"endpoint\":\"{}\",\"events\":{},\"completed_events\":{},\"pending_events\":{},\"checkpoint_endpoints\":{},\"checkpoint_completed_events\":{},\"restored_endpoints\":{},\"restored_completed_events\":{},\"restored_pending_events\":{},\"runtime_restores\":{},\"post_restore_completed_events\":{},\"first_tick\":{},\"last_tick\":{}}}",
            self.kind.as_str(),
            json_escape(&self.endpoint),
            self.events,
            self.completed_events,
            self.pending_events,
            self.checkpoint_endpoints,
            self.checkpoint_completed_events,
            self.restored_endpoints,
            self.restored_completed_events,
            self.restored_pending_events,
            self.runtime_restores,
            self.post_restore_completed_events,
            optional_tick_json(self.first_tick),
            optional_tick_json(self.last_tick),
        )
    }
}

impl Rem6TraceReplayExecutionSummary {
    fn to_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"completed\",\"max_tick\":{},\"final_tick\":{},\"checkpoint_count\":{},\"checkpoint_restored_count\":{},\"checkpoint_component_count\":{},\"checkpoint_chunk_count\":{},\"checkpoint_payload_bytes\":{},\"checkpoint_restored_component_count\":{},\"checkpoint_restored_chunk_count\":{},\"checkpoint_restored_payload_bytes\":{}}}",
            max_tick,
            self.final_tick(),
            self.host_actions().checkpoint_count(),
            self.host_actions().checkpoint_restore_count(),
            self.checkpoint_component_count(),
            self.checkpoint_chunk_count(),
            self.checkpoint_payload_bytes(),
            self.checkpoint_restored_component_count(),
            self.checkpoint_restored_chunk_count(),
            self.checkpoint_restored_payload_bytes(),
        )
    }
}

impl Rem6GupsExecutionSummary {
    fn to_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"completed\",\"max_tick\":{},\"final_tick\":{},\"scheduled_requests\":{},\"response_stats\":{}}}",
            max_tick,
            self.final_tick,
            self.scheduled_requests,
            gups_response_stats_json(&self.response_stats),
        )
    }
}

fn traffic_trace_summary_json(
    summary: &rem6_workload::WorkloadTrafficTraceReplaySummary,
    parallel_summary: &rem6_workload::WorkloadParallelExecutionSummary,
    data_cache: &CliDataCacheSummary,
    data_cache_dram_summary: &rem6_workload::WorkloadParallelExecutionSummary,
    data_cache_dram_accesses: usize,
) -> String {
    let mut fields = vec![format!(
        "\"route\":\"{}\"",
        json_escape(summary.route().as_str())
    )];
    push_json_usize(&mut fields, "scheduled_count", summary.scheduled_count());
    push_json_usize(
        &mut fields,
        "response_delivery_count",
        summary.response_delivery_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_completed_response_count",
        summary.trace_completed_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_retry_response_count",
        summary.trace_retry_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_store_conditional_failed_response_count",
        summary.trace_store_conditional_failed_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_read_response_count",
        summary.trace_read_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_write_response_count",
        summary.trace_write_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_prefetch_response_count",
        summary.trace_prefetch_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_invalidate_response_count",
        summary.trace_invalidate_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_clean_response_count",
        summary.trace_clean_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_upgrade_response_count",
        summary.trace_upgrade_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_llsc_response_count",
        summary.trace_llsc_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_locked_rmw_response_count",
        summary.trace_locked_rmw_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_writable_intent_response_count",
        summary.trace_writable_intent_response_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_response_data_byte_count",
        summary.trace_response_data_byte_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_response_fill_data_byte_count",
        summary.trace_response_fill_data_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_trace_event_count",
        summary.memory_trace_event_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_write_completion_count",
        summary.memory_write_completion_count(),
    );
    push_json_u64(
        &mut fields,
        "memory_write_completion_byte_count",
        summary.memory_write_completion_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "active_fabric_lane_count",
        parallel_summary.active_fabric_lane_count(),
    );
    push_json_usize(
        &mut fields,
        "active_fabric_virtual_network_count",
        parallel_summary.active_fabric_virtual_network_count(),
    );
    push_json_usize(
        &mut fields,
        "fabric_transfer_count",
        parallel_summary.fabric_transfer_count(),
    );
    push_json_u64(
        &mut fields,
        "fabric_byte_count",
        parallel_summary.fabric_byte_count(),
    );
    push_json_u64(
        &mut fields,
        "fabric_flit_count",
        parallel_summary.fabric_flit_count(),
    );
    push_json_u64(
        &mut fields,
        "fabric_occupied_ticks",
        parallel_summary.fabric_occupied_ticks(),
    );
    push_json_u64(
        &mut fields,
        "fabric_queue_delay_ticks",
        parallel_summary.fabric_queue_delay_ticks(),
    );
    push_json_u64(
        &mut fields,
        "fabric_max_queue_delay_ticks",
        parallel_summary.fabric_max_queue_delay_ticks(),
    );
    push_json_u64(
        &mut fields,
        "fabric_credit_delay_ticks",
        parallel_summary.fabric_credit_delay_ticks(),
    );
    push_json_u64(
        &mut fields,
        "fabric_max_credit_delay_ticks",
        parallel_summary.fabric_max_credit_delay_ticks(),
    );
    push_json_usize(
        &mut fields,
        "contended_fabric_lane_count",
        parallel_summary.contended_fabric_lane_count(),
    );
    push_json_usize(
        &mut fields,
        "data_cache_dram_accesses",
        data_cache_dram_accesses,
    );
    push_json_u64(
        &mut fields,
        "data_cache_cpu_responses",
        data_cache.cpu_responses,
    );
    push_json_u64(
        &mut fields,
        "data_cache_directory_decisions",
        data_cache.directory_decisions,
    );
    push_json_usize(
        &mut fields,
        "active_dram_target_count",
        data_cache_dram_summary.active_dram_target_count(),
    );
    push_json_usize(
        &mut fields,
        "active_dram_port_count",
        data_cache_dram_summary.active_dram_port_count(),
    );
    push_json_usize(
        &mut fields,
        "active_dram_bank_count",
        data_cache_dram_summary.active_dram_bank_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_access_count",
        data_cache_dram_summary.dram_access_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_read_count",
        data_cache_dram_summary.dram_read_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_write_count",
        data_cache_dram_summary.dram_write_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_row_hit_count",
        data_cache_dram_summary.dram_row_hit_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_row_miss_count",
        data_cache_dram_summary.dram_row_miss_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_command_count",
        data_cache_dram_summary.dram_command_count(),
    );
    push_json_usize(
        &mut fields,
        "dram_turnaround_count",
        data_cache_dram_summary.dram_turnaround_count(),
    );
    push_json_u64(
        &mut fields,
        "dram_total_ready_latency_cycles",
        data_cache_dram_summary.dram_total_ready_latency_cycles(),
    );
    push_json_u64(
        &mut fields,
        "dram_max_ready_latency_cycles",
        data_cache_dram_summary.dram_max_ready_latency_cycles(),
    );
    fields.push(format!(
        "\"fabric_link_activities\":[{}]",
        fabric_link_activities_json(parallel_summary)
    ));
    fields.push(format!(
        "\"fabric_lane_activities\":[{}]",
        fabric_lane_activities_json(parallel_summary)
    ));
    fields.push(format!(
        "\"fabric_hop_activities\":[{}]",
        fabric_hop_activities_json(parallel_summary)
    ));
    fields.extend(fabric_wait_for_json_fields(parallel_summary));
    push_json_usize(
        &mut fields,
        "trace_data_cache_response_count",
        summary.trace_data_cache_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_maintenance_response_count",
        summary.trace_data_cache_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_clean_maintenance_response_count",
        summary.trace_data_cache_clean_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_invalidate_maintenance_response_count",
        summary.trace_data_cache_invalidate_maintenance_response_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_error_count",
        summary.trace_data_cache_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_invalid_destination_error_count",
        summary.trace_data_cache_invalid_destination_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_bad_address_error_count",
        summary.trace_data_cache_bad_address_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_read_error_count",
        summary.trace_data_cache_read_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_write_error_count",
        summary.trace_data_cache_write_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_functional_read_error_count",
        summary.trace_data_cache_functional_read_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_data_cache_functional_write_error_count",
        summary.trace_data_cache_functional_write_error_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_count",
        summary.memory_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_invalid_destination_count",
        summary.memory_failure_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_bad_address_count",
        summary.memory_failure_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_read_count",
        summary.memory_failure_read_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_write_count",
        summary.memory_failure_write_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_functional_read_count",
        summary.memory_failure_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "memory_failure_functional_write_count",
        summary.memory_failure_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_count",
        summary.trace_error_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_invalid_destination_count",
        summary.trace_error_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_bad_address_count",
        summary.trace_error_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_read_count",
        summary.trace_error_read_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_write_count",
        summary.trace_error_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_functional_read_count",
        summary.trace_error_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_error_functional_write_count",
        summary.trace_error_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_access_count",
        summary.trace_htm_access_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_begin_count",
        summary.trace_htm_begin_count(),
    );
    push_json_usize(
        &mut fields,
        "control_ack_count",
        summary.control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "sync_control_ack_count",
        summary.sync_control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_control_ack_count",
        summary.htm_control_ack_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_count",
        summary.control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_invalid_destination_count",
        summary.control_failure_invalid_destination_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_bad_address_count",
        summary.control_failure_bad_address_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_read_count",
        summary.control_failure_read_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_write_count",
        summary.control_failure_write_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_functional_read_count",
        summary.control_failure_functional_read_count(),
    );
    push_json_usize(
        &mut fields,
        "control_failure_functional_write_count",
        summary.control_failure_functional_write_count(),
    );
    push_json_usize(
        &mut fields,
        "sync_control_failure_count",
        summary.sync_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "tlb_control_failure_count",
        summary.tlb_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "cache_control_failure_count",
        summary.cache_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_control_failure_count",
        summary.htm_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "diagnostic_control_failure_count",
        summary.diagnostic_control_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "sideband_event_count",
        summary.sideband_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_sideband_failure_count",
        summary.trace_sideband_failure_count(),
    );
    push_json_usize(
        &mut fields,
        "tlb_sync_event_count",
        summary.tlb_sync_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_tlb_sync_count",
        summary.trace_tlb_sync_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_tlb_sync_flushed_entry_count",
        summary.trace_tlb_sync_flushed_entry_count(),
    );
    push_json_usize(
        &mut fields,
        "cache_flush_event_count",
        summary.cache_flush_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_cache_flush_count",
        summary.trace_cache_flush_count(),
    );
    push_json_u64(
        &mut fields,
        "trace_cache_flush_data_byte_count",
        summary.trace_cache_flush_data_byte_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_l1_invalidation_count",
        summary.trace_l1_invalidation_count(),
    );
    push_json_usize(
        &mut fields,
        "diagnostic_print_event_count",
        summary.diagnostic_print_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_diagnostic_count",
        summary.trace_diagnostic_count(),
    );
    push_json_usize(
        &mut fields,
        "htm_abort_event_count",
        summary.htm_abort_event_count(),
    );
    push_json_usize(
        &mut fields,
        "trace_htm_abort_count",
        summary.trace_htm_abort_count(),
    );
    format!("{{{}}}", fields.join(","))
}

fn push_json_usize(fields: &mut Vec<String>, name: &str, value: usize) {
    fields.push(format!("\"{name}\":{value}"));
}

fn push_json_u64(fields: &mut Vec<String>, name: &str, value: u64) {
    fields.push(format!("\"{name}\":{value}"));
}

impl Rem6ExecutionSummary {
    fn to_simulation_json(
        &self,
        max_tick: u64,
        max_instructions: Option<u64>,
        memory_route_delay: u64,
        host_event_delay: u64,
        memory_system: Option<RunMemorySystem>,
    ) -> String {
        let instruction_limit = match self.stop {
            Rem6ExecutionStop::InstructionLimit { instruction_limit } => Some(instruction_limit),
            Rem6ExecutionStop::Idle
            | Rem6ExecutionStop::HostTrap { .. }
            | Rem6ExecutionStop::HostStop { .. }
            | Rem6ExecutionStop::TickLimit { .. } => max_instructions,
        };
        let memory_system = optional_string_json(memory_system.map(RunMemorySystem::as_str));
        let common = format!(
            "\"max_tick\":{},\"instruction_limit\":{},\"memory_system\":{},\"memory_route_delay\":{},\"host_event_delay\":{},\"executed_ticks\":{},\"final_tick\":{},\"cores\":{},\"committed_instructions\":{},\"instruction_probes\":{},\"instruction_cache_runs\":{},\"instruction_cache_msi_runs\":{},\"instruction_cache_mesi_runs\":{},\"instruction_cache_moesi_runs\":{},\"instruction_cache_chi_runs\":{},\"instruction_cache_cpu_responses\":{},\"instruction_cache_directory_decisions\":{},\"instruction_cache_dram_accesses\":{},\"instruction_cache_bank_accepted\":{},\"instruction_cache_bank_immediate_hits\":{},\"instruction_cache_bank_scheduled_misses\":{},\"instruction_cache_bank_coalesced_misses\":{},\"instruction_cache_l2_runs\":{},\"instruction_cache_l2_msi_runs\":{},\"instruction_cache_l2_mesi_runs\":{},\"instruction_cache_l2_moesi_runs\":{},\"instruction_cache_l2_chi_runs\":{},\"instruction_cache_l2_cpu_responses\":{},\"instruction_cache_l2_directory_decisions\":{},\"instruction_cache_l2_dram_accesses\":{},\"instruction_cache_l2_bank_accepted\":{},\"instruction_cache_l2_bank_immediate_hits\":{},\"instruction_cache_l2_bank_scheduled_misses\":{},\"instruction_cache_l2_bank_coalesced_misses\":{},\"instruction_cache_l2_prefetch_fills\":{},\"instruction_cache_l3_runs\":{},\"instruction_cache_l3_msi_runs\":{},\"instruction_cache_l3_mesi_runs\":{},\"instruction_cache_l3_moesi_runs\":{},\"instruction_cache_l3_chi_runs\":{},\"instruction_cache_l3_cpu_responses\":{},\"instruction_cache_l3_directory_decisions\":{},\"instruction_cache_l3_dram_accesses\":{},\"instruction_cache_l3_bank_accepted\":{},\"instruction_cache_l3_bank_immediate_hits\":{},\"instruction_cache_l3_bank_scheduled_misses\":{},\"instruction_cache_l3_bank_coalesced_misses\":{},\"instruction_cache_l3_prefetch_fills\":{},\"instruction_cache_prefetch_identified\":{},\"instruction_cache_prefetch_issued\":{},\"instruction_cache_prefetch_useful\":{},\"instruction_cache_prefetch_useful_but_miss\":{},\"instruction_cache_prefetch_unused\":{},\"instruction_cache_prefetch_demand_mshr_misses\":{},\"instruction_cache_prefetch_hit_in_cache\":{},\"instruction_cache_prefetch_hit_in_mshr\":{},\"instruction_cache_prefetch_hit_in_write_buffer\":{},\"instruction_cache_prefetch_late\":{},\"instruction_cache_prefetch_accuracy_ppm\":{},\"instruction_cache_prefetch_coverage_ppm\":{},\"instruction_cache_prefetch_span_page\":{},\"instruction_cache_prefetch_useful_span_page\":{},\"instruction_cache_prefetch_in_cache\":{},\"instruction_cache_prefetch_fills\":{},\"instruction_cache_prefetch_queue_enqueued\":{},\"instruction_cache_prefetch_queue_issued\":{},\"instruction_cache_prefetch_queue_dropped\":{},\"instruction_cache_prefetch_translation_queue_enqueued\":{},\"instruction_cache_prefetch_translation_queue_issued\":{},\"instruction_cache_prefetch_translation_queue_translated\":{},\"instruction_cache_prefetch_translation_queue_dropped\":{},\"data_cache_runs\":{},\"data_cache_msi_runs\":{},\"data_cache_mesi_runs\":{},\"data_cache_moesi_runs\":{},\"data_cache_chi_runs\":{},\"data_cache_cpu_responses\":{},\"data_cache_directory_decisions\":{},\"data_cache_dram_accesses\":{},\"data_cache_bank_accepted\":{},\"data_cache_bank_immediate_hits\":{},\"data_cache_bank_scheduled_misses\":{},\"data_cache_bank_coalesced_misses\":{},\"data_cache_l2_runs\":{},\"data_cache_l2_msi_runs\":{},\"data_cache_l2_mesi_runs\":{},\"data_cache_l2_moesi_runs\":{},\"data_cache_l2_chi_runs\":{},\"data_cache_l2_cpu_responses\":{},\"data_cache_l2_directory_decisions\":{},\"data_cache_l2_dram_accesses\":{},\"data_cache_l2_bank_accepted\":{},\"data_cache_l2_bank_immediate_hits\":{},\"data_cache_l2_bank_scheduled_misses\":{},\"data_cache_l2_bank_coalesced_misses\":{},\"data_cache_l2_prefetch_fills\":{},\"data_cache_l3_runs\":{},\"data_cache_l3_msi_runs\":{},\"data_cache_l3_mesi_runs\":{},\"data_cache_l3_moesi_runs\":{},\"data_cache_l3_chi_runs\":{},\"data_cache_l3_cpu_responses\":{},\"data_cache_l3_directory_decisions\":{},\"data_cache_l3_dram_accesses\":{},\"data_cache_l3_bank_accepted\":{},\"data_cache_l3_bank_immediate_hits\":{},\"data_cache_l3_bank_scheduled_misses\":{},\"data_cache_l3_bank_coalesced_misses\":{},\"data_cache_l3_prefetch_fills\":{},\"data_cache_prefetch_identified\":{},\"data_cache_prefetch_issued\":{},\"data_cache_prefetch_useful\":{},\"data_cache_prefetch_useful_but_miss\":{},\"data_cache_prefetch_unused\":{},\"data_cache_prefetch_demand_mshr_misses\":{},\"data_cache_prefetch_hit_in_cache\":{},\"data_cache_prefetch_hit_in_mshr\":{},\"data_cache_prefetch_hit_in_write_buffer\":{},\"data_cache_prefetch_late\":{},\"data_cache_prefetch_accuracy_ppm\":{},\"data_cache_prefetch_coverage_ppm\":{},\"data_cache_prefetch_span_page\":{},\"data_cache_prefetch_useful_span_page\":{},\"data_cache_prefetch_in_cache\":{},\"data_cache_prefetch_fills\":{},\"data_cache_prefetch_queue_enqueued\":{},\"data_cache_prefetch_queue_issued\":{},\"data_cache_prefetch_queue_dropped\":{},\"data_cache_prefetch_translation_queue_enqueued\":{},\"data_cache_prefetch_translation_queue_issued\":{},\"data_cache_prefetch_translation_queue_translated\":{},\"data_cache_prefetch_translation_queue_dropped\":{},\"data_access_probes\":{}",
            max_tick,
            optional_count_json(instruction_limit),
            memory_system,
            memory_route_delay,
            host_event_delay,
            self.final_tick,
            self.final_tick,
            self.cores.len(),
            self.committed_instructions,
            self.instruction_probes.to_json(),
            self.instruction_cache.runs,
            self.instruction_cache.msi_runs,
            self.instruction_cache.mesi_runs,
            self.instruction_cache.moesi_runs,
            self.instruction_cache.chi_runs,
            self.instruction_cache.cpu_responses,
            self.instruction_cache.directory_decisions,
            self.instruction_cache.dram_accesses,
            self.instruction_cache.bank_accepted,
            self.instruction_cache.bank_immediate_hits,
            self.instruction_cache.bank_scheduled_misses,
            self.instruction_cache.bank_coalesced_misses,
            self.instruction_cache_l2.runs,
            self.instruction_cache_l2.msi_runs,
            self.instruction_cache_l2.mesi_runs,
            self.instruction_cache_l2.moesi_runs,
            self.instruction_cache_l2.chi_runs,
            self.instruction_cache_l2.cpu_responses,
            self.instruction_cache_l2.directory_decisions,
            self.instruction_cache_l2.dram_accesses,
            self.instruction_cache_l2.bank_accepted,
            self.instruction_cache_l2.bank_immediate_hits,
            self.instruction_cache_l2.bank_scheduled_misses,
            self.instruction_cache_l2.bank_coalesced_misses,
            self.instruction_cache_l2.prefetch_fills,
            self.instruction_cache_l3.runs,
            self.instruction_cache_l3.msi_runs,
            self.instruction_cache_l3.mesi_runs,
            self.instruction_cache_l3.moesi_runs,
            self.instruction_cache_l3.chi_runs,
            self.instruction_cache_l3.cpu_responses,
            self.instruction_cache_l3.directory_decisions,
            self.instruction_cache_l3.dram_accesses,
            self.instruction_cache_l3.bank_accepted,
            self.instruction_cache_l3.bank_immediate_hits,
            self.instruction_cache_l3.bank_scheduled_misses,
            self.instruction_cache_l3.bank_coalesced_misses,
            self.instruction_cache_l3.prefetch_fills,
            self.instruction_cache.prefetch_identified,
            self.instruction_cache.prefetch_issued,
            self.instruction_cache.prefetch_useful,
            self.instruction_cache.prefetch_useful_but_miss,
            self.instruction_cache.prefetch_unused,
            self.instruction_cache.prefetch_demand_mshr_misses,
            self.instruction_cache.prefetch_hit_in_cache,
            self.instruction_cache.prefetch_hit_in_mshr,
            self.instruction_cache.prefetch_hit_in_write_buffer,
            self.instruction_cache.prefetch_late,
            optional_count_json(self.instruction_cache.prefetch_accuracy_ppm),
            optional_count_json(self.instruction_cache.prefetch_coverage_ppm),
            self.instruction_cache.prefetch_span_page,
            self.instruction_cache.prefetch_useful_span_page,
            self.instruction_cache.prefetch_in_cache,
            self.instruction_cache.prefetch_fills,
            self.instruction_cache.prefetch_queue_enqueued,
            self.instruction_cache.prefetch_queue_issued,
            self.instruction_cache.prefetch_queue_dropped,
            self.instruction_cache.prefetch_translation_queue_enqueued,
            self.instruction_cache.prefetch_translation_queue_issued,
            self.instruction_cache.prefetch_translation_queue_translated,
            self.instruction_cache.prefetch_translation_queue_dropped,
            self.data_cache.runs,
            self.data_cache.msi_runs,
            self.data_cache.mesi_runs,
            self.data_cache.moesi_runs,
            self.data_cache.chi_runs,
            self.data_cache.cpu_responses,
            self.data_cache.directory_decisions,
            self.data_cache.dram_accesses,
            self.data_cache.bank_accepted,
            self.data_cache.bank_immediate_hits,
            self.data_cache.bank_scheduled_misses,
            self.data_cache.bank_coalesced_misses,
            self.data_cache_l2.runs,
            self.data_cache_l2.msi_runs,
            self.data_cache_l2.mesi_runs,
            self.data_cache_l2.moesi_runs,
            self.data_cache_l2.chi_runs,
            self.data_cache_l2.cpu_responses,
            self.data_cache_l2.directory_decisions,
            self.data_cache_l2.dram_accesses,
            self.data_cache_l2.bank_accepted,
            self.data_cache_l2.bank_immediate_hits,
            self.data_cache_l2.bank_scheduled_misses,
            self.data_cache_l2.bank_coalesced_misses,
            self.data_cache_l2.prefetch_fills,
            self.data_cache_l3.runs,
            self.data_cache_l3.msi_runs,
            self.data_cache_l3.mesi_runs,
            self.data_cache_l3.moesi_runs,
            self.data_cache_l3.chi_runs,
            self.data_cache_l3.cpu_responses,
            self.data_cache_l3.directory_decisions,
            self.data_cache_l3.dram_accesses,
            self.data_cache_l3.bank_accepted,
            self.data_cache_l3.bank_immediate_hits,
            self.data_cache_l3.bank_scheduled_misses,
            self.data_cache_l3.bank_coalesced_misses,
            self.data_cache_l3.prefetch_fills,
            self.data_cache.prefetch_identified,
            self.data_cache.prefetch_issued,
            self.data_cache.prefetch_useful,
            self.data_cache.prefetch_useful_but_miss,
            self.data_cache.prefetch_unused,
            self.data_cache.prefetch_demand_mshr_misses,
            self.data_cache.prefetch_hit_in_cache,
            self.data_cache.prefetch_hit_in_mshr,
            self.data_cache.prefetch_hit_in_write_buffer,
            self.data_cache.prefetch_late,
            optional_count_json(self.data_cache.prefetch_accuracy_ppm),
            optional_count_json(self.data_cache.prefetch_coverage_ppm),
            self.data_cache.prefetch_span_page,
            self.data_cache.prefetch_useful_span_page,
            self.data_cache.prefetch_in_cache,
            self.data_cache.prefetch_fills,
            self.data_cache.prefetch_queue_enqueued,
            self.data_cache.prefetch_queue_issued,
            self.data_cache.prefetch_queue_dropped,
            self.data_cache.prefetch_translation_queue_enqueued,
            self.data_cache.prefetch_translation_queue_issued,
            self.data_cache.prefetch_translation_queue_translated,
            self.data_cache.prefetch_translation_queue_dropped,
            self.data_access_probes.to_json(),
        );
        match self.stop {
            Rem6ExecutionStop::Idle => {
                format!("{{\"status\":\"idle\",\"stop_reason\":\"idle\",{common}}}")
            }
            Rem6ExecutionStop::HostTrap {
                stop_code,
                trap,
                trap_pc,
            } => format!(
                "{{\"status\":\"executed_until_trap\",\"stop_reason\":\"host_trap\",{},\"stop_code\":{},\"trap\":\"{}\",\"trap_pc\":\"0x{:x}\"}}",
                common, stop_code, trap, trap_pc
            ),
            Rem6ExecutionStop::HostStop { stop_code } => format!(
                "{{\"status\":\"stopped_by_host\",\"stop_reason\":\"host_stop\",{},\"stop_code\":{}}}",
                common, stop_code
            ),
            Rem6ExecutionStop::TickLimit { tick_limit } => format!(
                "{{\"status\":\"stopped_at_tick_limit\",\"stop_reason\":\"tick_limit\",{},\"tick_limit\":{}}}",
                common, tick_limit
            ),
            Rem6ExecutionStop::InstructionLimit { .. } => format!(
                "{{\"status\":\"stopped_at_instruction_limit\",\"stop_reason\":\"instruction_limit\",{}}}",
                common
            ),
        }
    }

    fn to_parallel_json(&self, worker_limit: usize, min_remote_delay: u64) -> String {
        let slots = self
            .parallel_scheduler_worker_slots
            .iter()
            .map(super::Rem6ParallelWorkerSlotSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let lanes = self
            .parallel_scheduler_worker_lanes
            .iter()
            .map(super::Rem6ParallelWorkerLaneSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let partitions = self
            .parallel_scheduler_partitions
            .iter()
            .map(Rem6ParallelPartitionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let frontiers = self
            .parallel_scheduler_frontiers
            .iter()
            .map(Rem6ParallelFrontierSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let final_frontiers = self
            .parallel_scheduler_final_frontiers
            .iter()
            .map(Rem6ParallelFrontierSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let ready_partitions = self
            .parallel_scheduler_ready_partitions
            .iter()
            .map(Rem6ParallelReadyPartitionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"scheduler\":{{\"worker_limit\":{},\"min_remote_delay\":{},\"epochs\":{},\"dispatches\":{},\"batches\":{},\"max_workers\":{},\"total_workers\":{},\"active_partitions\":{},\"remote_sends\":{},\"batch_worker_ticks\":{},\"batch_worker_capacity_ticks\":{},\"batch_idle_worker_ticks\":{},\"worker_slots\":[{}],\"worker_lanes\":[{}],\"partitions\":[{}],\"frontiers\":[{}],\"final_frontiers\":[{}],\"ready_partitions\":[{}]}}}}",
            worker_limit,
            min_remote_delay,
            self.parallel_scheduler_epochs,
            self.parallel_scheduler_dispatches,
            self.parallel_scheduler_batches,
            self.parallel_scheduler_max_workers,
            self.parallel_scheduler_total_workers,
            self.parallel_scheduler_active_partitions,
            self.parallel_scheduler_remote_sends,
            self.parallel_scheduler_batch_worker_ticks,
            self.parallel_scheduler_batch_worker_capacity_ticks,
            self.parallel_scheduler_batch_idle_worker_ticks,
            slots,
            lanes,
            partitions,
            frontiers,
            final_frontiers,
            ready_partitions,
        )
    }

    fn debug_json_field(&self) -> Option<String> {
        self.debug
            .has_enabled_flags()
            .then(|| format!(",\"debug\":{}", self.debug.to_json()))
    }

    fn to_cores_json(&self) -> String {
        let cores = self
            .cores
            .iter()
            .map(Rem6CoreSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{cores}]")
    }

    fn to_memory_json(&self) -> String {
        let dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{dumps}]")
    }

    fn to_memory_resources_json(&self) -> String {
        self.memory_resources.to_json()
    }

    fn to_riscv_guest_writes_json(&self) -> String {
        let writes = self
            .riscv_guest_writes
            .iter()
            .map(Rem6RiscvGuestWriteSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{writes}]")
    }

    fn to_riscv_unknown_syscalls_json(&self) -> String {
        let syscalls = self
            .riscv_unknown_syscalls
            .iter()
            .map(Rem6RiscvUnknownSyscallSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{syscalls}]")
    }

    fn to_riscv_sbi_console_json(&self) -> String {
        self.riscv_sbi_console.to_json()
    }

    fn to_riscv_sbi_timers_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_timers
                .iter()
                .map(Rem6RiscvSbiTimerSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_hsm_events_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_hsm_events
                .iter()
                .map(Rem6RiscvSbiHsmSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_hsm_wakes_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_hsm_wakes
                .iter()
                .map(Rem6RiscvSbiHsmWakeSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_hsm_statuses_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_hsm_statuses
                .iter()
                .map(Rem6RiscvSbiHsmStatusSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_ipis_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_ipis
                .iter()
                .map(Rem6RiscvSbiIpiSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_rfences_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_rfences
                .iter()
                .map(Rem6RiscvSbiRfenceSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_rfence_completions_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_rfence_completions
                .iter()
                .map(Rem6RiscvSbiRfenceCompletionSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_riscv_sbi_resets_json(&self) -> String {
        format!(
            "[{}]",
            self.riscv_sbi_resets
                .iter()
                .map(Rem6RiscvSbiResetSummary::to_json)
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn to_transport_json(&self) -> String {
        format!(
            "{{\"fetch\":{},\"data\":{}}}",
            self.fetch_transport.to_json(),
            self.data_transport.to_json()
        )
    }

    fn to_host_actions_json(&self) -> String {
        self.host_actions.to_json()
    }

    fn to_dram_json(&self) -> String {
        self.dram.to_json()
    }
}

impl Rem6RiscvGuestWriteSummary {
    fn to_json(&self) -> String {
        let text = std::str::from_utf8(&self.bytes)
            .map(|value| format!("\"{}\"", json_escape(value)))
            .unwrap_or_else(|_| "null".to_string());
        format!(
            "{{\"fd\":{},\"address\":\"0x{:x}\",\"tick\":{},\"bytes\":{},\"text\":{},\"hex\":\"{}\"}}",
            self.fd,
            self.address,
            self.tick,
            self.bytes.len(),
            text,
            bytes_to_hex(&self.bytes),
        )
    }
}

impl Rem6RiscvUnknownSyscallSummary {
    fn to_json(&self) -> String {
        let arguments = self
            .arguments
            .iter()
            .map(|argument| format!("\"0x{argument:x}\""))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"pc\":\"0x{:x}\",\"number\":{},\"tick\":{},\"arguments\":[{}]}}",
            self.pc, self.number, self.tick, arguments
        )
    }
}

impl Rem6RiscvSbiConsoleSummary {
    fn to_json(&self) -> String {
        let text = std::str::from_utf8(self.bytes())
            .map(|value| format!("\"{}\"", json_escape(value)))
            .unwrap_or_else(|_| "null".to_string());
        format!(
            "{{\"bytes\":{},\"text\":{},\"hex\":\"{}\"}}",
            self.byte_count(),
            text,
            bytes_to_hex(self.bytes()),
        )
    }
}

impl Rem6RiscvSbiTimerSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"deadline\":{}}}",
            self.cpu(),
            self.deadline()
        )
    }
}

impl Rem6RiscvSbiHsmSummary {
    fn to_json(&self) -> String {
        if self.is_hart_suspend() {
            format!(
                "{{\"source_cpu\":{},\"function\":{},\"suspend_type\":\"0x{:x}\",\"resume_addr\":\"0x{:x}\",\"opaque\":\"0x{:x}\"}}",
                self.source_cpu(),
                self.function(),
                self.arg0(),
                self.arg1(),
                self.arg2(),
            )
        } else {
            format!(
                "{{\"source_cpu\":{},\"function\":{},\"target_hart\":{},\"start_addr\":\"0x{:x}\",\"opaque\":\"0x{:x}\"}}",
                self.source_cpu(),
                self.function(),
                self.arg0(),
                self.arg1(),
                self.arg2(),
            )
        }
    }
}

impl Rem6RiscvSbiHsmWakeSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"source_cpu\":{},\"target_hart\":{},\"interrupt_bits\":\"0x{:x}\"}}",
            self.source_cpu(),
            self.target_hart(),
            self.interrupt_bits()
        )
    }
}

impl Rem6RiscvSbiHsmStatusSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"source_cpu\":{},\"target_hart\":{},\"status\":{},\"status_name\":\"{}\"}}",
            self.source_cpu(),
            self.target_hart(),
            self.status(),
            self.status_name()
        )
    }
}

impl Rem6RiscvSbiIpiSummary {
    fn to_json(&self) -> String {
        let targets = self
            .targets()
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"source_cpu\":{},\"hart_mask\":\"0x{:x}\",\"hart_mask_base\":\"0x{:x}\",\"targets\":[{}]}}",
            self.source_cpu(),
            self.hart_mask(),
            self.hart_mask_base(),
            targets,
        )
    }
}

impl Rem6RiscvSbiRfenceSummary {
    fn to_json(&self) -> String {
        let targets = self
            .targets()
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let address_space = self
            .address_space()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"source_cpu\":{},\"function\":{},\"hart_mask\":\"0x{:x}\",\"hart_mask_base\":\"0x{:x}\",\"start_addr\":\"0x{:x}\",\"size\":\"0x{:x}\",\"address_space\":{},\"targets\":[{}]}}",
            self.source_cpu(),
            self.function(),
            self.hart_mask(),
            self.hart_mask_base(),
            self.start_addr(),
            self.size(),
            address_space,
            targets,
        )
    }
}

impl Rem6RiscvSbiRfenceCompletionSummary {
    fn to_json(&self) -> String {
        let address_space = self
            .address_space()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        let flushed_entries = self
            .flushed_entries()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"source_cpu\":{},\"target_hart\":{},\"function\":{},\"start_addr\":\"0x{:x}\",\"size\":\"0x{:x}\",\"address_space\":{},\"completed_tick\":{},\"flushed_entries\":{}}}",
            self.source_cpu(),
            self.target_hart(),
            self.function(),
            self.start_addr(),
            self.size(),
            address_space,
            self.completed_tick(),
            flushed_entries,
        )
    }
}

impl Rem6RiscvSbiResetSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"cpu\":{},\"reset_type\":{},\"reset_reason\":{},\"code\":{}}}",
            self.cpu(),
            self.reset_type(),
            self.reset_reason(),
            self.code(),
        )
    }
}

impl Rem6HostActionSummary {
    fn to_json(&self) -> String {
        let injected_commands = self
            .injected_commands
            .iter()
            .map(Rem6HostInjectedCommandSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let guest_host_calls = self
            .guest_host_calls
            .iter()
            .map(Rem6GuestHostCallSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let roi_begin = self
            .roi_begin
            .iter()
            .map(Rem6HostWorkMarkerSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let roi_end = self
            .roi_end
            .iter()
            .map(Rem6HostWorkMarkerSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let stats_resets = self
            .stats_resets
            .iter()
            .map(Rem6HostStatsResetSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let stats_dumps = self
            .stats_dumps
            .iter()
            .map(Rem6HostStatsDumpSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let checkpoints = self
            .checkpoints
            .iter()
            .map(Rem6HostCheckpointSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let checkpoint_restores = self
            .checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let execution_modes = self
            .execution_modes
            .iter()
            .map(Rem6HostExecutionModeSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let execution_mode_switches = self
            .execution_mode_switches
            .iter()
            .map(Rem6HostExecutionModeSwitchSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let stops = self
            .stops
            .iter()
            .map(Rem6HostStopActionSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"total_action_count\":{},\"injected_command_count\":{},\"guest_host_call_count\":{},\"roi_begin_count\":{},\"roi_end_count\":{},\"stats_reset_count\":{},\"stats_dump_count\":{},\"checkpoint_count\":{},\"checkpoint_restored_count\":{},\"checkpoint_restored_component_count\":{},\"checkpoint_restored_chunk_count\":{},\"checkpoint_restored_payload_bytes\":{},\"execution_mode_switch_count\":{},\"stop_count\":{},\"injected_commands\":[{}],\"guest_host_calls\":[{}],\"roi_begin\":[{}],\"roi_end\":[{}],\"stats_resets\":[{}],\"stats_dumps\":[{}],\"checkpoints\":[{}],\"checkpoint_restores\":[{}],\"execution_modes\":[{}],\"execution_mode_switches\":[{}],\"stops\":[{}]}}",
            self.total_action_count,
            self.injected_command_count,
            self.guest_host_calls.len(),
            self.roi_begin.len(),
            self.roi_end.len(),
            self.stats_resets.len(),
            self.stats_dumps.len(),
            self.checkpoints.len(),
            self.checkpoint_restored_count,
            self.checkpoint_restored_component_count,
            self.checkpoint_restored_chunk_count,
            self.checkpoint_restored_payload_bytes,
            self.execution_mode_switch_count,
            self.stops.len(),
            injected_commands,
            guest_host_calls,
            roi_begin,
            roi_end,
            stats_resets,
            stats_dumps,
            checkpoints,
            checkpoint_restores,
            execution_modes,
            execution_mode_switches,
            stops,
        )
    }
}

impl Rem6HostInjectedCommandSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"command\":\"{}\"}}",
            self.tick,
            self.event,
            self.source,
            json_escape(&self.command),
        )
    }
}

impl Rem6GuestHostCallSummary {
    fn to_json(&self) -> String {
        let arguments = self
            .arguments
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"selector\":{},\"arguments\":[{}],\"argument_count\":{},\"payload_bytes\":{},\"response_status\":{},\"response_return_count\":{},\"response_payload_bytes\":{}}}",
            self.tick,
            self.event,
            self.source,
            self.selector,
            arguments,
            self.argument_count,
            self.payload_bytes,
            self.response_status,
            self.response_return_count,
            self.response_payload_bytes,
        )
    }
}

impl Rem6HostWorkMarkerSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"work_id\":{},\"thread_id\":{}}}",
            self.tick, self.event, self.source, self.work_id, self.thread_id
        )
    }
}

impl Rem6HostStatsResetSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"tick\":{},\"epoch\":{}}}",
            self.id, self.tick, self.epoch
        )
    }
}

impl Rem6HostStatsDumpSummary {
    fn to_json(&self) -> String {
        let samples = self
            .samples
            .iter()
            .map(Rem6HostStatsDumpSampleSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"id\":{},\"tick\":{},\"epoch\":{},\"reset_tick\":{},\"sample_count\":{},\"samples\":[{}]}}",
            self.id,
            self.tick,
            self.epoch,
            self.reset_tick,
            self.samples.len(),
            samples
        )
    }
}

impl Rem6HostStatsDumpSampleSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"path\":\"{}\",\"kind\":\"{}\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\"}}",
            json_escape(&self.path),
            json_escape(&self.kind),
            json_escape(&self.unit),
            self.value,
            json_escape(&self.reset_policy)
        )
    }
}

impl Rem6HostExecutionModeSwitchSummary {
    fn to_json(&self) -> String {
        let previous_mode = self
            .previous_mode
            .map(|mode| format!("\"{}\"", json_escape(mode)))
            .unwrap_or_else(|| "null".to_string());
        let state_transfer = self
            .state_transfer
            .as_ref()
            .map(Rem6ExecutionModeStateTransferSummary::to_json)
            .unwrap_or_else(|| "{\"captured\":false}".to_string());
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"target\":\"{}\",\"previous_mode\":{},\"mode\":\"{}\",\"stats_epoch\":{},\"stats_reset_tick\":{},\"state_transfer\":{}}}",
            self.tick,
            self.event,
            self.source,
            json_escape(&self.target),
            previous_mode,
            json_escape(self.mode),
            self.stats_epoch,
            self.stats_reset_tick,
            state_transfer,
        )
    }
}

impl Rem6HostExecutionModeSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"target\":\"{}\",\"mode\":\"{}\"}}",
            json_escape(&self.target),
            self.mode
        )
    }
}

impl Rem6ExecutionModeStateTransferSummary {
    fn to_json(&self) -> String {
        let components = self
            .components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let quiescence_gate = self.quiescence_gate.to_json();
        let writeback_width = optional_count_json(self.writeback_width);
        let reserved_future_completions = optional_count_json(self.reserved_future_completions);
        let earliest_unpublished_writeback_tick =
            optional_tick_json(self.earliest_unpublished_writeback_tick);
        format!(
            "{{\"captured\":true,\"manifest_label\":\"{}\",\"manifest_tick\":{},\"component_count\":{},\"chunk_count\":{},\"payload_bytes\":{},\"restorable\":{},\"live_data_handoff\":{},\"writeback_width\":{},\"reserved_future_completions\":{},\"earliest_unpublished_writeback_tick\":{},\"quiescence_gate\":{},\"components\":[{}]}}",
            json_escape(&self.manifest_label),
            self.manifest_tick,
            self.component_count,
            self.chunk_count,
            self.payload_bytes,
            self.restorable,
            self.live_data_handoff,
            writeback_width,
            reserved_future_completions,
            earliest_unpublished_writeback_tick,
            quiescence_gate,
            components,
        )
    }
}

impl Rem6ExecutionModeQuiescenceGateSummary {
    fn to_json(&self) -> String {
        let checker = self
            .checker
            .as_ref()
            .map(|checker| {
                format!(
                    ",\"checker\":{{\"checked_instructions\":{},\"mismatches\":{}}}",
                    checker.checked_instructions, checker.mismatches
                )
            })
            .unwrap_or_default();
        format!(
            "{{\"validated\":{},\"target\":\"{}\",\"captured_component_count\":{},\"captured_chunk_count\":{},\"captured_payload_bytes\":{}{}}}",
            self.validated,
            json_escape(&self.target),
            self.captured_component_count,
            self.captured_chunk_count,
            self.captured_payload_bytes,
            checker,
        )
    }
}

impl Rem6HostStopActionSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"tick\":{},\"event\":{},\"source\":{},\"code\":{}}}",
            self.tick, self.event, self.source, self.code
        )
    }
}

impl Rem6InstructionProbeSummary {
    fn to_json(&self) -> String {
        let pc_count = self
            .pc_count
            .as_ref()
            .map(Rem6PcCountTrackerSummary::to_instruction_probe_json_suffix)
            .unwrap_or_default();
        format!(
            "{{\"event_count\":{},\"retired_instruction_events\":{},\"tracked_instructions\":{},\"pc_sample_events\":{},\"pc_target_counters\":{}{}}}",
            self.event_count,
            self.retired_instruction_events,
            self.tracked_instructions,
            self.pc_sample_events,
            self.pc_target_counters,
            pc_count,
        )
    }
}

impl Rem6PcCountTrackerSummary {
    fn to_instruction_probe_json_suffix(&self) -> String {
        format!(
            ",\"pc_target_armed\":{},\"pc_current_pair\":{},\"pc_target_counts\":{},\"pc_pending_targets\":{}",
            self.armed,
            self.current_pair.to_json(),
            pc_count_pair_slice_to_json(&self.counters),
            pc_count_pair_slice_to_json(&self.pending_targets),
        )
    }
}

impl Rem6PcCountPairSummary {
    fn to_json(self) -> String {
        format!("{{\"pc\":\"0x{:x}\",\"count\":{}}}", self.pc, self.count)
    }
}

fn pc_count_pair_slice_to_json(pairs: &[Rem6PcCountPairSummary]) -> String {
    format!(
        "[{}]",
        pairs
            .iter()
            .copied()
            .map(Rem6PcCountPairSummary::to_json)
            .collect::<Vec<_>>()
            .join(",")
    )
}

impl Rem6DataAccessProbeSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"sample_count\":{},\"stack_distance\":{{\"infinite_samples\":{},\"finite_samples\":{},\"stack_depth\":{}}},\"memory_footprint\":{{\"cache_line_bytes\":{},\"cache_line_total_bytes\":{},\"page_bytes\":{},\"page_total_bytes\":{}}}}}",
            self.sample_count,
            self.stack_distance_infinite_samples,
            self.stack_distance_finite_samples,
            self.stack_distance_stack_depth,
            self.memory_footprint_cache_line_bytes,
            self.memory_footprint_cache_line_total_bytes,
            self.memory_footprint_page_bytes,
            self.memory_footprint_page_total_bytes,
        )
    }
}

fn optional_string_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", json_escape(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_tick_json(value: Option<u64>) -> String {
    value
        .map(|tick| tick.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_count_json(value: Option<u64>) -> String {
    value
        .map(|count| count.to_string())
        .unwrap_or_else(|| "null".to_string())
}

impl Rem6MemoryDump {
    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"hex\":\"{}\"}}",
            self.address,
            self.data.len(),
            bytes_to_hex(&self.data),
        )
    }
}
