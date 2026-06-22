use crate::{
    data_cache_runtime::CliDataCacheSummary, transport_summary::Rem6MemoryTransportSummary,
    Rem6DramSummary, Rem6RunFabricSummary,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6MemoryResourceSummary {
    pub(crate) activity: u64,
    pub(crate) active: u64,
    pub(crate) cache_activity: u64,
    pub(crate) active_caches: u64,
    pub(crate) cache_bank_accepted: u64,
    pub(crate) cache_bank_immediate_hits: u64,
    pub(crate) cache_bank_scheduled_misses: u64,
    pub(crate) cache_bank_coalesced_misses: u64,
    pub(crate) transport_activity: u64,
    pub(crate) active_transports: u64,
    pub(crate) fabric_activity: u64,
    pub(crate) active_fabric_resources: u64,
    pub(crate) dram_activity: u64,
    pub(crate) active_dram_resources: u64,
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

impl Rem6MemoryResourceSummary {
    pub(crate) fn from_run_resources(inputs: Rem6MemoryResourceInputs<'_>) -> Self {
        let cache_activity = inputs
            .cache_summaries()
            .fold(0_u64, |sum, cache| sum.saturating_add(cache.runs));
        let active_caches = inputs
            .cache_summaries()
            .filter(|cache| cache.runs != 0)
            .count() as u64;
        let cache_bank_accepted = inputs
            .cache_summaries()
            .fold(0_u64, |sum, cache| sum.saturating_add(cache.bank_accepted));
        let cache_bank_immediate_hits = inputs.cache_summaries().fold(0_u64, |sum, cache| {
            sum.saturating_add(cache.bank_immediate_hits)
        });
        let cache_bank_scheduled_misses = inputs.cache_summaries().fold(0_u64, |sum, cache| {
            sum.saturating_add(cache.bank_scheduled_misses)
        });
        let cache_bank_coalesced_misses = inputs.cache_summaries().fold(0_u64, |sum, cache| {
            sum.saturating_add(cache.bank_coalesced_misses)
        });
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
            activity: cache_activity
                .saturating_add(transport_activity)
                .saturating_add(fabric_activity)
                .saturating_add(dram_activity),
            active: active_caches
                .saturating_add(active_transports)
                .saturating_add(active_fabric_resources)
                .saturating_add(active_dram_resources),
            cache_activity,
            active_caches,
            cache_bank_accepted,
            cache_bank_immediate_hits,
            cache_bank_scheduled_misses,
            cache_bank_coalesced_misses,
            transport_activity,
            active_transports,
            fabric_activity,
            active_fabric_resources,
            dram_activity,
            active_dram_resources,
        }
    }
}
