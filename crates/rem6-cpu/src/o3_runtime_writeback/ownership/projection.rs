use super::*;

impl O3FinalizedWritebackPortStats {
    pub(in crate::o3_runtime) fn checkpoint_projection(
        &self,
    ) -> crate::RiscvO3LiveCheckpointFinalizedWriteback {
        crate::RiscvO3LiveCheckpointFinalizedWriteback {
            cycles: self.cycles,
            admitted_rows: self.admitted_rows,
            deferred_rows: self.deferred_rows,
            deferred_row_cycles: self.deferred_row_cycles,
            max_ready_rows_per_cycle: self.max_ready_rows_per_cycle,
            max_deferred_rows: self.max_deferred_rows,
            partial_cycle_ticks: self.partial_finalized_cycle_ticks.clone(),
            partial_ready_rows_by_tick: self.partial_finalized_ready_rows_by_tick.clone(),
            partial_deferred_rows_by_tick: self.partial_finalized_deferred_rows_by_tick.clone(),
            closed_before_tick: self.closed_before_tick,
        }
    }

    pub(in crate::o3_runtime) fn from_checkpoint_projection(
        value: &crate::RiscvO3LiveCheckpointFinalizedWriteback,
    ) -> Self {
        Self {
            cycles: value.cycles,
            admitted_rows: value.admitted_rows,
            deferred_rows: value.deferred_rows,
            deferred_row_cycles: value.deferred_row_cycles,
            max_ready_rows_per_cycle: value.max_ready_rows_per_cycle,
            max_deferred_rows: value.max_deferred_rows,
            partial_finalized_cycle_ticks: value.partial_cycle_ticks.clone(),
            partial_finalized_ready_rows_by_tick: value.partial_ready_rows_by_tick.clone(),
            partial_finalized_deferred_rows_by_tick: value.partial_deferred_rows_by_tick.clone(),
            closed_before_tick: value.closed_before_tick,
        }
    }

    pub(in crate::o3_runtime) fn from_aggregate(stats: O3RuntimeStats) -> Self {
        Self {
            cycles: stats.writeback_port_cycles(),
            admitted_rows: stats.writeback_port_admitted_rows(),
            deferred_rows: stats.writeback_port_deferred_rows(),
            deferred_row_cycles: stats.writeback_port_deferred_row_cycles(),
            max_ready_rows_per_cycle: stats.writeback_port_max_ready_rows_per_cycle(),
            max_deferred_rows: stats.writeback_port_max_deferred_rows(),
            partial_finalized_cycle_ticks: BTreeSet::new(),
            partial_finalized_ready_rows_by_tick: BTreeMap::new(),
            partial_finalized_deferred_rows_by_tick: BTreeMap::new(),
            closed_before_tick: 0,
        }
    }
}
