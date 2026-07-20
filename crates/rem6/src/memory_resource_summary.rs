use crate::{
    data_cache_runtime::CliDataCacheSummary, transport_summary::Rem6MemoryTransportSummary,
    Rem6DramSummary, Rem6DramTargetSummary, Rem6RunFabricRouterActivity, Rem6RunFabricSummary,
};
use std::collections::BTreeSet;

use rem6_fabric::{
    FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6MemoryResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) cache: Rem6CacheResourceSummary,
    pub(crate) cache_l1: Rem6CacheResourceSummary,
    pub(crate) cache_l2: Rem6CacheResourceSummary,
    pub(crate) cache_l3: Rem6CacheResourceSummary,
    pub(crate) cache_instruction: Rem6CacheResourceHierarchySummary,
    pub(crate) cache_data: Rem6CacheResourceHierarchySummary,
    pub(crate) transport: Rem6TransportResourceSummary,
    pub(crate) transport_fetch: Rem6TransportResourceSummary,
    pub(crate) transport_data: Rem6TransportResourceSummary,
    pub(crate) fabric: Rem6FabricResourceSummary,
    pub(crate) dram: Rem6DramResourceSummary,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6CacheResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) msi_runs: u64,
    pub(crate) mesi_runs: u64,
    pub(crate) moesi_runs: u64,
    pub(crate) chi_runs: u64,
    pub(crate) cpu_responses: u64,
    pub(crate) directory_decisions: u64,
    pub(crate) dram_accesses: u64,
    pub(crate) bank_accepted: u64,
    pub(crate) bank_immediate_hits: u64,
    pub(crate) bank_scheduled_misses: u64,
    pub(crate) bank_coalesced_misses: u64,
    pub(crate) prefetch_identified: u64,
    pub(crate) prefetch_issued: u64,
    pub(crate) prefetch_useful: u64,
    pub(crate) prefetch_useful_but_miss: u64,
    pub(crate) prefetch_unused: u64,
    pub(crate) prefetch_demand_mshr_misses: u64,
    pub(crate) prefetch_hit_in_cache: u64,
    pub(crate) prefetch_hit_in_mshr: u64,
    pub(crate) prefetch_hit_in_write_buffer: u64,
    pub(crate) prefetch_late: u64,
    pub(crate) prefetch_accuracy_ppm: Option<u64>,
    pub(crate) prefetch_coverage_ppm: Option<u64>,
    pub(crate) prefetch_span_page: u64,
    pub(crate) prefetch_useful_span_page: u64,
    pub(crate) prefetch_in_cache: u64,
    pub(crate) prefetch_fills: u64,
    pub(crate) prefetch_queue_enqueued: u64,
    pub(crate) prefetch_queue_issued: u64,
    pub(crate) prefetch_queue_dropped: u64,
    pub(crate) prefetch_translation_queue_enqueued: u64,
    pub(crate) prefetch_translation_queue_issued: u64,
    pub(crate) prefetch_translation_queue_translated: u64,
    pub(crate) prefetch_translation_queue_dropped: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6CacheResourceHierarchySummary {
    pub(crate) aggregate: Rem6CacheResourceSummary,
    pub(crate) l1: Rem6CacheResourceSummary,
    pub(crate) l2: Rem6CacheResourceSummary,
    pub(crate) l3: Rem6CacheResourceSummary,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6TransportResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) request_arrivals: u64,
    pub(crate) responses: u64,
    pub(crate) response_arrivals: u64,
    pub(crate) round_trip_ticks: u64,
    pub(crate) max_round_trip_ticks: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6FabricResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) active_virtual_networks: u64,
    pub(crate) active_links: u64,
    pub(crate) active_hops: u64,
    pub(crate) active_routers: u64,
    pub(crate) bytes: u64,
    pub(crate) flits: u64,
    pub(crate) occupied_ticks: u64,
    pub(crate) queue_delay_ticks: u64,
    pub(crate) max_queue_delay_ticks: u64,
    pub(crate) credit_delay_ticks: u64,
    pub(crate) max_credit_delay_ticks: u64,
    pub(crate) contended_lanes: u64,
    pub(crate) link_activities: Vec<FabricLinkActivity>,
    pub(crate) lane_activities: Vec<FabricLaneActivity>,
    pub(crate) hop_activities: Vec<FabricHopActivity>,
    pub(crate) router_activities: Vec<Rem6RunFabricRouterActivity>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DramResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) active_targets: u64,
    pub(crate) active_ports: u64,
    pub(crate) active_banks: u64,
    pub(crate) profiled_targets: u64,
    pub(crate) accesses: u64,
    pub(crate) reads: u64,
    pub(crate) writes: u64,
    pub(crate) read_bytes: u64,
    pub(crate) write_bytes: u64,
    pub(crate) row_hits: u64,
    pub(crate) read_row_hits: u64,
    pub(crate) write_row_hits: u64,
    pub(crate) row_misses: u64,
    pub(crate) refreshes: u64,
    pub(crate) refresh_ticks: u64,
    pub(crate) commands: u64,
    pub(crate) turnarounds: u64,
    pub(crate) total_ready_latency_ticks: u64,
    pub(crate) read_ready_latency_ticks: u64,
    pub(crate) max_ready_latency_ticks: u64,
    pub(crate) low_power_active_powerdown_entries: u64,
    pub(crate) low_power_active_powerdown_ticks: u64,
    pub(crate) low_power_precharge_powerdown_entries: u64,
    pub(crate) low_power_precharge_powerdown_ticks: u64,
    pub(crate) low_power_self_refresh_entries: u64,
    pub(crate) low_power_self_refresh_ticks: u64,
    pub(crate) low_power_exits: u64,
    pub(crate) low_power_exit_latency_ticks: u64,
    pub(crate) targets: Vec<Rem6DramTargetSummary>,
}

pub(crate) struct Rem6MemoryResourceInputs<'a> {
    pub(crate) instruction_caches: [&'a CliDataCacheSummary; 3],
    pub(crate) data_caches: [&'a CliDataCacheSummary; 3],
    pub(crate) fetch_transport: &'a Rem6MemoryTransportSummary,
    pub(crate) data_transport: &'a Rem6MemoryTransportSummary,
    pub(crate) fabric: &'a Rem6RunFabricSummary,
    pub(crate) dram: &'a Rem6DramSummary,
}

impl Rem6MemoryResourceInputs<'_> {
    fn cache_summaries(&self) -> impl Iterator<Item = &CliDataCacheSummary> {
        self.instruction_caches
            .iter()
            .chain(self.data_caches.iter())
            .copied()
    }
}

impl Rem6CacheResourceSummary {
    fn from_cache_summaries<'a>(caches: impl IntoIterator<Item = &'a CliDataCacheSummary>) -> Self {
        let mut summary = caches
            .into_iter()
            .fold(Self::default(), |mut summary, cache| {
                summary.activity = summary.activity.saturating_add(cache.runs);
                summary.active += u64::from(cache.runs != 0);
                summary.msi_runs = summary.msi_runs.saturating_add(cache.msi_runs);
                summary.mesi_runs = summary.mesi_runs.saturating_add(cache.mesi_runs);
                summary.moesi_runs = summary.moesi_runs.saturating_add(cache.moesi_runs);
                summary.chi_runs = summary.chi_runs.saturating_add(cache.chi_runs);
                summary.cpu_responses = summary.cpu_responses.saturating_add(cache.cpu_responses);
                summary.directory_decisions = summary
                    .directory_decisions
                    .saturating_add(cache.directory_decisions);
                summary.dram_accesses = summary.dram_accesses.saturating_add(cache.dram_accesses);
                summary.bank_accepted = summary.bank_accepted.saturating_add(cache.bank_accepted);
                summary.bank_immediate_hits = summary
                    .bank_immediate_hits
                    .saturating_add(cache.bank_immediate_hits);
                summary.bank_scheduled_misses = summary
                    .bank_scheduled_misses
                    .saturating_add(cache.bank_scheduled_misses);
                summary.bank_coalesced_misses = summary
                    .bank_coalesced_misses
                    .saturating_add(cache.bank_coalesced_misses);
                summary.prefetch_identified = summary
                    .prefetch_identified
                    .saturating_add(cache.prefetch_identified);
                summary.prefetch_issued = summary
                    .prefetch_issued
                    .saturating_add(cache.prefetch_issued);
                summary.prefetch_useful = summary
                    .prefetch_useful
                    .saturating_add(cache.prefetch_useful);
                summary.prefetch_useful_but_miss = summary
                    .prefetch_useful_but_miss
                    .saturating_add(cache.prefetch_useful_but_miss);
                summary.prefetch_unused = summary
                    .prefetch_unused
                    .saturating_add(cache.prefetch_unused);
                summary.prefetch_demand_mshr_misses = summary
                    .prefetch_demand_mshr_misses
                    .saturating_add(cache.prefetch_demand_mshr_misses);
                summary.prefetch_hit_in_cache = summary
                    .prefetch_hit_in_cache
                    .saturating_add(cache.prefetch_hit_in_cache);
                summary.prefetch_hit_in_mshr = summary
                    .prefetch_hit_in_mshr
                    .saturating_add(cache.prefetch_hit_in_mshr);
                summary.prefetch_hit_in_write_buffer = summary
                    .prefetch_hit_in_write_buffer
                    .saturating_add(cache.prefetch_hit_in_write_buffer);
                summary.prefetch_late = summary.prefetch_late.saturating_add(cache.prefetch_late);
                summary.prefetch_span_page = summary
                    .prefetch_span_page
                    .saturating_add(cache.prefetch_span_page);
                summary.prefetch_useful_span_page = summary
                    .prefetch_useful_span_page
                    .saturating_add(cache.prefetch_useful_span_page);
                summary.prefetch_in_cache = summary
                    .prefetch_in_cache
                    .saturating_add(cache.prefetch_in_cache);
                summary.prefetch_fills =
                    summary.prefetch_fills.saturating_add(cache.prefetch_fills);
                summary.prefetch_queue_enqueued = summary
                    .prefetch_queue_enqueued
                    .saturating_add(cache.prefetch_queue_enqueued);
                summary.prefetch_queue_issued = summary
                    .prefetch_queue_issued
                    .saturating_add(cache.prefetch_queue_issued);
                summary.prefetch_queue_dropped = summary
                    .prefetch_queue_dropped
                    .saturating_add(cache.prefetch_queue_dropped);
                summary.prefetch_translation_queue_enqueued = summary
                    .prefetch_translation_queue_enqueued
                    .saturating_add(cache.prefetch_translation_queue_enqueued);
                summary.prefetch_translation_queue_issued = summary
                    .prefetch_translation_queue_issued
                    .saturating_add(cache.prefetch_translation_queue_issued);
                summary.prefetch_translation_queue_translated = summary
                    .prefetch_translation_queue_translated
                    .saturating_add(cache.prefetch_translation_queue_translated);
                summary.prefetch_translation_queue_dropped = summary
                    .prefetch_translation_queue_dropped
                    .saturating_add(cache.prefetch_translation_queue_dropped);
                summary
            });
        summary.prefetch_accuracy_ppm = ratio_ppm(summary.prefetch_useful, summary.prefetch_issued);
        summary.prefetch_coverage_ppm = ratio_ppm(
            summary.prefetch_useful,
            summary
                .prefetch_useful
                .saturating_add(summary.prefetch_demand_mshr_misses),
        );
        summary
    }
}

fn ratio_ppm(numerator: u64, denominator: u64) -> Option<u64> {
    if denominator == 0 {
        return None;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    Some(ppm.min(u128::from(u64::MAX)) as u64)
}

impl Rem6CacheResourceHierarchySummary {
    fn from_cache_levels(caches: [&CliDataCacheSummary; 3]) -> Self {
        let l1 = Rem6CacheResourceSummary::from_cache_summaries([caches[0]]);
        let l2 = Rem6CacheResourceSummary::from_cache_summaries([caches[1]]);
        let l3 = Rem6CacheResourceSummary::from_cache_summaries([caches[2]]);
        let aggregate = Rem6CacheResourceSummary::from_cache_summaries(caches);

        Self {
            aggregate,
            l1,
            l2,
            l3,
        }
    }
}

impl Rem6TransportResourceSummary {
    fn from_transport(summary: &Rem6MemoryTransportSummary) -> Self {
        let counters = summary.counters;
        Self {
            activity: counters.requests,
            active: u64::from(counters.requests != 0),
            request_arrivals: counters.request_arrivals,
            responses: counters.responses,
            response_arrivals: counters.response_arrivals,
            round_trip_ticks: counters.round_trip_ticks,
            max_round_trip_ticks: counters.max_round_trip_ticks,
        }
    }

    fn combine(summaries: [Self; 2]) -> Self {
        summaries
            .into_iter()
            .fold(Self::default(), |mut combined, summary| {
                combined.activity = combined.activity.saturating_add(summary.activity);
                combined.active = combined.active.saturating_add(summary.active);
                combined.request_arrivals = combined
                    .request_arrivals
                    .saturating_add(summary.request_arrivals);
                combined.responses = combined.responses.saturating_add(summary.responses);
                combined.response_arrivals = combined
                    .response_arrivals
                    .saturating_add(summary.response_arrivals);
                combined.round_trip_ticks = combined
                    .round_trip_ticks
                    .saturating_add(summary.round_trip_ticks);
                combined.max_round_trip_ticks = combined
                    .max_round_trip_ticks
                    .max(summary.max_round_trip_ticks);
                combined
            })
    }
}

impl Rem6FabricResourceSummary {
    fn from_fabric(summary: &Rem6RunFabricSummary) -> Self {
        Self {
            activity: summary.transfers(),
            active: summary.active_lanes(),
            active_virtual_networks: summary.active_virtual_networks(),
            active_links: summary.link_activities().len() as u64,
            active_hops: active_fabric_hop_count(summary.hop_activities()),
            active_routers: summary.router_activities().len() as u64,
            bytes: summary.bytes(),
            flits: summary.flits(),
            occupied_ticks: summary.occupied_ticks(),
            queue_delay_ticks: summary.queue_delay_ticks(),
            max_queue_delay_ticks: summary.max_queue_delay_ticks(),
            credit_delay_ticks: summary.credit_delay_ticks(),
            max_credit_delay_ticks: summary.max_credit_delay_ticks(),
            contended_lanes: summary.contended_lanes(),
            link_activities: summary.link_activities().to_vec(),
            lane_activities: summary.lane_activities().to_vec(),
            hop_activities: summary.hop_activities().to_vec(),
            router_activities: summary.router_activities().to_vec(),
        }
    }

    pub(crate) fn link_activities(&self) -> &[FabricLinkActivity] {
        &self.link_activities
    }

    pub(crate) fn lane_activities(&self) -> &[FabricLaneActivity] {
        &self.lane_activities
    }

    pub(crate) fn hop_activities(&self) -> &[FabricHopActivity] {
        &self.hop_activities
    }

    pub(crate) fn router_activities(&self) -> &[Rem6RunFabricRouterActivity] {
        &self.router_activities
    }

    pub(crate) fn virtual_network_activities(&self) -> Vec<FabricVirtualNetworkActivity> {
        FabricVirtualNetworkActivity::from_lanes(self.lane_activities.iter())
    }
}

fn active_fabric_hop_count(hop_activities: &[FabricHopActivity]) -> u64 {
    hop_activities
        .iter()
        .map(|activity| {
            (
                activity.link().clone(),
                activity.virtual_network(),
                activity.hop_index(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

impl Rem6DramResourceSummary {
    fn from_dram(summary: &Rem6DramSummary) -> Self {
        let activity = summary
            .accesses
            .max(summary.reads.saturating_add(summary.writes))
            .max(summary.row_hits.saturating_add(summary.row_misses))
            .max(summary.commands)
            .max(summary.refreshes)
            .max(summary.turnarounds)
            .max(
                summary
                    .low_power_active_powerdown_entries
                    .saturating_add(summary.low_power_precharge_powerdown_entries)
                    .saturating_add(summary.low_power_self_refresh_entries),
            )
            .max(summary.low_power_exits);
        let active = summary
            .active_targets
            .max(summary.active_ports)
            .max(summary.active_banks)
            .max(u64::from(activity != 0));

        Self {
            activity,
            active,
            active_targets: summary.active_targets,
            active_ports: summary.active_ports,
            active_banks: summary.active_banks,
            profiled_targets: summary.profiled_targets,
            accesses: summary.accesses,
            reads: summary.reads,
            writes: summary.writes,
            read_bytes: summary.read_bytes,
            write_bytes: summary.write_bytes,
            row_hits: summary.row_hits,
            read_row_hits: summary.read_row_hits,
            write_row_hits: summary.write_row_hits,
            row_misses: summary.row_misses,
            refreshes: summary.refreshes,
            refresh_ticks: summary.refresh_ticks,
            commands: summary.commands,
            turnarounds: summary.turnarounds,
            total_ready_latency_ticks: summary.total_ready_latency_ticks,
            read_ready_latency_ticks: summary.read_ready_latency_ticks,
            max_ready_latency_ticks: summary.max_ready_latency_ticks,
            low_power_active_powerdown_entries: summary.low_power_active_powerdown_entries,
            low_power_active_powerdown_ticks: summary.low_power_active_powerdown_ticks,
            low_power_precharge_powerdown_entries: summary.low_power_precharge_powerdown_entries,
            low_power_precharge_powerdown_ticks: summary.low_power_precharge_powerdown_ticks,
            low_power_self_refresh_entries: summary.low_power_self_refresh_entries,
            low_power_self_refresh_ticks: summary.low_power_self_refresh_ticks,
            low_power_exits: summary.low_power_exits,
            low_power_exit_latency_ticks: summary.low_power_exit_latency_ticks,
            targets: summary.targets.clone(),
        }
    }
}

impl Rem6MemoryResourceSummary {
    pub(crate) fn from_run_resources(inputs: Rem6MemoryResourceInputs<'_>) -> Self {
        let cache = Rem6CacheResourceSummary::from_cache_summaries(inputs.cache_summaries());
        let cache_l1 = Rem6CacheResourceSummary::from_cache_summaries([
            inputs.instruction_caches[0],
            inputs.data_caches[0],
        ]);
        let cache_l2 = Rem6CacheResourceSummary::from_cache_summaries([
            inputs.instruction_caches[1],
            inputs.data_caches[1],
        ]);
        let cache_l3 = Rem6CacheResourceSummary::from_cache_summaries([
            inputs.instruction_caches[2],
            inputs.data_caches[2],
        ]);
        let cache_instruction =
            Rem6CacheResourceHierarchySummary::from_cache_levels(inputs.instruction_caches);
        let cache_data = Rem6CacheResourceHierarchySummary::from_cache_levels(inputs.data_caches);
        let transport_fetch = Rem6TransportResourceSummary::from_transport(inputs.fetch_transport);
        let transport_data = Rem6TransportResourceSummary::from_transport(inputs.data_transport);
        let transport = Rem6TransportResourceSummary::combine([
            transport_fetch.clone(),
            transport_data.clone(),
        ]);
        let fabric = Rem6FabricResourceSummary::from_fabric(inputs.fabric);
        let dram = Rem6DramResourceSummary::from_dram(inputs.dram);

        Self {
            activity: cache
                .activity
                .saturating_add(transport.activity)
                .saturating_add(fabric.activity)
                .saturating_add(dram.activity),
            active: cache
                .active
                .saturating_add(transport.active)
                .saturating_add(fabric.active)
                .saturating_add(dram.active),
            cache,
            cache_l1,
            cache_l2,
            cache_l3,
            cache_instruction,
            cache_data,
            transport,
            transport_fetch,
            transport_data,
            fabric,
            dram,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dram_resource_activity_counts_refresh_only_activity() {
        let summary = Rem6DramSummary {
            refreshes: 7,
            refresh_ticks: 91,
            ..Rem6DramSummary::default()
        };

        let resource = Rem6DramResourceSummary::from_dram(&summary);

        assert_eq!(resource.activity, 7);
        assert_eq!(resource.active, 1);
        assert_eq!(resource.refreshes, 7);
        assert_eq!(resource.refresh_ticks, 91);
    }

    #[test]
    fn dram_resource_activity_counts_low_power_only_activity() {
        let summary = Rem6DramSummary {
            low_power_active_powerdown_entries: 2,
            low_power_precharge_powerdown_entries: 3,
            low_power_self_refresh_entries: 5,
            low_power_exits: 4,
            ..Rem6DramSummary::default()
        };

        let resource = Rem6DramResourceSummary::from_dram(&summary);

        assert_eq!(resource.activity, 10);
        assert_eq!(resource.active, 1);
        assert_eq!(resource.low_power_active_powerdown_entries, 2);
        assert_eq!(resource.low_power_precharge_powerdown_entries, 3);
        assert_eq!(resource.low_power_self_refresh_entries, 5);
        assert_eq!(resource.low_power_exits, 4);
    }

    #[test]
    fn dram_resource_preserves_profiled_targets_for_internal_consumers() {
        let summary = Rem6DramSummary {
            profiled_targets: 3,
            ..Rem6DramSummary::default()
        };

        let resource = Rem6DramResourceSummary::from_dram(&summary);

        assert_eq!(resource.profiled_targets, 3);
    }
}
