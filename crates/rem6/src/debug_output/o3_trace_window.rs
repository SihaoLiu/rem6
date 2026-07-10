use super::*;

macro_rules! push_event_window_row_stats {
    ($stats:expr, $row:expr, $prefix:literal) => {{
        let row = $row.unwrap_or_default();
        for (suffix, unit, value) in [
            (concat!($prefix, ".sequence"), "Count", row.sequence),
            (concat!($prefix, ".tick"), "Tick", row.tick),
            (concat!($prefix, ".issue_tick"), "Tick", row.issue_tick),
            (
                concat!($prefix, ".writeback_tick"),
                "Tick",
                row.writeback_tick,
            ),
            (concat!($prefix, ".commit_tick"), "Tick", row.commit_tick),
            (
                concat!($prefix, ".issue_to_writeback_ticks"),
                "Tick",
                row.issue_to_writeback_ticks,
            ),
            (
                concat!($prefix, ".writeback_to_commit_ticks"),
                "Tick",
                row.writeback_to_commit_ticks,
            ),
            (
                concat!($prefix, ".issue_to_commit_ticks"),
                "Tick",
                row.issue_to_commit_ticks,
            ),
            (concat!($prefix, ".pc"), "Address", row.pc),
            (
                concat!($prefix, ".rob_occupancy"),
                "Count",
                row.rob_occupancy,
            ),
            (
                concat!($prefix, ".rob_commits_at_tick"),
                "Count",
                row.rob_commits_at_tick,
            ),
            (
                concat!($prefix, ".rob_commit_blocked"),
                "Count",
                row.rob_commit_blocked,
            ),
            (
                concat!($prefix, ".lsq_occupancy"),
                "Count",
                row.lsq_occupancy,
            ),
            (
                concat!($prefix, ".rename_map_entries"),
                "Count",
                row.rename_map_entries,
            ),
            (
                concat!($prefix, ".lsq_data_latency_ticks"),
                "Tick",
                row.lsq_data_latency_ticks,
            ),
            (
                concat!($prefix, ".fu_latency_cycles"),
                "Cycle",
                row.fu_latency_cycles,
            ),
        ] {
            $stats.push(Rem6O3TraceStat {
                suffix,
                unit,
                value,
            });
        }
        for (suffix, value) in [
            (
                concat!($prefix, ".lsq_operation.load"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::Load),
            ),
            (
                concat!($prefix, ".lsq_operation.store"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::Store),
            ),
            (
                concat!($prefix, ".lsq_operation.load_reserved"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::LoadReserved),
            ),
            (
                concat!($prefix, ".lsq_operation.store_conditional"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::StoreConditional),
            ),
            (
                concat!($prefix, ".lsq_operation.atomic"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::Atomic),
            ),
            (
                concat!($prefix, ".lsq_operation.float_load"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::FloatLoad),
            ),
            (
                concat!($prefix, ".lsq_operation.float_store"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::FloatStore),
            ),
            (
                concat!($prefix, ".lsq_operation.vector_load"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::VectorLoad),
            ),
            (
                concat!($prefix, ".lsq_operation.vector_store"),
                u64::from(row.lsq_operation == O3RuntimeLsqOperation::VectorStore),
            ),
            (
                concat!($prefix, ".lsq_ordering.acquire"),
                u64::from(row.lsq_ordering == O3RuntimeLsqOrdering::Acquire),
            ),
            (
                concat!($prefix, ".lsq_ordering.release"),
                u64::from(row.lsq_ordering == O3RuntimeLsqOrdering::Release),
            ),
            (
                concat!($prefix, ".lsq_ordering.acquire_release"),
                u64::from(row.lsq_ordering == O3RuntimeLsqOrdering::AcquireRelease),
            ),
        ] {
            $stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
    }};
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rem6O3TraceWindowRow {
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
    lsq_operation: O3RuntimeLsqOperation,
    lsq_ordering: O3RuntimeLsqOrdering,
    rename_map_entries: u64,
    lsq_data_latency_ticks: u64,
    fu_latency_cycles: u64,
}

impl Default for Rem6O3TraceWindowRow {
    fn default() -> Self {
        Self {
            sequence: 0,
            tick: 0,
            issue_tick: 0,
            writeback_tick: 0,
            commit_tick: 0,
            issue_to_writeback_ticks: 0,
            writeback_to_commit_ticks: 0,
            issue_to_commit_ticks: 0,
            pc: 0,
            rob_occupancy: 0,
            rob_commits_at_tick: 0,
            rob_commit_blocked: 0,
            lsq_occupancy: 0,
            lsq_operation: O3RuntimeLsqOperation::None,
            lsq_ordering: O3RuntimeLsqOrdering::None,
            rename_map_entries: 0,
            lsq_data_latency_ticks: 0,
            fu_latency_cycles: 0,
        }
    }
}

impl Rem6O3TraceWindowRow {
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
            lsq_operation: event.lsq_operation(),
            lsq_ordering: event.lsq_ordering(),
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
pub(super) struct Rem6O3TraceWindowRows {
    first: Option<Rem6O3TraceWindowRow>,
    last: Option<Rem6O3TraceWindowRow>,
    max_rob_occupancy: Option<Rem6O3TraceWindowRow>,
    max_lsq_occupancy: Option<Rem6O3TraceWindowRow>,
    max_rename_map_entries: Option<Rem6O3TraceWindowRow>,
    max_structural_pressure: Option<Rem6O3TraceWindowRow>,
    max_lsq_data_latency: Option<Rem6O3TraceWindowRow>,
    max_fu_latency: Option<Rem6O3TraceWindowRow>,
}

impl Rem6O3TraceWindowRows {
    pub(super) fn add(&mut self, event: &O3RuntimeTraceRecord) {
        let row = Rem6O3TraceWindowRow::from_event(event);
        if self.first.is_none_or(|current| row.tick < current.tick) {
            self.first = Some(row);
        }
        if self.last.is_none_or(|current| row.tick >= current.tick) {
            self.last = Some(row);
        }
        if self
            .max_rob_occupancy
            .is_none_or(|current| row.rob_occupancy >= current.rob_occupancy)
        {
            self.max_rob_occupancy = Some(row);
        }
        if self
            .max_lsq_occupancy
            .is_none_or(|current| row.lsq_occupancy >= current.lsq_occupancy)
        {
            self.max_lsq_occupancy = Some(row);
        }
        if self
            .max_rename_map_entries
            .is_none_or(|current| row.rename_map_entries >= current.rename_map_entries)
        {
            self.max_rename_map_entries = Some(row);
        }
        if self.max_structural_pressure.is_none_or(|current| {
            row.structural_pressure_key() >= current.structural_pressure_key()
        }) {
            self.max_structural_pressure = Some(row);
        }
        if self
            .max_lsq_data_latency
            .is_none_or(|current| row.lsq_data_latency_ticks >= current.lsq_data_latency_ticks)
        {
            self.max_lsq_data_latency = Some(row);
        }
        if self
            .max_fu_latency
            .is_none_or(|current| row.fu_latency_cycles >= current.fu_latency_cycles)
        {
            self.max_fu_latency = Some(row);
        }
    }

    pub(super) fn push_stats(
        self,
        stats: &mut Vec<Rem6O3TraceStat>,
        records: u64,
        span_ticks: u64,
    ) {
        for (suffix, unit, value) in [
            ("event_window.records", "Count", records),
            ("event_window.span_ticks", "Tick", span_ticks),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit,
                value,
            });
        }
        push_event_window_row_stats!(stats, self.first, "event_window.first");
        push_event_window_row_stats!(stats, self.last, "event_window.last");
        push_event_window_row_stats!(
            stats,
            self.max_rob_occupancy,
            "event_window.max_rob_occupancy"
        );
        push_event_window_row_stats!(
            stats,
            self.max_lsq_occupancy,
            "event_window.max_lsq_occupancy"
        );
        push_event_window_row_stats!(
            stats,
            self.max_rename_map_entries,
            "event_window.max_rename_map_entries"
        );
        push_event_window_row_stats!(
            stats,
            self.max_structural_pressure,
            "event_window.max_structural_pressure"
        );
        push_event_window_row_stats!(
            stats,
            self.max_lsq_data_latency,
            "event_window.max_lsq_data_latency"
        );
        push_event_window_row_stats!(stats, self.max_fu_latency, "event_window.max_fu_latency");
    }
}
