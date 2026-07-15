use std::collections::{BTreeMap, VecDeque};

use crate::o3_pipeline::{
    O3PendingStateSnapshot, O3PipelineStage, O3WritebackCompletion, O3WritebackTransferBuffer,
    O3WritebackTransferPolicy, O3WritebackTransferSnapshot,
};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3WritebackReservation {
    sequence: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: usize,
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
        decision_counted: bool,
    ) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            admitted_tick,
            slot,
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

    pub(crate) fn remove_future_from_sequence(&mut self, sequence: u64, now: u64) {
        self.by_tick.retain(|_, reservations| {
            reservations.retain(|reservation| {
                reservation.sequence < sequence || reservation.admitted_tick <= now
            });
            !reservations.is_empty()
        });
    }

    pub(crate) fn clear(&mut self) {
        self.by_tick.clear();
    }

    pub(crate) fn prune_before(&mut self, tick: u64) {
        self.by_tick
            .retain(|admitted_tick, _| *admitted_tick >= tick);
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
}

impl O3LiveWritebackReady {
    pub(crate) const fn fixed_fu(sequence: u64, raw_ready_tick: u64) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            source: O3LiveWritebackReadySource::FixedFu,
        }
    }

    pub(crate) const fn scalar_load(sequence: u64, raw_ready_tick: u64) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            source: O3LiveWritebackReadySource::ScalarLoad,
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveWritebackReadySource {
    FixedFu,
    ScalarLoad,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3WritebackPortStatsDelta {
    cycles: u64,
    admitted_rows: u64,
    deferred_rows: u64,
    deferred_row_cycles: u64,
    max_ready_rows_per_cycle: u64,
    max_deferred_rows: u64,
}

impl O3RuntimeState {
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

    pub(crate) fn reserve_writeback_completions<I>(
        &mut self,
        ready: I,
    ) -> Result<Vec<O3WritebackReservation>, O3RuntimeError>
    where
        I: IntoIterator<Item = O3LiveWritebackReady>,
    {
        let mut ready = ready.into_iter().collect::<Vec<_>>();
        ready.sort_by_key(|row| (row.sequence(), row.raw_ready_tick()));
        for pair in ready.windows(2) {
            if pair[0].sequence() == pair[1].sequence() {
                return Err(O3RuntimeError::DuplicateWritebackReadySequence {
                    sequence: pair[0].sequence(),
                });
            }
        }

        let mut requested_sequences = ready.iter().map(|row| row.sequence()).collect::<Vec<_>>();
        requested_sequences.sort_unstable();

        let mut new_rows = Vec::new();
        for row in &ready {
            match row.source() {
                O3LiveWritebackReadySource::FixedFu | O3LiveWritebackReadySource::ScalarLoad => {}
            }
            if let Some(existing) = self.writeback_calendar.reservation(row.sequence()) {
                if existing.raw_ready_tick != row.raw_ready_tick() {
                    return Err(O3RuntimeError::WritebackReservationMismatch {
                        sequence: row.sequence(),
                        existing_raw_ready_tick: existing.raw_ready_tick,
                        requested_raw_ready_tick: row.raw_ready_tick(),
                    });
                }
            } else {
                new_rows.push(*row);
            }
        }
        if new_rows.is_empty() {
            return Ok(self.reservations_for_sequences(&requested_sequences));
        }

        let writeback = self.snapshot.pending_state().writeback().clone();
        let deferred = writeback.deferred().len();
        if deferred != 0 {
            return Err(O3RuntimeError::StableWritebackQueueNotEmpty { deferred });
        }

        let mut staged_calendar = self.writeback_calendar.clone();
        let mut staged_cycle_ticks = self.live_writeback_cycle_ticks.clone();
        let mut staged_ready_rows_by_tick = self.live_writeback_ready_rows_by_tick.clone();
        let mut stats_delta = O3WritebackPortStatsDelta::default();
        let mut buffer = O3WritebackTransferBuffer::from_snapshot(writeback)
            .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
        let mut pending_rows = new_rows;
        pending_rows.sort_by_key(|row| (row.raw_ready_tick(), row.sequence()));
        let mut pending_rows = VecDeque::from(pending_rows);
        let mut raw_ready_by_sequence = pending_rows
            .iter()
            .map(|row| (row.sequence(), row.raw_ready_tick()))
            .collect::<BTreeMap<_, _>>();
        let mut base_tick = pending_rows
            .front()
            .expect("new rows are nonempty")
            .raw_ready_tick();

        while !pending_rows.is_empty() || !buffer.is_empty() {
            let mut newly_ready = Vec::new();
            while pending_rows
                .front()
                .is_some_and(|row| row.raw_ready_tick() <= base_tick)
            {
                let row = pending_rows
                    .pop_front()
                    .expect("front row was just observed");
                staged_ready_rows_by_tick
                    .entry(row.raw_ready_tick())
                    .or_default()
                    .insert(row.sequence());
                newly_ready.push(O3WritebackCompletion::new(row.sequence()));
            }
            if staged_cycle_ticks.insert(base_tick) {
                stats_delta.cycles = stats_delta.cycles.saturating_add(1);
            }
            if let Some(rows) = staged_ready_rows_by_tick.get(&base_tick) {
                stats_delta.max_ready_rows_per_cycle = stats_delta
                    .max_ready_rows_per_cycle
                    .max(u64::try_from(rows.len()).unwrap_or(u64::MAX));
            }

            let cycle = buffer
                .plan_cycle_with_occupied_slots(
                    staged_calendar.occupied_slots(base_tick),
                    newly_ready,
                )
                .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
            for admission in cycle.admissions() {
                let sequence = admission.completion().sequence();
                let admitted_tick = base_tick
                    .checked_add(admission.cycle_offset())
                    .ok_or(O3RuntimeError::WritebackTickOverflow { tick: base_tick })?;
                let raw_ready_tick = *raw_ready_by_sequence
                    .get(&sequence)
                    .expect("admitted writeback row has a raw-ready tick");
                let reservation = O3WritebackReservation::new(
                    sequence,
                    raw_ready_tick,
                    admitted_tick,
                    admission.slot(),
                    true,
                );
                staged_calendar.insert(reservation)?;
                stats_delta.admitted_rows = stats_delta.admitted_rows.saturating_add(1);
                if admitted_tick > raw_ready_tick {
                    stats_delta.deferred_rows = stats_delta.deferred_rows.saturating_add(1);
                    stats_delta.deferred_row_cycles = stats_delta
                        .deferred_row_cycles
                        .saturating_add(admitted_tick - raw_ready_tick);
                }
                raw_ready_by_sequence.remove(&sequence);
            }
            stats_delta.max_deferred_rows = stats_delta
                .max_deferred_rows
                .max(u64::try_from(buffer.pending_deferred_count()).unwrap_or(u64::MAX));

            if buffer.is_empty() {
                let Some(next) = pending_rows.front() else {
                    break;
                };
                base_tick = next.raw_ready_tick();
            } else {
                base_tick = base_tick
                    .checked_add(1)
                    .ok_or(O3RuntimeError::WritebackTickOverflow { tick: base_tick })?;
            }
        }

        let drained_snapshot = buffer.snapshot();
        self.rebuild_pending_writeback_snapshot(drained_snapshot)?;
        self.writeback_calendar = staged_calendar;
        self.live_writeback_cycle_ticks = staged_cycle_ticks;
        self.live_writeback_ready_rows_by_tick = staged_ready_rows_by_tick;
        self.stats.record_writeback_port_delta(stats_delta);
        Ok(self.reservations_for_sequences(&requested_sequences))
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

    fn reservations_for_sequences(&self, sequences: &[u64]) -> Vec<O3WritebackReservation> {
        let mut reservations = sequences
            .iter()
            .filter_map(|sequence| self.writeback_calendar.reservation(*sequence))
            .collect::<Vec<_>>();
        reservations.sort_by_key(|reservation| reservation.sequence);
        reservations
    }

    fn rebuild_pending_writeback_snapshot(
        &mut self,
        writeback: O3WritebackTransferSnapshot,
    ) -> Result<(), O3RuntimeError> {
        let pending_state = self.snapshot.pending_state();
        let resolved_dependency_scopes = pending_state.resolved_dependency_scopes().to_vec();
        let ready = pending_state.ready().to_vec();
        self.snapshot.pending_state =
            O3PendingStateSnapshot::new(resolved_dependency_scopes, ready, writeback)
                .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
        Ok(())
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
        if self
            .writeback_calendar
            .reservation(sequence)
            .is_some_and(|reservation| reservation.admitted_tick() > now)
        {
            self.writeback_calendar.remove_sequence(sequence);
        }
    }

    pub(super) fn discard_future_writeback_from_sequence(&mut self, sequence: u64, now: u64) {
        self.writeback_calendar
            .remove_future_from_sequence(sequence, now);
    }

    pub(crate) fn discard_all_writeback_reservations(&mut self) {
        self.writeback_calendar.clear();
        self.live_writeback_cycle_ticks.clear();
        self.live_writeback_ready_rows_by_tick.clear();
    }

    pub(crate) fn prune_writeback_calendar_before(&mut self, tick: u64) {
        self.writeback_calendar.prune_before(tick);
    }

    pub(super) fn remove_live_writeback_sequence(&mut self, sequence: u64) {
        self.writeback_calendar.remove_sequence(sequence);
    }

    pub(super) fn clear_live_writeback_state(&mut self) {
        self.discard_all_writeback_reservations();
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
    fn record_writeback_port_delta(&mut self, delta: O3WritebackPortStatsDelta) {
        self.writeback_port_cycles = self.writeback_port_cycles.saturating_add(delta.cycles);
        self.writeback_port_admitted_rows = self
            .writeback_port_admitted_rows
            .saturating_add(delta.admitted_rows);
        self.writeback_port_deferred_rows = self
            .writeback_port_deferred_rows
            .saturating_add(delta.deferred_rows);
        self.writeback_port_deferred_row_cycles = self
            .writeback_port_deferred_row_cycles
            .saturating_add(delta.deferred_row_cycles);
        self.writeback_port_max_ready_rows_per_cycle = self
            .writeback_port_max_ready_rows_per_cycle
            .max(delta.max_ready_rows_per_cycle);
        self.writeback_port_max_deferred_rows = self
            .writeback_port_max_deferred_rows
            .max(delta.max_deferred_rows);
    }
}
