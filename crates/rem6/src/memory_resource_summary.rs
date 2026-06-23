use crate::{
    data_cache_runtime::CliDataCacheSummary, transport_summary::Rem6MemoryTransportSummary,
    Rem6DramSummary, Rem6RunFabricSummary,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6MemoryResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) cache: Rem6CacheResourceSummary,
    pub(crate) cache_l1: Rem6CacheResourceSummary,
    pub(crate) cache_l2: Rem6CacheResourceSummary,
    pub(crate) cache_l3: Rem6CacheResourceSummary,
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
    pub(crate) cpu_responses: u64,
    pub(crate) directory_decisions: u64,
    pub(crate) dram_accesses: u64,
    pub(crate) bank_accepted: u64,
    pub(crate) bank_immediate_hits: u64,
    pub(crate) bank_scheduled_misses: u64,
    pub(crate) bank_coalesced_misses: u64,
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
    pub(crate) bytes: u64,
    pub(crate) flits: u64,
    pub(crate) occupied_ticks: u64,
    pub(crate) queue_delay_ticks: u64,
    pub(crate) max_queue_delay_ticks: u64,
    pub(crate) credit_delay_ticks: u64,
    pub(crate) max_credit_delay_ticks: u64,
    pub(crate) contended_lanes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DramResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) active_targets: u64,
    pub(crate) active_ports: u64,
    pub(crate) active_banks: u64,
    pub(crate) accesses: u64,
    pub(crate) reads: u64,
    pub(crate) writes: u64,
    pub(crate) row_hits: u64,
    pub(crate) row_misses: u64,
    pub(crate) commands: u64,
    pub(crate) turnarounds: u64,
    pub(crate) total_ready_latency_ticks: u64,
    pub(crate) max_ready_latency_ticks: u64,
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
        caches
            .into_iter()
            .fold(Self::default(), |mut summary, cache| {
                summary.activity = summary.activity.saturating_add(cache.runs);
                summary.active += u64::from(cache.runs != 0);
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
                summary
            })
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
            bytes: summary.bytes(),
            flits: summary.flits(),
            occupied_ticks: summary.occupied_ticks(),
            queue_delay_ticks: summary.queue_delay_ticks(),
            max_queue_delay_ticks: summary.max_queue_delay_ticks(),
            credit_delay_ticks: summary.credit_delay_ticks(),
            max_credit_delay_ticks: summary.max_credit_delay_ticks(),
            contended_lanes: summary.contended_lanes(),
        }
    }
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
            accesses: summary.accesses,
            reads: summary.reads,
            writes: summary.writes,
            row_hits: summary.row_hits,
            row_misses: summary.row_misses,
            commands: summary.commands,
            turnarounds: summary.turnarounds,
            total_ready_latency_ticks: summary.total_ready_latency_ticks,
            max_ready_latency_ticks: summary.max_ready_latency_ticks,
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
    }
}
