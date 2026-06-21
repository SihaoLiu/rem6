use crate::{
    data_cache_runtime::CliDataCacheSummary, transport_summary::Rem6MemoryTransportSummary,
    Rem6DramSummary,
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
    pub(crate) dram_activity: u64,
    pub(crate) active_dram_resources: u64,
}

impl Rem6MemoryResourceSummary {
    pub(crate) fn from_run_resources(
        instruction_cache: &CliDataCacheSummary,
        data_cache: &CliDataCacheSummary,
        fetch_transport: &Rem6MemoryTransportSummary,
        data_transport: &Rem6MemoryTransportSummary,
        dram: &Rem6DramSummary,
    ) -> Self {
        let cache_activity = instruction_cache.runs.saturating_add(data_cache.runs);
        let active_caches =
            u64::from(instruction_cache.runs != 0) + u64::from(data_cache.runs != 0);
        let cache_bank_accepted = instruction_cache
            .bank_accepted
            .saturating_add(data_cache.bank_accepted);
        let cache_bank_immediate_hits = instruction_cache
            .bank_immediate_hits
            .saturating_add(data_cache.bank_immediate_hits);
        let cache_bank_scheduled_misses = instruction_cache
            .bank_scheduled_misses
            .saturating_add(data_cache.bank_scheduled_misses);
        let cache_bank_coalesced_misses = instruction_cache
            .bank_coalesced_misses
            .saturating_add(data_cache.bank_coalesced_misses);
        let transport_activity = fetch_transport
            .counters
            .requests
            .saturating_add(data_transport.counters.requests);
        let active_transports = u64::from(fetch_transport.counters.requests != 0)
            + u64::from(data_transport.counters.requests != 0);
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
                .saturating_add(dram_activity),
            active: active_caches
                .saturating_add(active_transports)
                .saturating_add(active_dram_resources),
            cache_activity,
            active_caches,
            cache_bank_accepted,
            cache_bank_immediate_hits,
            cache_bank_scheduled_misses,
            cache_bank_coalesced_misses,
            transport_activity,
            active_transports,
            dram_activity,
            active_dram_resources,
        }
    }
}
