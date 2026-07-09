use rem6_cpu::O3RuntimeTraceRecord;
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::helpers::register_o3_counter;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventWindowRow {
    sequence: u64,
    tick: u64,
    pc: u64,
    rob_occupancy: u64,
    lsq_occupancy: u64,
    rename_map_entries: u64,
    lsq_data_latency_ticks: u64,
    fu_latency_cycles: u64,
}

impl O3EventWindowRow {
    fn from_event(event: &O3RuntimeTraceRecord) -> Self {
        Self {
            sequence: event.sequence(),
            tick: event.tick(),
            pc: event.pc().get(),
            rob_occupancy: event.rob_occupancy(),
            lsq_occupancy: event.lsq_occupancy(),
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
    pc: StatId,
    rob_occupancy: StatId,
    lsq_occupancy: StatId,
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
            pc: register_o3_counter(registry, &prefix, "pc", "Address")?,
            rob_occupancy: register_o3_counter(registry, &prefix, "rob_occupancy", "Count")?,
            lsq_occupancy: register_o3_counter(registry, &prefix, "lsq_occupancy", "Count")?,
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
            (self.pc, row.pc),
            (self.rob_occupancy, row.rob_occupancy),
            (self.lsq_occupancy, row.lsq_occupancy),
            (self.rename_map_entries, row.rename_map_entries),
            (self.lsq_data_latency_ticks, row.lsq_data_latency_ticks),
            (self.fu_latency_cycles, row.fu_latency_cycles),
        ] {
            registry.set_resettable_counter(stat, value)?;
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
