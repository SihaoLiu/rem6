use std::collections::{BTreeSet, VecDeque};

use crate::o3_pipeline::{
    O3PendingStateSnapshot, O3WritebackCompletion, O3WritebackTransferBuffer,
    O3WritebackTransferSnapshot,
};

use super::*;

pub(super) struct O3WritebackReplanTransaction {
    pending_state: O3PendingStateSnapshot,
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    live_speculative_executions: Vec<O3LiveSpeculativeExecution>,
    writeback_calendar: O3WritebackReservationCalendar,
    live_writeback_counted_sequences: BTreeSet<u64>,
    finalized_writeback_port_stats: O3FinalizedWritebackPortStats,
    stats: O3RuntimeStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct O3WritebackReservationPlan {
    calendar: O3WritebackReservationCalendar,
    writeback: O3WritebackTransferSnapshot,
}

impl O3WritebackReplanTransaction {
    pub(super) fn capture(runtime: &O3RuntimeState) -> Self {
        Self {
            pending_state: runtime.snapshot.pending_state.clone(),
            reorder_buffer: runtime.snapshot.reorder_buffer.clone(),
            live_speculative_executions: runtime.live_speculative_executions.clone(),
            writeback_calendar: runtime.writeback_calendar.clone(),
            live_writeback_counted_sequences: runtime.live_writeback_counted_sequences.clone(),
            finalized_writeback_port_stats: runtime.finalized_writeback_port_stats.clone(),
            stats: runtime.stats,
        }
    }

    pub(super) fn commit(self, runtime: &mut O3RuntimeState) {
        runtime.snapshot.pending_state = self.pending_state;
        runtime.snapshot.reorder_buffer = self.reorder_buffer;
        runtime.live_speculative_executions = self.live_speculative_executions;
        runtime.writeback_calendar = self.writeback_calendar;
        runtime.live_writeback_counted_sequences = self.live_writeback_counted_sequences;
        runtime.finalized_writeback_port_stats = self.finalized_writeback_port_stats;
        runtime.stats = self.stats;
    }

    pub(super) fn reserve_writeback_completions_in_place(
        &mut self,
        mut ready: Vec<O3LiveWritebackReady>,
        published_writeback_sequences: &BTreeSet<u64>,
        live_data_accesses: &[O3LiveDataAccess],
    ) -> Result<Vec<O3WritebackReservation>, O3RuntimeError> {
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
        for row in ready {
            if let Some(existing) = self.writeback_calendar.reservation(row.sequence()) {
                if existing.raw_ready_tick != row.raw_ready_tick() {
                    return Err(O3RuntimeError::WritebackReservationMismatch {
                        sequence: row.sequence(),
                        existing_raw_ready_tick: existing.raw_ready_tick,
                        requested_raw_ready_tick: row.raw_ready_tick(),
                    });
                }
                if existing.source != row.source() {
                    return Err(O3RuntimeError::WritebackReservationSourceMismatch {
                        sequence: row.sequence(),
                        existing_source: existing.source.name(),
                        requested_source: row.source().name(),
                    });
                }
            } else {
                if row.raw_ready_tick() < self.finalized_writeback_port_stats.closed_before_tick {
                    return Err(O3RuntimeError::WritebackReservationTickClosed {
                        sequence: row.sequence(),
                        raw_ready_tick: row.raw_ready_tick(),
                        closed_before_tick: self.finalized_writeback_port_stats.closed_before_tick,
                    });
                }
                new_rows.push(row);
            }
        }
        if new_rows.is_empty() {
            return Ok(self.reservations_for_sequences(&requested_sequences));
        }

        let writeback = self.pending_state.writeback().clone();
        let deferred = writeback.deferred().len();
        if deferred != 0 {
            return Err(O3RuntimeError::StableWritebackQueueNotEmpty { deferred });
        }
        self.live_writeback_counted_sequences
            .extend(new_rows.iter().map(|row| row.sequence()));
        let mut staged_calendar = self.writeback_calendar.clone();
        if let Some(raw_ready_tick) = new_rows.iter().map(|row| row.raw_ready_tick()).min() {
            let replanned = staged_calendar
                .by_tick
                .values()
                .flatten()
                .filter(|reservation| {
                    !published_writeback_sequences.contains(&reservation.sequence)
                })
                .filter(|reservation| reservation.raw_ready_tick >= raw_ready_tick)
                .copied()
                .collect::<Vec<_>>();
            for reservation in &replanned {
                staged_calendar.remove_sequence(reservation.sequence);
            }
            new_rows.extend(replanned.into_iter().map(O3LiveWritebackReady::replanned));
        }
        let mut pending_rows = new_rows;
        for row in &pending_rows {
            staged_calendar.remove_sequence(row.sequence());
        }

        let initial_plan =
            plan_writeback_reservations(staged_calendar.clone(), writeback.clone(), &pending_rows)?;
        let moved_producers =
            self.moved_reservation_sequences(&initial_plan.calendar, &pending_rows);
        let invalidated = self.speculative_descendants(&moved_producers);
        let plan = if invalidated.is_empty() {
            initial_plan
        } else {
            self.invalidate_speculative_descendants(&invalidated);
            self.live_writeback_counted_sequences
                .retain(|sequence| !invalidated.contains(sequence));
            pending_rows.retain(|row| !invalidated.contains(&row.sequence()));
            for sequence in &invalidated {
                staged_calendar.remove_sequence(*sequence);
            }
            plan_writeback_reservations(staged_calendar, writeback, &pending_rows)?
        };

        self.pending_state = rebuilt_pending_state(&self.pending_state, plan.writeback)?;
        self.writeback_calendar = plan.calendar;
        self.sync_writeback_reservation_owners(live_data_accesses)?;
        let replacement_schedule = O3WritebackPortStatsSchedule::from_calendar(
            &self.writeback_calendar,
            &self.live_writeback_counted_sequences,
        )?;
        self.finalized_writeback_port_stats
            .reconcile_live_schedule(&replacement_schedule)?;
        self.stats.set_writeback_port_schedule(
            &self.finalized_writeback_port_stats,
            &replacement_schedule,
        )?;
        Ok(self.reservations_for_sequences(&requested_sequences))
    }

    fn moved_reservation_sequences(
        &self,
        planned: &O3WritebackReservationCalendar,
        pending_rows: &[O3LiveWritebackReady],
    ) -> BTreeSet<u64> {
        pending_rows
            .iter()
            .filter_map(|row| {
                let previous = self.writeback_calendar.reservation(row.sequence())?;
                let replacement = planned.reservation(row.sequence())?;
                (previous.admitted_tick != replacement.admitted_tick).then_some(row.sequence())
            })
            .collect()
    }

    fn speculative_descendants(&self, producers: &BTreeSet<u64>) -> BTreeSet<u64> {
        let mut invalidated = BTreeSet::new();
        let mut frontier = producers.clone();
        loop {
            let added = self
                .live_speculative_executions
                .iter()
                .filter(|issued| !invalidated.contains(&issued.sequence))
                .filter(|issued| {
                    issued
                        .producer_sequences
                        .iter()
                        .any(|producer| frontier.contains(producer))
                })
                .map(|issued| issued.sequence)
                .collect::<BTreeSet<_>>();
            if added.is_empty() {
                break;
            }
            invalidated.extend(added.iter().copied());
            frontier = added;
        }
        invalidated
    }

    fn invalidate_speculative_descendants(&mut self, invalidated: &BTreeSet<u64>) {
        self.live_speculative_executions
            .retain(|issued| !invalidated.contains(&issued.sequence));
        for entry in &mut self.reorder_buffer {
            if entry.is_live_staged() && invalidated.contains(&entry.sequence()) {
                *entry = entry.with_ready(false).with_ready_tick(0);
            }
        }
    }

    fn reservations_for_sequences(&self, sequences: &[u64]) -> Vec<O3WritebackReservation> {
        let mut reservations = sequences
            .iter()
            .filter_map(|sequence| self.writeback_calendar.reservation(*sequence))
            .collect::<Vec<_>>();
        reservations.sort_by_key(|reservation| reservation.sequence);
        reservations
    }

    fn sync_writeback_reservation_owners(
        &mut self,
        live_data_accesses: &[O3LiveDataAccess],
    ) -> Result<(), O3RuntimeError> {
        let reservations = self
            .writeback_calendar
            .by_tick
            .values()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        for reservation in reservations {
            Self::validate_live_memory_result_writeback_owner(live_data_accesses, reservation)?;
            self.sync_live_fixed_fu_writeback_owner(reservation)?;
        }
        Ok(())
    }

    fn validate_live_memory_result_writeback_owner(
        live_data_accesses: &[O3LiveDataAccess],
        reservation: O3WritebackReservation,
    ) -> Result<(), O3RuntimeError> {
        let Some(live) = live_data_accesses
            .iter()
            .find(|live| live.sequence == reservation.sequence)
        else {
            return Ok(());
        };
        let supported_result = live
            .execution
            .execution()
            .memory_access()
            .is_some_and(|access| o3_memory_result_destination(access).is_some());
        if live.outcome != O3LiveDataAccessOutcome::Completed
            || live.memory_result.is_none()
            || !supported_result
        {
            return Ok(());
        }
        if reservation.source != O3LiveWritebackReadySource::MemoryResult {
            return Err(O3RuntimeError::WritebackOwnerSourceMismatch {
                sequence: reservation.sequence,
                owner: "live data access",
                reservation_source: reservation.source.name(),
            });
        }
        let owner_raw_ready_tick = live
            .response_tick
            .and_then(|response_tick| response_tick.checked_add(1))
            .ok_or(O3RuntimeError::WritebackOwnerMissingRawReadyTick {
                sequence: reservation.sequence,
                owner: "live data access",
            })?;
        if owner_raw_ready_tick != reservation.raw_ready_tick {
            return Err(O3RuntimeError::WritebackOwnerReservationMismatch {
                sequence: reservation.sequence,
                owner: "live data access",
                owner_raw_ready_tick,
                reservation_raw_ready_tick: reservation.raw_ready_tick,
            });
        }
        Ok(())
    }

    fn sync_live_fixed_fu_writeback_owner(
        &mut self,
        reservation: O3WritebackReservation,
    ) -> Result<(), O3RuntimeError> {
        let mut matched = false;
        for issued in self
            .live_speculative_executions
            .iter_mut()
            .filter(|issued| issued.sequence == reservation.sequence)
        {
            matched = true;
            if reservation.source != O3LiveWritebackReadySource::FixedFu {
                return Err(O3RuntimeError::WritebackOwnerSourceMismatch {
                    sequence: reservation.sequence,
                    owner: "fixed-FU speculative execution",
                    reservation_source: reservation.source.name(),
                });
            }
            if issued.raw_ready_tick != reservation.raw_ready_tick {
                return Err(O3RuntimeError::WritebackOwnerReservationMismatch {
                    sequence: reservation.sequence,
                    owner: "fixed-FU speculative execution",
                    owner_raw_ready_tick: issued.raw_ready_tick,
                    reservation_raw_ready_tick: reservation.raw_ready_tick,
                });
            }
            issued.admitted_writeback_tick = reservation.admitted_tick;
            issued.writeback_slot = Some(reservation.slot);
        }
        if !matched {
            return Ok(());
        }
        let rob = self
            .reorder_buffer
            .iter_mut()
            .find(|entry| entry.sequence() == reservation.sequence && entry.is_live_staged())
            .ok_or(O3RuntimeError::WritebackOwnerMissing {
                sequence: reservation.sequence,
                owner: "fixed-FU ROB",
            })?;
        if rob.is_ready() {
            rob.mark_ready_at(reservation.admitted_tick);
        }
        Ok(())
    }
}

fn plan_writeback_reservations(
    mut calendar: O3WritebackReservationCalendar,
    writeback: O3WritebackTransferSnapshot,
    pending_rows: &[O3LiveWritebackReady],
) -> Result<O3WritebackReservationPlan, O3RuntimeError> {
    let mut buffer = O3WritebackTransferBuffer::from_snapshot(writeback)
        .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
    let mut pending_rows = pending_rows.to_vec();
    pending_rows.sort_by_key(|row| (row.raw_ready_tick(), row.sequence()));
    let ready_by_sequence = pending_rows
        .iter()
        .map(|row| (row.sequence(), *row))
        .collect::<BTreeMap<_, _>>();
    let mut pending_rows = VecDeque::from(pending_rows);
    let mut base_tick = pending_rows
        .front()
        .expect("pending writeback rows are nonempty")
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
            newly_ready.push(O3WritebackCompletion::new(row.sequence()));
        }
        let cycle = buffer
            .plan_cycle_with_occupied_slots(calendar.occupied_slots(base_tick), newly_ready)
            .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
        for admission in cycle.admissions() {
            let sequence = admission.completion().sequence();
            let row = ready_by_sequence
                .get(&sequence)
                .copied()
                .expect("admitted writeback row has ready metadata");
            let admitted_tick = base_tick
                .checked_add(admission.cycle_offset())
                .ok_or(O3RuntimeError::WritebackTickOverflow { tick: base_tick })?;
            calendar.insert(O3WritebackReservation::new(
                sequence,
                row.raw_ready_tick(),
                admitted_tick,
                admission.slot(),
                row.source(),
                row.decision_counted(),
            ))?;
        }
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
    Ok(O3WritebackReservationPlan {
        calendar,
        writeback: buffer.snapshot(),
    })
}

fn rebuilt_pending_state(
    pending_state: &O3PendingStateSnapshot,
    writeback: O3WritebackTransferSnapshot,
) -> Result<O3PendingStateSnapshot, O3RuntimeError> {
    O3PendingStateSnapshot::new(
        pending_state.resolved_dependency_scopes().to_vec(),
        pending_state.ready().to_vec(),
        writeback,
    )
    .map_err(|error| O3RuntimeError::InvalidPendingState { error })
}
