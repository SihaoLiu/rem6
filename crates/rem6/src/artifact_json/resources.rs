use crate::{
    Rem6CacheResourceSummary, Rem6DramResourceSummary, Rem6FabricResourceSummary,
    Rem6MemoryResourceSummary, Rem6TransportResourceSummary,
};

use super::{dram_targets_json, json_escape};

impl Rem6MemoryResourceSummary {
    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"activity\":{},\"active\":{},\"cache\":{{\"activity\":{},\"active\":{},\"cpu_responses\":{},\"directory_decisions\":{},\"dram_accesses\":{},\"bank_accepted\":{},\"bank_immediate_hits\":{},\"bank_scheduled_misses\":{},\"bank_coalesced_misses\":{},\"l1\":{},\"l2\":{},\"l3\":{}}},\"transport\":{{\"activity\":{},\"active\":{},\"request_arrivals\":{},\"responses\":{},\"response_arrivals\":{},\"round_trip_ticks\":{},\"max_round_trip_ticks\":{},\"fetch\":{},\"data\":{}}},\"fabric\":{},\"dram\":{}}}",
            self.activity,
            self.active,
            self.cache.activity,
            self.cache.active,
            self.cache.cpu_responses,
            self.cache.directory_decisions,
            self.cache.dram_accesses,
            self.cache.bank_accepted,
            self.cache.bank_immediate_hits,
            self.cache.bank_scheduled_misses,
            self.cache.bank_coalesced_misses,
            cache_resource_json(&self.cache_l1),
            cache_resource_json(&self.cache_l2),
            cache_resource_json(&self.cache_l3),
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

fn dram_resource_json(summary: &Rem6DramResourceSummary) -> String {
    format!(
        "{{\"activity\":{},\"active\":{},\"active_targets\":{},\"active_ports\":{},\"active_banks\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"targets\":[{}]}}",
        summary.activity,
        summary.active,
        summary.active_targets,
        summary.active_ports,
        summary.active_banks,
        summary.accesses,
        summary.reads,
        summary.writes,
        summary.row_hits,
        summary.row_misses,
        summary.commands,
        summary.turnarounds,
        summary.total_ready_latency_ticks,
        summary.max_ready_latency_ticks,
        dram_targets_json(&summary.targets),
    )
}

fn fabric_resource_json(summary: &Rem6FabricResourceSummary) -> String {
    format!(
        "{{\"activity\":{},\"active\":{},\"active_virtual_networks\":{},\"bytes\":{},\"flits\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_lanes\":{},\"lane_activities\":[{}],\"hop_activities\":[{}]}}",
        summary.activity,
        summary.active,
        summary.active_virtual_networks,
        summary.bytes,
        summary.flits,
        summary.occupied_ticks,
        summary.queue_delay_ticks,
        summary.max_queue_delay_ticks,
        summary.credit_delay_ticks,
        summary.max_credit_delay_ticks,
        summary.contended_lanes,
        fabric_resource_lane_activities_json(summary),
        fabric_resource_hop_activities_json(summary),
    )
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
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
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
    format!(
        "{{\"activity\":{},\"active\":{},\"cpu_responses\":{},\"directory_decisions\":{},\"dram_accesses\":{},\"bank_accepted\":{},\"bank_immediate_hits\":{},\"bank_scheduled_misses\":{},\"bank_coalesced_misses\":{}}}",
        summary.activity,
        summary.active,
        summary.cpu_responses,
        summary.directory_decisions,
        summary.dram_accesses,
        summary.bank_accepted,
        summary.bank_immediate_hits,
        summary.bank_scheduled_misses,
        summary.bank_coalesced_misses,
    )
}
