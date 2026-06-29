use rem6_fabric::FabricHopActivity;

use crate::{
    Rem6CacheResourceHierarchySummary, Rem6CacheResourceSummary, Rem6DramResourceSummary,
    Rem6FabricResourceSummary, Rem6MemoryResourceSummary, Rem6TransportResourceSummary,
};

use super::{dram_targets_json, json_escape, optional_count_json};

impl Rem6MemoryResourceSummary {
    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"activity\":{},\"active\":{},\"cache\":{},\"transport\":{{\"activity\":{},\"active\":{},\"request_arrivals\":{},\"responses\":{},\"response_arrivals\":{},\"round_trip_ticks\":{},\"max_round_trip_ticks\":{},\"fetch\":{},\"data\":{}}},\"fabric\":{},\"dram\":{}}}",
            self.activity,
            self.active,
            aggregate_cache_resource_json(self),
            self.transport.activity,
            self.transport.active,
            self.transport.request_arrivals,
            self.transport.responses,
            self.transport.response_arrivals,
            self.transport.round_trip_ticks,
            self.transport.max_round_trip_ticks,
            transport_resource_json(&self.transport_fetch),
            transport_resource_json(&self.transport_data),
            fabric_resource_json(&self.fabric),
            dram_resource_json(&self.dram),
        )
    }
}

fn aggregate_cache_resource_json(summary: &Rem6MemoryResourceSummary) -> String {
    format!(
        "{{{},\"l1\":{},\"l2\":{},\"l3\":{},\"instruction\":{},\"data\":{}}}",
        cache_resource_fields_json(&summary.cache),
        cache_resource_json(&summary.cache_l1),
        cache_resource_json(&summary.cache_l2),
        cache_resource_json(&summary.cache_l3),
        cache_resource_hierarchy_json(&summary.cache_instruction),
        cache_resource_hierarchy_json(&summary.cache_data),
    )
}

fn cache_resource_hierarchy_json(summary: &Rem6CacheResourceHierarchySummary) -> String {
    format!(
        "{{{},\"l1\":{},\"l2\":{},\"l3\":{}}}",
        cache_resource_fields_json(&summary.aggregate),
        cache_resource_json(&summary.l1),
        cache_resource_json(&summary.l2),
        cache_resource_json(&summary.l3),
    )
}

fn dram_resource_json(summary: &Rem6DramResourceSummary) -> String {
    format!(
        "{{\"activity\":{},\"active\":{},\"active_targets\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"read_row_hits\":{},\"write_row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"read_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{},\"targets\":[{}]}}",
        summary.activity,
        summary.active,
        summary.active_targets,
        summary.active_ports,
        summary.active_banks,
        summary.accesses,
        summary.reads,
        summary.writes,
        summary.read_bytes,
        summary.write_bytes,
        summary.row_hits,
        summary.read_row_hits,
        summary.write_row_hits,
        summary.row_misses,
        summary.refreshes,
        summary.refresh_ticks,
        summary.commands,
        summary.turnarounds,
        summary.total_ready_latency_ticks,
        summary.read_ready_latency_ticks,
        summary.max_ready_latency_ticks,
        dram_low_power_json(
            summary.low_power_active_powerdown_entries,
            summary.low_power_active_powerdown_ticks,
            summary.low_power_precharge_powerdown_entries,
            summary.low_power_precharge_powerdown_ticks,
            summary.low_power_self_refresh_entries,
            summary.low_power_self_refresh_ticks,
            summary.low_power_exits,
            summary.low_power_exit_latency_ticks,
        ),
        dram_targets_json(&summary.targets),
    )
}

pub(super) fn dram_low_power_json(
    active_powerdown_entries: u64,
    active_powerdown_ticks: u64,
    precharge_powerdown_entries: u64,
    precharge_powerdown_ticks: u64,
    self_refresh_entries: u64,
    self_refresh_ticks: u64,
    exits: u64,
    exit_latency_ticks: u64,
) -> String {
    format!(
        "{{\"active_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"precharge_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"self_refresh\":{{\"entries\":{},\"ticks\":{}}},\"exits\":{},\"exit_latency_ticks\":{}}}",
        active_powerdown_entries,
        active_powerdown_ticks,
        precharge_powerdown_entries,
        precharge_powerdown_ticks,
        self_refresh_entries,
        self_refresh_ticks,
        exits,
        exit_latency_ticks,
    )
}

fn fabric_resource_json(summary: &Rem6FabricResourceSummary) -> String {
    format!(
        "{{\"activity\":{},\"active\":{},\"active_virtual_networks\":{},\"active_links\":{},\"active_hops\":{},\"bytes\":{},\"flits\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_lanes\":{},\"link_activities\":[{}],\"lane_activities\":[{}],\"hop_activities\":[{}]}}",
        summary.activity,
        summary.active,
        summary.active_virtual_networks,
        summary.active_links,
        summary.active_hops,
        summary.bytes,
        summary.flits,
        summary.occupied_ticks,
        summary.queue_delay_ticks,
        summary.max_queue_delay_ticks,
        summary.credit_delay_ticks,
        summary.max_credit_delay_ticks,
        summary.contended_lanes,
        fabric_resource_link_activities_json(summary),
        fabric_resource_lane_activities_json(summary),
        fabric_resource_hop_activities_json(summary),
    )
}

fn fabric_resource_link_activities_json(summary: &Rem6FabricResourceSummary) -> String {
    summary
        .link_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"active_virtual_networks\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_virtual_networks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.active_virtual_network_count(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.contended_virtual_network_count(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fabric_resource_lane_activities_json(summary: &Rem6FabricResourceSummary) -> String {
    summary
        .lane_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fabric_resource_hop_activities_json(summary: &Rem6FabricResourceSummary) -> String {
    summary
        .hop_activities()
        .iter()
        .map(|activity| {
            let router = fabric_resource_hop_router_json(activity);
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"router\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                router,
                activity.bytes(),
                activity.flits(),
                activity.ready_tick(),
                activity.start_tick(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.depart_tick(),
                activity.arrival_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fabric_resource_hop_router_json(activity: &FabricHopActivity) -> String {
    match activity.router() {
        Some(router) => format!(
            "{{\"router\":\"{}\",\"input_port\":{},\"output_port\":{},\"virtual_channel\":{},\"ready_tick\":{},\"start_tick\":{},\"latency_ticks\":{},\"depart_tick\":{},\"queue_delay_ticks\":{}}}",
            json_escape(router.router().as_str()),
            router.input_port(),
            router.output_port(),
            router.virtual_channel(),
            router.ready_tick(),
            router.start_tick(),
            router.latency_ticks(),
            router.depart_tick(),
            router.queue_delay_ticks(),
        ),
        None => "null".to_string(),
    }
}

fn transport_resource_json(summary: &Rem6TransportResourceSummary) -> String {
    format!(
        "{{\"activity\":{},\"active\":{},\"request_arrivals\":{},\"responses\":{},\"response_arrivals\":{},\"round_trip_ticks\":{},\"max_round_trip_ticks\":{}}}",
        summary.activity,
        summary.active,
        summary.request_arrivals,
        summary.responses,
        summary.response_arrivals,
        summary.round_trip_ticks,
        summary.max_round_trip_ticks,
    )
}

fn cache_resource_json(summary: &Rem6CacheResourceSummary) -> String {
    format!("{{{}}}", cache_resource_fields_json(summary))
}

fn cache_resource_fields_json(summary: &Rem6CacheResourceSummary) -> String {
    format!(
        "\"activity\":{},\"active\":{},\"cpu_responses\":{},\"directory_decisions\":{},\"dram_accesses\":{},\"bank_accepted\":{},\"bank_immediate_hits\":{},\"bank_scheduled_misses\":{},\"bank_coalesced_misses\":{},\"prefetch_identified\":{},\"prefetch_issued\":{},\"prefetch_useful\":{},\"prefetch_useful_but_miss\":{},\"prefetch_unused\":{},\"prefetch_demand_mshr_misses\":{},\"prefetch_hit_in_cache\":{},\"prefetch_hit_in_mshr\":{},\"prefetch_hit_in_write_buffer\":{},\"prefetch_late\":{},\"prefetch_accuracy_ppm\":{},\"prefetch_coverage_ppm\":{},\"prefetch_span_page\":{},\"prefetch_useful_span_page\":{},\"prefetch_in_cache\":{},\"prefetch_fills\":{},\"prefetch_queue_enqueued\":{},\"prefetch_queue_issued\":{},\"prefetch_queue_dropped\":{},\"prefetch_translation_queue_enqueued\":{},\"prefetch_translation_queue_issued\":{},\"prefetch_translation_queue_translated\":{},\"prefetch_translation_queue_dropped\":{}",
        summary.activity,
        summary.active,
        summary.cpu_responses,
        summary.directory_decisions,
        summary.dram_accesses,
        summary.bank_accepted,
        summary.bank_immediate_hits,
        summary.bank_scheduled_misses,
        summary.bank_coalesced_misses,
        summary.prefetch_identified,
        summary.prefetch_issued,
        summary.prefetch_useful,
        summary.prefetch_useful_but_miss,
        summary.prefetch_unused,
        summary.prefetch_demand_mshr_misses,
        summary.prefetch_hit_in_cache,
        summary.prefetch_hit_in_mshr,
        summary.prefetch_hit_in_write_buffer,
        summary.prefetch_late,
        optional_count_json(summary.prefetch_accuracy_ppm),
        optional_count_json(summary.prefetch_coverage_ppm),
        summary.prefetch_span_page,
        summary.prefetch_useful_span_page,
        summary.prefetch_in_cache,
        summary.prefetch_fills,
        summary.prefetch_queue_enqueued,
        summary.prefetch_queue_issued,
        summary.prefetch_queue_dropped,
        summary.prefetch_translation_queue_enqueued,
        summary.prefetch_translation_queue_issued,
        summary.prefetch_translation_queue_translated,
        summary.prefetch_translation_queue_dropped,
    )
}
