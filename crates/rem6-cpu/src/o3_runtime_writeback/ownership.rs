use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3FinalizedWritebackPortStats {
    pub(super) cycles: u64,
    pub(super) admitted_rows: u64,
    pub(super) deferred_rows: u64,
    pub(super) deferred_row_cycles: u64,
    pub(super) max_ready_rows_per_cycle: u64,
    pub(super) max_deferred_rows: u64,
    pub(super) partial_finalized_cycle_ticks: BTreeSet<u64>,
    pub(super) partial_finalized_ready_rows_by_tick: BTreeMap<u64, u64>,
    pub(super) partial_finalized_deferred_rows_by_tick: BTreeMap<u64, u64>,
    pub(super) closed_before_tick: u64,
}

impl O3FinalizedWritebackPortStats {
    pub(super) fn from_aggregate(stats: O3RuntimeStats) -> Self {
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

    pub(super) fn observe_finalized_schedule(
        &mut self,
        finalized: &O3WritebackPortStatsSchedule,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<(), O3RuntimeError> {
        self.admitted_rows =
            checked_stat_add(self.admitted_rows, finalized.admitted_rows, "admitted_rows")?;
        self.deferred_rows =
            checked_stat_add(self.deferred_rows, finalized.deferred_rows, "deferred_rows")?;
        self.deferred_row_cycles = checked_stat_add(
            self.deferred_row_cycles,
            finalized.deferred_row_cycles,
            "deferred_row_cycles",
        )?;
        self.partial_finalized_cycle_ticks
            .extend(finalized.cycle_ticks.iter().copied());
        for (tick, rows) in &finalized.ready_rows_by_tick {
            let rows = u64::try_from(rows.len()).map_err(|_| {
                O3RuntimeError::WritebackStatisticsOverflow {
                    counter: "max_ready_rows_per_cycle",
                }
            })?;
            let partial = self
                .partial_finalized_ready_rows_by_tick
                .entry(*tick)
                .or_default();
            *partial = checked_stat_add(*partial, rows, "max_ready_rows_per_cycle")?;
        }
        for (tick, rows) in &finalized.deferred_rows_by_tick {
            let partial = self
                .partial_finalized_deferred_rows_by_tick
                .entry(*tick)
                .or_default();
            *partial = checked_stat_add(*partial, *rows, "max_deferred_rows")?;
        }
        self.reconcile_live_schedule(live)?;
        Ok(())
    }

    pub(super) fn reconcile_live_schedule(
        &mut self,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<(), O3RuntimeError> {
        self.reconcile_live_schedule_before(live, self.closed_before_tick)
    }

    pub(super) fn close_before(
        &mut self,
        tick: u64,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<(), O3RuntimeError> {
        let closed_before_tick = self.closed_before_tick.max(tick);
        self.reconcile_live_schedule_before(live, closed_before_tick)?;
        self.closed_before_tick = closed_before_tick;
        Ok(())
    }

    pub(super) fn close_all_reopenable_ticks(
        &mut self,
        retained_last_tick: Option<u64>,
    ) -> Result<(), O3RuntimeError> {
        let retained_last_tick = retained_last_tick
            .into_iter()
            .chain(self.partial_finalized_cycle_ticks.last().copied())
            .chain(
                self.partial_finalized_ready_rows_by_tick
                    .last_key_value()
                    .map(|(tick, _)| *tick),
            )
            .chain(
                self.partial_finalized_deferred_rows_by_tick
                    .last_key_value()
                    .map(|(tick, _)| *tick),
            )
            .max();
        let Some(retained_last_tick) = retained_last_tick else {
            return Ok(());
        };
        let closed_before_tick = retained_last_tick.checked_add(1).ok_or(
            O3RuntimeError::WritebackClosureTickOverflow {
                tick: retained_last_tick,
            },
        )?;
        self.close_before(closed_before_tick, &O3WritebackPortStatsSchedule::default())
    }

    pub(super) fn reset_counters_preserving_closure(&mut self) {
        let closed_before_tick = self.closed_before_tick;
        *self = Self::default();
        self.closed_before_tick = closed_before_tick;
    }

    fn reconcile_live_schedule_before(
        &mut self,
        live: &O3WritebackPortStatsSchedule,
        closed_before_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        let closed_cycles = self
            .partial_finalized_cycle_ticks
            .iter()
            .filter(|tick| **tick < closed_before_tick && !live.cycle_ticks.contains(tick))
            .copied()
            .collect::<Vec<_>>();
        let added_cycles = u64::try_from(closed_cycles.len())
            .map_err(|_| O3RuntimeError::WritebackStatisticsOverflow { counter: "cycles" })?;
        let cycles = checked_stat_add(self.cycles, added_cycles, "cycles")?;
        self.cycles = cycles;
        for tick in closed_cycles {
            self.partial_finalized_cycle_ticks.remove(&tick);
        }

        let closed_ready_ticks = self
            .partial_finalized_ready_rows_by_tick
            .keys()
            .filter(|tick| {
                **tick < closed_before_tick && !live.ready_rows_by_tick.contains_key(*tick)
            })
            .copied()
            .collect::<Vec<_>>();
        for tick in closed_ready_ticks {
            if let Some(rows) = self.partial_finalized_ready_rows_by_tick.remove(&tick) {
                self.max_ready_rows_per_cycle = self.max_ready_rows_per_cycle.max(rows);
            }
        }

        let closed_deferred_ticks = self
            .partial_finalized_deferred_rows_by_tick
            .keys()
            .filter(|tick| {
                **tick < closed_before_tick && !live.deferred_rows_by_tick.contains_key(*tick)
            })
            .copied()
            .collect::<Vec<_>>();
        for tick in closed_deferred_ticks {
            if let Some(rows) = self.partial_finalized_deferred_rows_by_tick.remove(&tick) {
                self.max_deferred_rows = self.max_deferred_rows.max(rows);
            }
        }
        Ok(())
    }

    pub(super) fn cycles_with_live(
        &self,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<u64, O3RuntimeError> {
        let partial_cycles = u64::try_from(self.partial_finalized_cycle_ticks.len())
            .map_err(|_| O3RuntimeError::WritebackStatisticsOverflow { counter: "cycles" })?;
        let overlap_cycles = u64::try_from(
            self.partial_finalized_cycle_ticks
                .intersection(&live.cycle_ticks)
                .count(),
        )
        .map_err(|_| O3RuntimeError::WritebackStatisticsOverflow { counter: "cycles" })?;
        let combined = checked_stat_add(self.cycles, partial_cycles, "cycles")?;
        let combined = checked_stat_add(combined, live.cycles, "cycles")?;
        combined
            .checked_sub(overlap_cycles)
            .ok_or(O3RuntimeError::WritebackStatisticsUnderflow {
                counter: "cycles",
                current: combined,
                removed: overlap_cycles,
            })
    }

    pub(super) fn max_ready_rows_with_live(
        &self,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<u64, O3RuntimeError> {
        let mut maximum = self.max_ready_rows_per_cycle;
        for (tick, partial) in &self.partial_finalized_ready_rows_by_tick {
            let live_rows = live
                .ready_rows_by_tick
                .get(tick)
                .map(BTreeSet::len)
                .unwrap_or(0);
            let live_rows = u64::try_from(live_rows).map_err(|_| {
                O3RuntimeError::WritebackStatisticsOverflow {
                    counter: "max_ready_rows_per_cycle",
                }
            })?;
            maximum = maximum.max(checked_stat_add(
                *partial,
                live_rows,
                "max_ready_rows_per_cycle",
            )?);
        }
        for (tick, rows) in &live.ready_rows_by_tick {
            if self.partial_finalized_ready_rows_by_tick.contains_key(tick) {
                continue;
            }
            let rows = u64::try_from(rows.len()).map_err(|_| {
                O3RuntimeError::WritebackStatisticsOverflow {
                    counter: "max_ready_rows_per_cycle",
                }
            })?;
            maximum = maximum.max(rows);
        }
        Ok(maximum)
    }

    pub(super) fn max_deferred_rows_with_live(
        &self,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<u64, O3RuntimeError> {
        let mut maximum = self.max_deferred_rows;
        for (tick, partial) in &self.partial_finalized_deferred_rows_by_tick {
            let live_rows = live.deferred_rows_by_tick.get(tick).copied().unwrap_or(0);
            maximum = maximum.max(checked_stat_add(*partial, live_rows, "max_deferred_rows")?);
        }
        for (tick, rows) in &live.deferred_rows_by_tick {
            if self
                .partial_finalized_deferred_rows_by_tick
                .contains_key(tick)
            {
                continue;
            }
            maximum = maximum.max(*rows);
        }
        Ok(maximum)
    }
}

impl O3RuntimeState {
    pub(super) fn finalize_live_writeback_ownership(
        &mut self,
        finalized: &BTreeSet<u64>,
        published_sequence: Option<u64>,
    ) -> Result<(), O3RuntimeError> {
        let finalized = finalized
            .intersection(&self.live_writeback_counted_sequences)
            .copied()
            .collect::<BTreeSet<_>>();
        let finalized_schedule =
            O3WritebackPortStatsSchedule::from_calendar(&self.writeback_calendar, &finalized)?;
        let mut live_sequences = self.live_writeback_counted_sequences.clone();
        for sequence in &finalized {
            live_sequences.remove(sequence);
        }
        let replacement =
            O3WritebackPortStatsSchedule::from_calendar(&self.writeback_calendar, &live_sequences)?;
        let mut finalized_stats = self.finalized_writeback_port_stats.clone();
        finalized_stats.observe_finalized_schedule(&finalized_schedule, &replacement)?;
        let mut stats = self.stats;
        stats.set_writeback_port_schedule(&finalized_stats, &replacement)?;
        let mut published = self.published_writeback_sequences.clone();
        if let Some(sequence) = published_sequence {
            published.insert(sequence);
        }

        self.live_writeback_counted_sequences = live_sequences;
        self.finalized_writeback_port_stats = finalized_stats;
        self.stats = stats;
        self.published_writeback_sequences = published;
        self.rebuild_live_writeback_schedule_ownership(replacement);
        Ok(())
    }
}
