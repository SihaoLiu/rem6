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
    pub(crate) transport_activity: u64,
    pub(crate) active_transports: u64,
    pub(crate) fabric_activity: u64,
    pub(crate) active_fabric_resources: u64,
    pub(crate) dram_activity: u64,
    pub(crate) active_dram_resources: u64,
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
        let transport_activity = inputs
            .fetch_transport
            .counters
            .requests
            .saturating_add(inputs.data_transport.counters.requests);
        let active_transports = u64::from(inputs.fetch_transport.counters.requests != 0)
            + u64::from(inputs.data_transport.counters.requests != 0);
        let fabric_activity = inputs.fabric.transfers();
        let active_fabric_resources = inputs.fabric.active_lanes();
        let dram = inputs.dram;
        let dram_activity = dram
            .accesses
            .max(dram.reads.saturating_add(dram.writes))
            .max(dram.row_hits.saturating_add(dram.row_misses))
            .max(dram.commands)
            .max(dram.refreshes)
            .max(dram.turnarounds)
            .max(
                dram.low_power_active_powerdown_entries
                    .saturating_add(dram.low_power_precharge_powerdown_entries)
                    .saturating_add(dram.low_power_self_refresh_entries),
            )
            .max(dram.low_power_exits);
        let active_dram_resources = dram
            .active_targets
            .max(dram.active_ports)
            .max(dram.active_banks)
            .max(u64::from(dram_activity != 0));

        Self {
            activity: cache
                .activity
                .saturating_add(transport_activity)
                .saturating_add(fabric_activity)
                .saturating_add(dram_activity),
            active: cache
                .active
                .saturating_add(active_transports)
                .saturating_add(active_fabric_resources)
                .saturating_add(active_dram_resources),
            cache,
            cache_l1,
            cache_l2,
            cache_l3,
            transport_activity,
            active_transports,
            fabric_activity,
            active_fabric_resources,
            dram_activity,
            active_dram_resources,
        }
    }
}
