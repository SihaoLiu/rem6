use rem6_cpu::{
    CpuId, O3RuntimeFuLatencyClass, O3RuntimeStats, O3RuntimeTraceRecord, RiscvCluster,
};

use crate::{formatting::json_escape, Rem6HostExecutionModeSummary};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6O3TraceRecord {
    cpu: u32,
    target: String,
    execution_mode: Option<&'static str>,
    stats: O3RuntimeStats,
    events: Vec<O3RuntimeTraceRecord>,
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
    lsq_load_bytes: u64,
    lsq_store_bytes: u64,
    store_load_forwarding_candidates: u64,
    store_load_forwarding_matches: u64,
    fu_latency_instructions: u64,
    fu_latency_cycles: u64,
    fu_integer_mul_instructions: u64,
    fu_integer_mul_latency_cycles: u64,
    fu_integer_div_instructions: u64,
    fu_integer_div_latency_cycles: u64,
    max_rob_occupancy: u64,
    max_lsq_occupancy: u64,
    rename_map_entries: u64,
    event_records: u64,
    event_rob_allocations: u64,
    event_rob_commits: u64,
    event_rename_writes: u64,
    event_lsq_loads: u64,
    event_lsq_stores: u64,
    event_lsq_load_bytes: u64,
    event_lsq_store_bytes: u64,
    event_store_load_forwarding_candidates: u64,
    event_store_load_forwarding_matches: u64,
    event_fu_latency_cycles: u64,
    event_fu_integer_mul_instructions: u64,
    event_fu_integer_mul_latency_cycles: u64,
    event_fu_integer_div_instructions: u64,
    event_fu_integer_div_latency_cycles: u64,
}

impl Rem6O3TraceRecord {
    fn new(
        cpu: CpuId,
        target: String,
        execution_mode: Option<&'static str>,
        stats: O3RuntimeStats,
        events: Vec<O3RuntimeTraceRecord>,
    ) -> Self {
        Self {
            cpu: cpu.get(),
            target,
            execution_mode,
            stats,
            events,
        }
    }

    pub(super) const fn cpu(&self) -> u32 {
        self.cpu
    }

    pub(super) fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    pub(super) fn events(&self) -> &[O3RuntimeTraceRecord] {
        &self.events
    }

    pub(super) fn to_json(&self) -> String {
        let events = self
            .events
            .iter()
            .map(o3_event_to_json)
            .collect::<Vec<_>>()
            .join(",");
        let execution_mode = self.execution_mode.map_or_else(
            || "null".to_string(),
            |mode| format!("\"{}\"", json_escape(mode)),
        );
        format!(
            "{{\"cpu\":{},\"target\":\"{}\",\"execution_mode\":{},\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"fu_integer_mul_instructions\":{},\"fu_integer_mul_latency_cycles\":{},\"fu_integer_div_instructions\":{},\"fu_integer_div_latency_cycles\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{},\"events\":[{}]}}",
            self.cpu,
            json_escape(&self.target),
            execution_mode,
            self.stats.instructions(),
            self.stats.rob_allocations(),
            self.stats.rob_commits(),
            self.stats.rename_writes(),
            self.stats.lsq_loads(),
            self.stats.lsq_stores(),
            self.stats.lsq_load_bytes(),
            self.stats.lsq_store_bytes(),
            self.stats.lsq_store_to_load_forwarding_candidates(),
            self.stats.lsq_store_to_load_forwarding_matches(),
            self.stats.fu_latency_instructions(),
            self.stats.fu_latency_cycles(),
            self.stats.fu_integer_mul_instructions(),
            self.stats.fu_integer_mul_latency_cycles(),
            self.stats.fu_integer_div_instructions(),
            self.stats.fu_integer_div_latency_cycles(),
            self.stats.max_rob_occupancy(),
            self.stats.max_lsq_occupancy(),
            self.stats.rename_map_entries(),
            events,
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

pub(super) fn o3_trace_records(
    cluster: &RiscvCluster,
    core_count: u32,
    execution_modes: &[Rem6HostExecutionModeSummary],
) -> Vec<Rem6O3TraceRecord> {
    let mut records = Vec::new();
    for cpu_index in 0..core_count {
        let cpu = CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        let stats = core.o3_runtime_stats();
        let events = core.o3_runtime_trace_records();
        if stats.has_activity() || !events.is_empty() {
            let target = format!("cpu{}", cpu.get());
            let execution_mode = execution_modes
                .iter()
                .find(|mode| mode.target == target)
                .map(|mode| mode.mode);
            records.push(Rem6O3TraceRecord::new(
                cpu,
                target,
                execution_mode,
                stats,
                events,
            ));
        }
    }
    records.sort_by_key(|record| record.cpu());
    records
}

pub(super) fn o3_trace_stats(records: &[Rem6O3TraceRecord]) -> Vec<Rem6O3TraceStat> {
    let mut totals = Rem6O3TraceTotals::default();
    for record in records {
        totals.add(record);
    }
    totals.stats()
}

impl Rem6O3TraceTotals {
    fn add(&mut self, record: &Rem6O3TraceRecord) {
        let stats = record.stats();
        self.records = self.records.saturating_add(1);
        self.instructions = self.instructions.saturating_add(stats.instructions());
        self.rob_allocations = self.rob_allocations.saturating_add(stats.rob_allocations());
        self.rob_commits = self.rob_commits.saturating_add(stats.rob_commits());
        self.rename_writes = self.rename_writes.saturating_add(stats.rename_writes());
        self.lsq_loads = self.lsq_loads.saturating_add(stats.lsq_loads());
        self.lsq_stores = self.lsq_stores.saturating_add(stats.lsq_stores());
        self.lsq_load_bytes = self.lsq_load_bytes.saturating_add(stats.lsq_load_bytes());
        self.lsq_store_bytes = self.lsq_store_bytes.saturating_add(stats.lsq_store_bytes());
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
        self.fu_integer_mul_instructions = self
            .fu_integer_mul_instructions
            .saturating_add(stats.fu_integer_mul_instructions());
        self.fu_integer_mul_latency_cycles = self
            .fu_integer_mul_latency_cycles
            .saturating_add(stats.fu_integer_mul_latency_cycles());
        self.fu_integer_div_instructions = self
            .fu_integer_div_instructions
            .saturating_add(stats.fu_integer_div_instructions());
        self.fu_integer_div_latency_cycles = self
            .fu_integer_div_latency_cycles
            .saturating_add(stats.fu_integer_div_latency_cycles());
        self.max_rob_occupancy = self.max_rob_occupancy.max(stats.max_rob_occupancy());
        self.max_lsq_occupancy = self.max_lsq_occupancy.max(stats.max_lsq_occupancy());
        self.rename_map_entries = self
            .rename_map_entries
            .saturating_add(stats.rename_map_entries());
        for event in record.events() {
            self.event_records = self.event_records.saturating_add(1);
            self.event_rob_allocations = self
                .event_rob_allocations
                .saturating_add(u64::from(event.rob_allocated()));
            self.event_rob_commits = self
                .event_rob_commits
                .saturating_add(u64::from(event.rob_committed()));
            self.event_rename_writes = self
                .event_rename_writes
                .saturating_add(event.rename_writes());
            self.event_lsq_loads = self.event_lsq_loads.saturating_add(event.lsq_loads());
            self.event_lsq_stores = self.event_lsq_stores.saturating_add(event.lsq_stores());
            self.event_lsq_load_bytes = self
                .event_lsq_load_bytes
                .saturating_add(event.lsq_load_bytes());
            self.event_lsq_store_bytes = self
                .event_lsq_store_bytes
                .saturating_add(event.lsq_store_bytes());
            self.event_store_load_forwarding_candidates = self
                .event_store_load_forwarding_candidates
                .saturating_add(u64::from(event.store_load_forwarding_candidate()));
            self.event_store_load_forwarding_matches = self
                .event_store_load_forwarding_matches
                .saturating_add(u64::from(event.store_load_forwarding_match()));
            self.event_fu_latency_cycles = self
                .event_fu_latency_cycles
                .saturating_add(event.fu_latency_cycles());
            if event.fu_latency_cycles() > 0 {
                match event.fu_latency_class() {
                    Some(O3RuntimeFuLatencyClass::ScalarIntegerMul) => {
                        self.event_fu_integer_mul_instructions =
                            self.event_fu_integer_mul_instructions.saturating_add(1);
                        self.event_fu_integer_mul_latency_cycles = self
                            .event_fu_integer_mul_latency_cycles
                            .saturating_add(event.fu_latency_cycles());
                    }
                    Some(O3RuntimeFuLatencyClass::ScalarIntegerDiv) => {
                        self.event_fu_integer_div_instructions =
                            self.event_fu_integer_div_instructions.saturating_add(1);
                        self.event_fu_integer_div_latency_cycles = self
                            .event_fu_integer_div_latency_cycles
                            .saturating_add(event.fu_latency_cycles());
                    }
                    None => {}
                }
            }
        }
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
            (
                "fu_integer_mul_instructions",
                self.fu_integer_mul_instructions,
            ),
            (
                "fu_integer_div_instructions",
                self.fu_integer_div_instructions,
            ),
            ("max_rob_occupancy", self.max_rob_occupancy),
            ("max_lsq_occupancy", self.max_lsq_occupancy),
            ("rename_map_entries", self.rename_map_entries),
            ("event.records", self.event_records),
            ("event.rob_allocations", self.event_rob_allocations),
            ("event.rob_commits", self.event_rob_commits),
            ("event.rename_writes", self.event_rename_writes),
            ("event.lsq_loads", self.event_lsq_loads),
            ("event.lsq_stores", self.event_lsq_stores),
            (
                "event.store_load_forwarding_candidates",
                self.event_store_load_forwarding_candidates,
            ),
            (
                "event.store_load_forwarding_matches",
                self.event_store_load_forwarding_matches,
            ),
            (
                "event.fu_integer_mul_instructions",
                self.event_fu_integer_mul_instructions,
            ),
            (
                "event.fu_integer_div_instructions",
                self.event_fu_integer_div_instructions,
            ),
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
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_load_bytes",
            unit: "Byte",
            value: self.lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_store_bytes",
            unit: "Byte",
            value: self.lsq_store_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_load_bytes",
            unit: "Byte",
            value: self.event_lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_store_bytes",
            unit: "Byte",
            value: self.event_lsq_store_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "fu_integer_mul_latency_cycles",
            unit: "Cycle",
            value: self.fu_integer_mul_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "fu_integer_div_latency_cycles",
            unit: "Cycle",
            value: self.fu_integer_div_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_integer_mul_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_integer_mul_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_integer_div_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_integer_div_latency_cycles,
        });
        stats
    }
}

fn o3_event_to_json(event: &O3RuntimeTraceRecord) -> String {
    let fu_latency_class = event.fu_latency_class().map_or_else(
        || "null".to_string(),
        |class| format!("\"{}\"", class.as_str()),
    );
    format!(
        "{{\"sequence\":{},\"pc\":\"0x{:x}\",\"rob_allocated\":{},\"rob_committed\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidate\":{},\"store_load_forwarding_match\":{},\"fu_latency_class\":{},\"fu_latency_cycles\":{},\"system_event\":{}}}",
        event.sequence(),
        event.pc().get(),
        event.rob_allocated(),
        event.rob_committed(),
        event.rename_writes(),
        event.lsq_loads(),
        event.lsq_stores(),
        event.lsq_load_bytes(),
        event.lsq_store_bytes(),
        event.store_load_forwarding_candidate(),
        event.store_load_forwarding_match(),
        fu_latency_class,
        event.fu_latency_cycles(),
        event.system_event(),
    )
}
