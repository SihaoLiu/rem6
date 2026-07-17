use std::collections::{BTreeMap, BTreeSet};

use crate::o3_pipeline::{
    O3PendingStateSnapshot, O3PipelineStage, O3WritebackTransferPolicy, O3WritebackTransferSnapshot,
};

use super::*;

#[path = "o3_runtime_writeback/replan.rs"]
mod replan;
use replan::O3WritebackReplanTransaction;
#[path = "o3_runtime_writeback/ownership.rs"]
mod ownership;
pub(super) use ownership::O3FinalizedWritebackPortStats;
#[cfg(test)]
#[path = "o3_runtime_writeback/ownership_debug_tests.rs"]
mod ownership_debug;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3WritebackReservation {
    sequence: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: usize,
    source: O3LiveWritebackReadySource,
    decision_counted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3RuntimeWritebackReservation {
    sequence: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: usize,
}

impl O3RuntimeWritebackReservation {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn raw_ready_tick(self) -> u64 {
        self.raw_ready_tick
    }

    pub const fn admitted_tick(self) -> u64 {
        self.admitted_tick
    }

    pub const fn slot(self) -> usize {
        self.slot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3WritebackDebugState {
    width: usize,
    reserved_future_completions: usize,
    earliest_unpublished_tick: Option<u64>,
}

impl RiscvO3WritebackDebugState {
    const fn new(
        width: usize,
        reserved_future_completions: usize,
        earliest_unpublished_tick: Option<u64>,
    ) -> Self {
        Self {
            width,
            reserved_future_completions,
            earliest_unpublished_tick,
        }
    }

    pub const fn width(self) -> usize {
        self.width
    }

    pub const fn reserved_future_completions(self) -> usize {
        self.reserved_future_completions
    }

    pub const fn earliest_unpublished_tick(self) -> Option<u64> {
        self.earliest_unpublished_tick
    }
}

impl From<O3WritebackReservation> for O3RuntimeWritebackReservation {
    fn from(reservation: O3WritebackReservation) -> Self {
        Self {
            sequence: reservation.sequence,
            raw_ready_tick: reservation.raw_ready_tick,
            admitted_tick: reservation.admitted_tick,
            slot: reservation.slot,
        }
    }
}

impl O3WritebackReservation {
    const fn new(
        sequence: u64,
        raw_ready_tick: u64,
        admitted_tick: u64,
        slot: usize,
        source: O3LiveWritebackReadySource,
        decision_counted: bool,
    ) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            admitted_tick,
            slot,
            source,
            decision_counted,
        }
    }

    #[cfg(test)]
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    #[cfg(test)]
    pub(crate) const fn raw_ready_tick(self) -> u64 {
        self.raw_ready_tick
    }

    pub(crate) const fn admitted_tick(self) -> u64 {
        self.admitted_tick
    }

    pub(crate) const fn slot(self) -> usize {
        self.slot
    }

    #[cfg(test)]
    pub(crate) const fn decision_counted(self) -> bool {
        self.decision_counted
    }

    #[cfg(test)]
    pub(crate) const fn source_name(self) -> &'static str {
        self.source.name()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct O3WritebackReservationCalendar {
    pub(crate) by_tick: BTreeMap<u64, Vec<O3WritebackReservation>>,
}

impl O3WritebackReservationCalendar {
    pub(crate) fn reservation(&self, sequence: u64) -> Option<O3WritebackReservation> {
        self.by_tick
            .values()
            .flatten()
            .find(|reservation| reservation.sequence == sequence)
            .copied()
    }

    #[cfg(test)]
    pub(crate) fn reservations(&self) -> Vec<O3WritebackReservation> {
        let mut reservations = self.by_tick.values().flatten().copied().collect::<Vec<_>>();
        reservations.sort_by_key(|reservation| reservation.sequence);
        reservations
    }

    pub(crate) fn occupied_slots(&self, tick: u64) -> Vec<usize> {
        let mut slots = self
            .by_tick
            .get(&tick)
            .into_iter()
            .flatten()
            .map(|reservation| reservation.slot)
            .collect::<Vec<_>>();
        slots.sort_unstable();
        slots
    }

    pub(crate) fn snapshot(&self) -> Vec<O3RuntimeWritebackReservation> {
        self.by_tick
            .values()
            .flatten()
            .copied()
            .map(O3RuntimeWritebackReservation::from)
            .collect()
    }

    pub(crate) fn insert(
        &mut self,
        reservation: O3WritebackReservation,
    ) -> Result<(), O3RuntimeError> {
        if self.reservation(reservation.sequence).is_some() {
            return Err(O3RuntimeError::DuplicateWritebackReadySequence {
                sequence: reservation.sequence,
            });
        }
        let tick_rows = self.by_tick.entry(reservation.admitted_tick).or_default();
        if tick_rows
            .iter()
            .any(|existing| existing.slot == reservation.slot)
        {
            return Err(O3RuntimeError::WritebackCalendarSlotOccupied {
                tick: reservation.admitted_tick,
                slot: reservation.slot,
            });
        }
        tick_rows.push(reservation);
        tick_rows.sort_by_key(|reservation| reservation.slot);
        Ok(())
    }

    pub(crate) fn remove_sequence(&mut self, sequence: u64) -> Option<O3WritebackReservation> {
        let mut removed = None;
        self.by_tick.retain(|_, reservations| {
            if let Some(index) = reservations
                .iter()
                .position(|reservation| reservation.sequence == sequence)
            {
                removed = Some(reservations.remove(index));
            }
            !reservations.is_empty()
        });
        removed
    }

    pub(crate) fn clear(&mut self) {
        self.by_tick.clear();
    }

    pub(crate) fn reserved_future_count(&self, now: u64) -> usize {
        self.by_tick
            .iter()
            .filter(|(tick, _)| **tick > now)
            .map(|(_, reservations)| reservations.len())
            .sum()
    }

    pub(crate) fn earliest_unpublished_tick(&self, now: u64) -> Option<u64> {
        self.by_tick.keys().copied().find(|tick| *tick > now)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_tick.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveWritebackReady {
    sequence: u64,
    raw_ready_tick: u64,
    source: O3LiveWritebackReadySource,
    decision_counted: bool,
}

impl O3LiveWritebackReady {
    pub(crate) const fn fixed_fu(sequence: u64, raw_ready_tick: u64) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            source: O3LiveWritebackReadySource::FixedFu,
            decision_counted: true,
        }
    }

    pub(crate) const fn memory_result(sequence: u64, raw_ready_tick: u64) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            source: O3LiveWritebackReadySource::MemoryResult,
            decision_counted: true,
        }
    }

    const fn replanned(reservation: O3WritebackReservation) -> Self {
        Self {
            sequence: reservation.sequence,
            raw_ready_tick: reservation.raw_ready_tick,
            source: reservation.source,
            decision_counted: reservation.decision_counted,
        }
    }

    const fn sequence(self) -> u64 {
        self.sequence
    }

    const fn raw_ready_tick(self) -> u64 {
        self.raw_ready_tick
    }

    const fn source(self) -> O3LiveWritebackReadySource {
        self.source
    }

    const fn decision_counted(self) -> bool {
        self.decision_counted
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveWritebackReadySource {
    FixedFu,
    MemoryResult,
}

impl O3LiveWritebackReadySource {
    const fn name(self) -> &'static str {
        match self {
            Self::FixedFu => "FixedFu",
            Self::MemoryResult => "MemoryResult",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3WritebackPortStatsSchedule {
    cycles: u64,
    admitted_rows: u64,
    deferred_rows: u64,
    deferred_row_cycles: u64,
    max_ready_rows_per_cycle: u64,
    max_deferred_rows: u64,
    cycle_ticks: BTreeSet<u64>,
    ready_rows_by_tick: BTreeMap<u64, BTreeSet<u64>>,
    deferred_rows_by_tick: BTreeMap<u64, u64>,
}

impl O3WritebackPortStatsSchedule {
    fn from_calendar(
        calendar: &O3WritebackReservationCalendar,
        counted_sequences: &BTreeSet<u64>,
    ) -> Result<Self, O3RuntimeError> {
        let reservations = calendar
            .by_tick
            .values()
            .flatten()
            .copied()
            .filter(|reservation| {
                reservation.decision_counted && counted_sequences.contains(&reservation.sequence)
            })
            .collect::<Vec<_>>();
        let mut schedule = Self {
            admitted_rows: u64::try_from(reservations.len()).map_err(|_| {
                O3RuntimeError::WritebackStatisticsOverflow {
                    counter: "admitted_rows",
                }
            })?,
            ..Self::default()
        };
        for reservation in reservations {
            schedule
                .ready_rows_by_tick
                .entry(reservation.raw_ready_tick)
                .or_default()
                .insert(reservation.sequence);
            let mut tick = reservation.raw_ready_tick;
            loop {
                schedule.cycle_ticks.insert(tick);
                if tick == reservation.admitted_tick {
                    break;
                }
                let rows = schedule.deferred_rows_by_tick.entry(tick).or_default();
                *rows = checked_stat_add(*rows, 1, "max_deferred_rows")?;
                tick = tick
                    .checked_add(1)
                    .ok_or(O3RuntimeError::WritebackStatisticsOverflow { counter: "cycles" })?;
            }
            if reservation.admitted_tick > reservation.raw_ready_tick {
                schedule.deferred_rows =
                    checked_stat_add(schedule.deferred_rows, 1, "deferred_rows")?;
                schedule.deferred_row_cycles = checked_stat_add(
                    schedule.deferred_row_cycles,
                    reservation.admitted_tick - reservation.raw_ready_tick,
                    "deferred_row_cycles",
                )?;
            }
        }
        schedule.cycles = u64::try_from(schedule.cycle_ticks.len())
            .map_err(|_| O3RuntimeError::WritebackStatisticsOverflow { counter: "cycles" })?;
        for rows in schedule.ready_rows_by_tick.values() {
            let rows = u64::try_from(rows.len()).map_err(|_| {
                O3RuntimeError::WritebackStatisticsOverflow {
                    counter: "max_ready_rows_per_cycle",
                }
            })?;
            schedule.max_ready_rows_per_cycle = schedule.max_ready_rows_per_cycle.max(rows);
        }
        schedule.max_deferred_rows = schedule
            .deferred_rows_by_tick
            .values()
            .copied()
            .max()
            .unwrap_or(0);
        Ok(schedule)
    }
}

impl O3RuntimeState {
    pub(crate) fn failure_diagnostic_writeback_reservation_count(&self) -> usize {
        self.writeback_calendar.by_tick.values().map(Vec::len).sum()
    }

    pub(crate) fn writeback_debug_state(&self, now: u64) -> RiscvO3WritebackDebugState {
        RiscvO3WritebackDebugState::new(
            self.snapshot
                .pending_state()
                .writeback()
                .policy()
                .writeback_width(),
            self.writeback_calendar.reserved_future_count(now),
            self.writeback_calendar.earliest_unpublished_tick(now),
        )
    }

    #[cfg(test)]
    pub(crate) fn writeback_reservation(&self, sequence: u64) -> Option<O3WritebackReservation> {
        self.writeback_calendar.reservation(sequence)
    }

    #[cfg(test)]
    pub(crate) fn writeback_reservations(&self) -> Vec<O3WritebackReservation> {
        self.writeback_calendar.reservations()
    }

    #[cfg(test)]
    pub(crate) fn force_test_writeback_reservation_to_memory_result(&mut self, sequence: u64) {
        for reservation in self.writeback_calendar.by_tick.values_mut().flatten() {
            if reservation.sequence == sequence {
                reservation.source = O3LiveWritebackReadySource::MemoryResult;
            }
        }
    }

    pub(crate) fn reserve_writeback_completions<I>(
        &mut self,
        ready: I,
    ) -> Result<Vec<O3WritebackReservation>, O3RuntimeError>
    where
        I: IntoIterator<Item = O3LiveWritebackReady>,
    {
        let ready = ready.into_iter().collect::<Vec<_>>();
        let mut transaction = O3WritebackReplanTransaction::capture(self);
        let reservations = transaction.reserve_writeback_completions_in_place(ready)?;
        transaction.commit(self);
        Ok(reservations)
    }

    pub(crate) fn reserve_fixed_fu_writeback(
        &mut self,
        sequence: u64,
        raw_ready_tick: u64,
        consumes_slot: bool,
    ) -> Result<(u64, Option<usize>), O3RuntimeError> {
        if !consumes_slot {
            return Ok((raw_ready_tick, None));
        }
        let reservation = self
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])?
            .into_iter()
            .next()
            .expect("single fixed-FU writeback reservation returns one row");
        Ok((reservation.admitted_tick(), Some(reservation.slot())))
    }

    pub(super) fn rebuild_writeback_policy(
        &mut self,
        writeback_width: usize,
    ) -> Result<(), O3RuntimeError> {
        let pending_state = self.snapshot.pending_state();
        let resolved_dependency_scopes = pending_state.resolved_dependency_scopes().to_vec();
        let ready = pending_state.ready().to_vec();
        let deferred = pending_state.writeback().deferred().to_vec();
        self.snapshot.pending_state = O3PendingStateSnapshot::new(
            resolved_dependency_scopes,
            ready,
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, writeback_width, 0)
                    .expect("validated RISC-V O3 writeback policy is valid"),
                deferred,
            ),
        )
        .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
        Ok(())
    }

    pub(super) fn discard_future_writeback_sequence(&mut self, sequence: u64, now: u64) {
        let discarded = self
            .writeback_calendar
            .reservation(sequence)
            .filter(|reservation| reservation.admitted_tick() > now)
            .map(|reservation| reservation.sequence)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if !discarded.is_empty() {
            self.discard_writeback_reservations(&discarded)
                .expect("discarded future writeback sequence has coherent live statistics");
        }
    }

    pub(super) fn discard_future_writeback_from_sequence(&mut self, sequence: u64, now: u64) {
        let discarded = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .filter(|reservation| {
                reservation.sequence >= sequence && reservation.admitted_tick() > now
            })
            .map(|reservation| reservation.sequence)
            .collect::<BTreeSet<_>>();
        if !discarded.is_empty() {
            self.discard_writeback_reservations(&discarded)
                .expect("discarded future writeback suffix has coherent live statistics");
        }
    }

    pub(crate) fn discard_live_writeback_reservations(&mut self) {
        let discarded = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .filter(|reservation| {
                !self
                    .published_writeback_sequences
                    .contains(&reservation.sequence)
            })
            .map(|reservation| reservation.sequence)
            .collect::<BTreeSet<_>>();
        self.discard_writeback_reservations(&discarded)
            .expect("discarded live writeback calendar has coherent statistics");
    }

    pub(super) fn discard_live_writeback_from_sequence(&mut self, sequence: u64) {
        let discarded = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .filter(|reservation| {
                reservation.sequence >= sequence
                    && !self
                        .published_writeback_sequences
                        .contains(&reservation.sequence)
            })
            .map(|reservation| reservation.sequence)
            .collect::<BTreeSet<_>>();
        self.discard_writeback_reservations(&discarded)
            .expect("discarded live writeback suffix has coherent statistics");
    }

    pub(crate) fn prune_writeback_calendar_before(&mut self, tick: u64) {
        self.finalize_writeback_reservations_before(tick)
            .expect("pruned writeback calendar has coherent live statistics");
    }

    pub(super) fn finalize_writeback_publication(&mut self, sequence: u64) {
        if self.writeback_calendar.reservation(sequence).is_none() {
            return;
        }
        let finalized = BTreeSet::from([sequence]);
        self.finalize_live_writeback_ownership(&finalized, Some(sequence))
            .expect("published writeback reservation has coherent live statistics");
    }

    pub(crate) fn finalize_all_writeback_reservations(&mut self) -> Result<(), O3RuntimeError> {
        let finalized = self.live_writeback_counted_sequences.clone();
        let schedule =
            O3WritebackPortStatsSchedule::from_calendar(&self.writeback_calendar, &finalized)?;
        let empty = O3WritebackPortStatsSchedule::default();
        let mut finalized_stats = self.finalized_writeback_port_stats.clone();
        finalized_stats.observe_finalized_schedule(&schedule, &empty)?;
        let retained_last_tick = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .flat_map(|reservation| [reservation.raw_ready_tick, reservation.admitted_tick])
            .max();
        finalized_stats.close_all_reopenable_ticks(retained_last_tick)?;
        let mut stats = self.stats;
        stats.set_writeback_port_schedule(&finalized_stats, &empty)?;

        self.live_writeback_counted_sequences.clear();
        self.finalized_writeback_port_stats = finalized_stats;
        self.stats = stats;
        self.rebuild_live_writeback_schedule_ownership(empty);
        self.writeback_calendar.clear();
        self.published_writeback_sequences.clear();
        Ok(())
    }

    pub(super) fn clear_all_writeback_state(&mut self) {
        self.writeback_calendar.clear();
        self.published_writeback_sequences.clear();
        self.live_writeback_counted_sequences.clear();
        self.live_writeback_cycle_ticks.clear();
        self.live_writeback_ready_rows_by_tick.clear();
        self.finalized_writeback_port_stats = O3FinalizedWritebackPortStats::default();
    }

    pub(crate) fn reset_all_writeback_state_preserving_stats(&mut self) {
        self.clear_all_writeback_state();
        self.seed_finalized_writeback_stats_from_aggregate();
    }

    pub(super) fn reset_writeback_stats_ownership(&mut self) {
        self.live_writeback_counted_sequences.clear();
        self.live_writeback_cycle_ticks.clear();
        self.live_writeback_ready_rows_by_tick.clear();
        self.finalized_writeback_port_stats
            .reset_counters_preserving_closure();
    }

    pub(super) fn seed_finalized_writeback_stats_from_aggregate(&mut self) {
        self.finalized_writeback_port_stats =
            O3FinalizedWritebackPortStats::from_aggregate(self.stats);
    }

    fn finalize_writeback_reservations_before(&mut self, tick: u64) -> Result<(), O3RuntimeError> {
        self.finalize_live_writeback_ownership_before(tick)?;
        let live = self.live_writeback_schedule()?;
        self.finalized_writeback_port_stats
            .close_before(tick, &live)?;
        self.stats
            .set_writeback_port_schedule(&self.finalized_writeback_port_stats, &live)?;
        let finalized = self
            .writeback_calendar
            .by_tick
            .range(..tick)
            .flat_map(|(_, reservations)| reservations)
            .map(|reservation| reservation.sequence)
            .collect::<Vec<_>>();
        for sequence in finalized {
            self.writeback_calendar.remove_sequence(sequence);
            self.published_writeback_sequences.remove(&sequence);
        }
        Ok(())
    }

    fn finalize_live_writeback_ownership_before(
        &mut self,
        tick: u64,
    ) -> Result<(), O3RuntimeError> {
        let finalized = self
            .writeback_calendar
            .by_tick
            .range(..tick)
            .flat_map(|(_, reservations)| reservations)
            .map(|reservation| reservation.sequence)
            .filter(|sequence| self.live_writeback_counted_sequences.contains(sequence))
            .collect::<BTreeSet<_>>();
        self.finalize_live_writeback_ownership(&finalized, None)
    }

    fn discard_writeback_reservations(
        &mut self,
        discarded: &BTreeSet<u64>,
    ) -> Result<(), O3RuntimeError> {
        for sequence in discarded {
            self.writeback_calendar.remove_sequence(*sequence);
            self.published_writeback_sequences.remove(sequence);
            self.live_writeback_counted_sequences.remove(sequence);
        }
        let live_sequences = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .map(|reservation| reservation.sequence)
            .collect::<BTreeSet<_>>();
        self.live_writeback_counted_sequences
            .retain(|sequence| live_sequences.contains(sequence));
        let replacement = self.live_writeback_schedule()?;
        self.finalized_writeback_port_stats
            .reconcile_live_schedule(&replacement)?;
        self.stats
            .set_writeback_port_schedule(&self.finalized_writeback_port_stats, &replacement)?;
        self.rebuild_live_writeback_schedule_ownership(replacement);
        Ok(())
    }

    fn live_writeback_schedule(&self) -> Result<O3WritebackPortStatsSchedule, O3RuntimeError> {
        O3WritebackPortStatsSchedule::from_calendar(
            &self.writeback_calendar,
            &self.live_writeback_counted_sequences,
        )
    }

    fn rebuild_live_writeback_schedule_ownership(
        &mut self,
        schedule: O3WritebackPortStatsSchedule,
    ) {
        self.live_writeback_cycle_ticks = schedule.cycle_ticks;
        self.live_writeback_ready_rows_by_tick = schedule.ready_rows_by_tick;
    }
}

impl crate::RiscvCore {
    pub fn o3_runtime_writeback_reservations(&self) -> Vec<O3RuntimeWritebackReservation> {
        self.with_o3_runtime(|runtime| runtime.writeback_calendar.snapshot())
    }
}

#[cfg(test)]
impl crate::RiscvCore {
    pub(crate) fn reserve_test_fixed_fu_writeback(
        &self,
        sequence: u64,
        raw_ready_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])
            .map(|_| ())
    }
}

impl O3RuntimeStats {
    fn set_writeback_port_schedule(
        &mut self,
        finalized: &O3FinalizedWritebackPortStats,
        live: &O3WritebackPortStatsSchedule,
    ) -> Result<(), O3RuntimeError> {
        self.writeback_port_cycles = finalized.cycles_with_live(live)?;
        self.writeback_port_admitted_rows =
            checked_stat_add(finalized.admitted_rows, live.admitted_rows, "admitted_rows")?;
        self.writeback_port_deferred_rows =
            checked_stat_add(finalized.deferred_rows, live.deferred_rows, "deferred_rows")?;
        self.writeback_port_deferred_row_cycles = checked_stat_add(
            finalized.deferred_row_cycles,
            live.deferred_row_cycles,
            "deferred_row_cycles",
        )?;
        self.writeback_port_max_ready_rows_per_cycle = finalized.max_ready_rows_with_live(live)?;
        self.writeback_port_max_deferred_rows = finalized.max_deferred_rows_with_live(live)?;
        Ok(())
    }
}

fn checked_stat_add(
    current: u64,
    added: u64,
    counter: &'static str,
) -> Result<u64, O3RuntimeError> {
    current
        .checked_add(added)
        .ok_or(O3RuntimeError::WritebackStatisticsOverflow { counter })
}
