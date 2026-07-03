use rem6_cpu::{CpuId, O3RuntimeStats, RiscvCluster};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rem6O3TraceRecord {
    cpu: u32,
    stats: O3RuntimeStats,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6O3TraceStat {
    suffix: &'static str,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6O3TraceTotals {
    records: u64,
    instructions: u64,
    rob_allocations: u64,
    rob_commits: u64,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    store_load_forwarding_candidates: u64,
    store_load_forwarding_matches: u64,
    fu_latency_instructions: u64,
    fu_latency_cycles: u64,
    max_rob_occupancy: u64,
    max_lsq_occupancy: u64,
    rename_map_entries: u64,
}

impl Rem6O3TraceRecord {
    fn new(cpu: CpuId, stats: O3RuntimeStats) -> Self {
        Self {
            cpu: cpu.get(),
            stats,
        }
    }

    pub(super) const fn cpu(self) -> u32 {
        self.cpu
    }

    pub(super) fn stats(self) -> O3RuntimeStats {
        self.stats
    }

    pub(super) fn to_json(self) -> String {
        format!(
            "{{\"cpu\":{},\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{}}}",
            self.cpu,
            self.stats.instructions(),
            self.stats.rob_allocations(),
            self.stats.rob_commits(),
            self.stats.rename_writes(),
            self.stats.lsq_loads(),
            self.stats.lsq_stores(),
            self.stats.lsq_store_to_load_forwarding_candidates(),
            self.stats.lsq_store_to_load_forwarding_matches(),
            self.stats.fu_latency_instructions(),
            self.stats.fu_latency_cycles(),
            self.stats.max_rob_occupancy(),
            self.stats.max_lsq_occupancy(),
            self.stats.rename_map_entries(),
        )
    }
}

impl Rem6O3TraceStat {
    pub(crate) const fn suffix(self) -> &'static str {
        self.suffix
    }

    pub(crate) const fn unit(self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(self) -> u64 {
        self.value
    }
}

pub(super) fn o3_trace_records(cluster: &RiscvCluster, core_count: u32) -> Vec<Rem6O3TraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        let stats = core.o3_runtime_stats();
        if stats.has_activity() {
            records.push(Rem6O3TraceRecord::new(cpu, stats));
        }
    }
    records.sort_by_key(|record| record.cpu());
    records
}

pub(super) fn o3_trace_stats(records: &[Rem6O3TraceRecord]) -> Vec<Rem6O3TraceStat> {
    let mut totals = Rem6O3TraceTotals::default();
    for record in records {
        totals.add(record.stats());
    }
    totals.stats()
}

impl Rem6O3TraceTotals {
    fn add(&mut self, stats: O3RuntimeStats) {
        self.records = self.records.saturating_add(1);
        self.instructions = self.instructions.saturating_add(stats.instructions());
        self.rob_allocations = self.rob_allocations.saturating_add(stats.rob_allocations());
        self.rob_commits = self.rob_commits.saturating_add(stats.rob_commits());
        self.rename_writes = self.rename_writes.saturating_add(stats.rename_writes());
        self.lsq_loads = self.lsq_loads.saturating_add(stats.lsq_loads());
        self.lsq_stores = self.lsq_stores.saturating_add(stats.lsq_stores());
        self.store_load_forwarding_candidates = self
            .store_load_forwarding_candidates
            .saturating_add(stats.lsq_store_to_load_forwarding_candidates());
        self.store_load_forwarding_matches = self
            .store_load_forwarding_matches
            .saturating_add(stats.lsq_store_to_load_forwarding_matches());
        self.fu_latency_instructions = self
            .fu_latency_instructions
            .saturating_add(stats.fu_latency_instructions());
        self.fu_latency_cycles = self
            .fu_latency_cycles
            .saturating_add(stats.fu_latency_cycles());
        self.max_rob_occupancy = self.max_rob_occupancy.max(stats.max_rob_occupancy());
        self.max_lsq_occupancy = self.max_lsq_occupancy.max(stats.max_lsq_occupancy());
        self.rename_map_entries = self
            .rename_map_entries
            .saturating_add(stats.rename_map_entries());
    }

    fn stats(self) -> Vec<Rem6O3TraceStat> {
        let mut stats = Vec::new();
        for (suffix, value) in [
            ("records", self.records),
            ("instructions", self.instructions),
            ("rob_allocations", self.rob_allocations),
            ("rob_commits", self.rob_commits),
            ("rename_writes", self.rename_writes),
            ("lsq_loads", self.lsq_loads),
            ("lsq_stores", self.lsq_stores),
            (
                "store_load_forwarding_candidates",
                self.store_load_forwarding_candidates,
            ),
            (
                "store_load_forwarding_matches",
                self.store_load_forwarding_matches,
            ),
            ("fu_latency_instructions", self.fu_latency_instructions),
            ("max_rob_occupancy", self.max_rob_occupancy),
            ("max_lsq_occupancy", self.max_lsq_occupancy),
            ("rename_map_entries", self.rename_map_entries),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        stats.push(Rem6O3TraceStat {
            suffix: "fu_latency_cycles",
            unit: "Cycle",
            value: self.fu_latency_cycles,
        });
        stats
    }
}
