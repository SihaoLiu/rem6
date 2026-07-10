use rem6_cpu::{O3RuntimeLsqOrdering, O3RuntimeTraceRecord};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::helpers::register_o3_counter;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventWindowRow {
    sequence: u64,
    tick: u64,
    issue_tick: u64,
    writeback_tick: u64,
    commit_tick: u64,
    issue_to_writeback_ticks: u64,
    writeback_to_commit_ticks: u64,
    issue_to_commit_ticks: u64,
    pc: u64,
    rob_occupancy: u64,
    rob_commits_at_tick: u64,
    rob_commit_blocked: u64,
    lsq_occupancy: u64,
    lsq_ordering: Option<O3RuntimeLsqOrdering>,
    rename_map_entries: u64,
    lsq_data_latency_ticks: u64,
    fu_latency_cycles: u64,
}

impl O3EventWindowRow {
    fn from_event(event: &O3RuntimeTraceRecord) -> Self {
        Self {
            sequence: event.sequence(),
            tick: event.tick(),
            issue_tick: event.issue_tick(),
            writeback_tick: event.writeback_tick(),
            commit_tick: event.commit_tick(),
            issue_to_writeback_ticks: event.issue_to_writeback_ticks(),
            writeback_to_commit_ticks: event.writeback_to_commit_ticks(),
            issue_to_commit_ticks: event.issue_to_commit_ticks(),
            pc: event.pc().get(),
            rob_occupancy: event.rob_occupancy(),
            rob_commits_at_tick: event.rob_commits_at_tick(),
            rob_commit_blocked: u64::from(event.rob_commit_blocked()),
            lsq_occupancy: event.lsq_occupancy(),
            lsq_ordering: Some(event.lsq_ordering()),
            rename_map_entries: event.rename_map_entries(),
            lsq_data_latency_ticks: event.lsq_data_latency_ticks(),
            fu_latency_cycles: event.fu_latency_cycles(),
        }
    }

    fn structural_pressure_key(self) -> (u64, u64, u64, u64, u64, u64) {
        let active_structures = u64::from(self.rob_occupancy != 0)
            + u64::from(self.lsq_occupancy != 0)
            + u64::from(self.rename_map_entries != 0);
        (
            active_structures,
            self.rob_occupancy
                .saturating_add(self.lsq_occupancy)
                .saturating_add(self.rename_map_entries),
            self.rob_occupancy,
            self.lsq_occupancy,
            self.rename_map_entries,
            self.sequence,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventWindowSnapshot {
    records: u64,
    first: O3EventWindowRow,
    last: O3EventWindowRow,
    max_rob_occupancy: O3EventWindowRow,
    max_lsq_occupancy: O3EventWindowRow,
    max_rename_map_entries: O3EventWindowRow,
    max_structural_pressure: O3EventWindowRow,
    max_lsq_data_latency: O3EventWindowRow,
    max_fu_latency: O3EventWindowRow,
}

impl RiscvO3RuntimeEventWindowSnapshot {
    pub(super) fn observe(&mut self, event: &O3RuntimeTraceRecord) {
        let row = O3EventWindowRow::from_event(event);
        if self.records == 0 {
            self.first = row;
            self.last = row;
            self.max_rob_occupancy = row;
            self.max_lsq_occupancy = row;
            self.max_rename_map_entries = row;
            self.max_structural_pressure = row;
            self.max_lsq_data_latency = row;
            self.max_fu_latency = row;
            self.records = 1;
            return;
        }

        if row.sequence <= self.last.sequence {
            self.update_existing(row);
            return;
        }

        self.records = self.records.saturating_add(1);
        self.last = row;
        self.update_new(row);
    }

    fn update_new(&mut self, row: O3EventWindowRow) {
        if row.rob_occupancy >= self.max_rob_occupancy.rob_occupancy {
            self.max_rob_occupancy = row;
        }
        if row.lsq_occupancy >= self.max_lsq_occupancy.lsq_occupancy {
            self.max_lsq_occupancy = row;
        }
        if row.rename_map_entries >= self.max_rename_map_entries.rename_map_entries {
            self.max_rename_map_entries = row;
        }
        if row.structural_pressure_key() >= self.max_structural_pressure.structural_pressure_key() {
            self.max_structural_pressure = row;
        }
        if row.lsq_data_latency_ticks >= self.max_lsq_data_latency.lsq_data_latency_ticks {
            self.max_lsq_data_latency = row;
        }
        if row.fu_latency_cycles >= self.max_fu_latency.fu_latency_cycles {
            self.max_fu_latency = row;
        }
    }

    fn update_existing(&mut self, row: O3EventWindowRow) {
        if row.sequence == self.first.sequence {
            self.first = row;
        }
        if row.sequence == self.last.sequence {
            self.last = row;
        }
        if row.sequence == self.max_rob_occupancy.sequence {
            self.max_rob_occupancy = row;
        }
        if row.sequence == self.max_lsq_occupancy.sequence {
            self.max_lsq_occupancy = row;
        }
        if row.sequence == self.max_rename_map_entries.sequence {
            self.max_rename_map_entries = row;
        }
        if row.sequence == self.max_structural_pressure.sequence
            || row.structural_pressure_key()
                > self.max_structural_pressure.structural_pressure_key()
        {
            self.max_structural_pressure = row;
        }
        if row.sequence == self.max_lsq_data_latency.sequence
            || row.lsq_data_latency_ticks > self.max_lsq_data_latency.lsq_data_latency_ticks
        {
            self.max_lsq_data_latency = row;
        }
        if row.sequence == self.max_fu_latency.sequence {
            self.max_fu_latency = row;
        }
    }

    fn span_ticks(self) -> u64 {
        self.last.tick.saturating_sub(self.first.tick)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeEventWindowRowStats {
    sequence: StatId,
    tick: StatId,
    issue_tick: StatId,
    writeback_tick: StatId,
    commit_tick: StatId,
    issue_to_writeback_ticks: StatId,
    writeback_to_commit_ticks: StatId,
    issue_to_commit_ticks: StatId,
    pc: StatId,
    rob_occupancy: StatId,
    rob_commits_at_tick: StatId,
    rob_commit_blocked: StatId,
    lsq_occupancy: StatId,
    lsq_ordering: [StatId; O3RuntimeLsqOrdering::TRACKED.len()],
    rename_map_entries: StatId,
    lsq_data_latency_ticks: StatId,
    fu_latency_cycles: StatId,
}

impl RiscvO3RuntimeEventWindowRowStats {
    fn register(registry: &mut StatsRegistry, prefix: &str, row: &str) -> Result<Self, StatsError> {
        let prefix = format!("{prefix}.event_window.{row}");
        Ok(Self {
            sequence: register_o3_counter(registry, &prefix, "sequence", "Count")?,
            tick: register_o3_counter(registry, &prefix, "tick", "Tick")?,
            issue_tick: register_o3_counter(registry, &prefix, "issue_tick", "Tick")?,
            writeback_tick: register_o3_counter(registry, &prefix, "writeback_tick", "Tick")?,
            commit_tick: register_o3_counter(registry, &prefix, "commit_tick", "Tick")?,
            issue_to_writeback_ticks: register_o3_counter(
                registry,
                &prefix,
                "issue_to_writeback_ticks",
                "Tick",
            )?,
            writeback_to_commit_ticks: register_o3_counter(
                registry,
                &prefix,
                "writeback_to_commit_ticks",
                "Tick",
            )?,
            issue_to_commit_ticks: register_o3_counter(
                registry,
                &prefix,
                "issue_to_commit_ticks",
                "Tick",
            )?,
            pc: register_o3_counter(registry, &prefix, "pc", "Address")?,
            rob_occupancy: register_o3_counter(registry, &prefix, "rob_occupancy", "Count")?,
            rob_commits_at_tick: register_o3_counter(
                registry,
                &prefix,
                "rob_commits_at_tick",
                "Count",
            )?,
            rob_commit_blocked: register_o3_counter(
                registry,
                &prefix,
                "rob_commit_blocked",
                "Count",
            )?,
            lsq_occupancy: register_o3_counter(registry, &prefix, "lsq_occupancy", "Count")?,
            lsq_ordering: [
                register_o3_counter(registry, &prefix, "lsq_ordering.acquire", "Count")?,
                register_o3_counter(registry, &prefix, "lsq_ordering.release", "Count")?,
                register_o3_counter(registry, &prefix, "lsq_ordering.acquire_release", "Count")?,
            ],
            rename_map_entries: register_o3_counter(
                registry,
                &prefix,
                "rename_map_entries",
                "Count",
            )?,
            lsq_data_latency_ticks: register_o3_counter(
                registry,
                &prefix,
                "lsq_data_latency_ticks",
                "Tick",
            )?,
            fu_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency_cycles",
                "Cycle",
            )?,
        })
    }

    fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        row: O3EventWindowRow,
    ) -> Result<(), StatsError> {
        for (stat, value) in [
            (self.sequence, row.sequence),
            (self.tick, row.tick),
            (self.issue_tick, row.issue_tick),
            (self.writeback_tick, row.writeback_tick),
            (self.commit_tick, row.commit_tick),
            (self.issue_to_writeback_ticks, row.issue_to_writeback_ticks),
            (
                self.writeback_to_commit_ticks,
                row.writeback_to_commit_ticks,
            ),
            (self.issue_to_commit_ticks, row.issue_to_commit_ticks),
            (self.pc, row.pc),
            (self.rob_occupancy, row.rob_occupancy),
            (self.rob_commits_at_tick, row.rob_commits_at_tick),
            (self.rob_commit_blocked, row.rob_commit_blocked),
            (self.lsq_occupancy, row.lsq_occupancy),
            (self.rename_map_entries, row.rename_map_entries),
            (self.lsq_data_latency_ticks, row.lsq_data_latency_ticks),
            (self.fu_latency_cycles, row.fu_latency_cycles),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        for (stat, ordering) in self
            .lsq_ordering
            .into_iter()
            .zip(O3RuntimeLsqOrdering::TRACKED)
        {
            registry.set_resettable_counter(stat, u64::from(row.lsq_ordering == Some(ordering)))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventWindowStats {
    records: StatId,
    span_ticks: StatId,
    first: RiscvO3RuntimeEventWindowRowStats,
    last: RiscvO3RuntimeEventWindowRowStats,
    max_rob_occupancy: RiscvO3RuntimeEventWindowRowStats,
    max_lsq_occupancy: RiscvO3RuntimeEventWindowRowStats,
    max_rename_map_entries: RiscvO3RuntimeEventWindowRowStats,
    max_structural_pressure: RiscvO3RuntimeEventWindowRowStats,
    max_lsq_data_latency: RiscvO3RuntimeEventWindowRowStats,
    max_fu_latency: RiscvO3RuntimeEventWindowRowStats,
}

impl RiscvO3RuntimeEventWindowStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        let event_window_prefix = format!("{prefix}.event_window");
        Ok(Self {
            records: register_o3_counter(registry, &event_window_prefix, "records", "Count")?,
            span_ticks: register_o3_counter(registry, &event_window_prefix, "span_ticks", "Tick")?,
            first: RiscvO3RuntimeEventWindowRowStats::register(registry, prefix, "first")?,
            last: RiscvO3RuntimeEventWindowRowStats::register(registry, prefix, "last")?,
            max_rob_occupancy: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_rob_occupancy",
            )?,
            max_lsq_occupancy: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_lsq_occupancy",
            )?,
            max_rename_map_entries: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_rename_map_entries",
            )?,
            max_structural_pressure: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_structural_pressure",
            )?,
            max_lsq_data_latency: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_lsq_data_latency",
            )?,
            max_fu_latency: RiscvO3RuntimeEventWindowRowStats::register(
                registry,
                prefix,
                "max_fu_latency",
            )?,
        })
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: RiscvO3RuntimeEventWindowSnapshot,
    ) -> Result<(), StatsError> {
        registry.set_resettable_counter(self.records, snapshot.records)?;
        registry.set_resettable_counter(self.span_ticks, snapshot.span_ticks())?;
        self.first.set_snapshot(registry, snapshot.first)?;
        self.last.set_snapshot(registry, snapshot.last)?;
        self.max_rob_occupancy
            .set_snapshot(registry, snapshot.max_rob_occupancy)?;
        self.max_lsq_occupancy
            .set_snapshot(registry, snapshot.max_lsq_occupancy)?;
        self.max_rename_map_entries
            .set_snapshot(registry, snapshot.max_rename_map_entries)?;
        self.max_structural_pressure
            .set_snapshot(registry, snapshot.max_structural_pressure)?;
        self.max_lsq_data_latency
            .set_snapshot(registry, snapshot.max_lsq_data_latency)?;
        self.max_fu_latency
            .set_snapshot(registry, snapshot.max_fu_latency)?;
        Ok(())
    }
}
