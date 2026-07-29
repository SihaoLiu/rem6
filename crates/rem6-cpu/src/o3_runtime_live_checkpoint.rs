use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, RiscvFloatRoundingMode, RiscvInstruction};
use rem6_memory::{AddressRange, MemoryRequestId};

use super::o3_runtime_issue::queue::{O3LiveIssueQueue, O3LiveIssueQueueCapture};
#[cfg(test)]
use super::o3_runtime_issue::{calendar::O3LiveIssueCalendar, O3LiveIssueDependencyTable};
#[cfg(test)]
use super::o3_runtime_writeback::O3LiveWritebackReady;
use super::o3_runtime_writeback::O3WritebackReservation;
use super::*;
use crate::{
    riscv_data_completion::RiscvDataCompletion,
    riscv_fetch_ahead::O3MemoryResultWindowAuthorization, RiscvCpuExecutionEvent,
    RiscvO3LiveCheckpointCompletedFpLoad, RiscvO3LiveCheckpointError as Error,
    RiscvO3LiveCheckpointFinalizedWriteback, RiscvO3LiveCheckpointIssueRow,
    RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointPendingDataAddress,
    RiscvO3LiveCheckpointProfile, RiscvO3LiveCheckpointReservation, RiscvO3LiveCheckpointService,
    RiscvO3LiveCheckpointTelemetry, RiscvO3LiveCheckpointWritebackSource,
};

#[path = "o3_runtime_live_checkpoint/pending_address.rs"]
mod pending_address;
#[path = "o3_runtime_live_checkpoint/support.rs"]
mod support;
use support::{
    checkpoint_telemetry, restore_telemetry, validate_unique_requests, validate_unique_values,
};

pub(crate) struct O3LiveCheckpointRuntimeProjection {
    pub(crate) stable: O3RuntimeCheckpointPayload,
    pub(crate) profile: RiscvO3LiveCheckpointProfile,
    pub(crate) issue_rows: Vec<RiscvO3LiveCheckpointIssueRow>,
    pub(crate) rename_rows: Vec<O3RenameMapEntry>,
    pub(crate) resident_sequences: Vec<u64>,
    pub(crate) service: RiscvO3LiveCheckpointService,
    pub(crate) finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback,
    pub(crate) writeback_counted_sequences: Vec<u64>,
    pub(crate) writeback_published_sequences: Vec<u64>,
    pub(crate) reservation: Option<RiscvO3LiveCheckpointReservation>,
    pub(crate) completed_result: Option<RiscvO3LiveCheckpointCompletedFpLoad>,
    pub(crate) pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>,
    pub(crate) finalized_rows: Vec<RiscvO3LiveCheckpointIssueRow>,
}

struct O3CompletedFpCheckpointProjection {
    result: RiscvO3LiveCheckpointCompletedFpLoad,
    reservation: O3WritebackReservation,
    finalized_rows: Vec<RiscvO3LiveCheckpointIssueRow>,
    normalized_finalized_sequences: BTreeSet<u64>,
}

pub(crate) struct PreparedRiscvO3LiveRestore {
    runtime: O3RuntimeState,
    events: Vec<RiscvCpuExecutionEvent>,
    memory_result_authorization: Option<(MemoryRequestId, O3MemoryResultWindowAuthorization)>,
}

impl PreparedRiscvO3LiveRestore {
    pub(crate) fn into_parts(
        self,
    ) -> (
        O3RuntimeState,
        Vec<RiscvCpuExecutionEvent>,
        Option<(MemoryRequestId, O3MemoryResultWindowAuthorization)>,
    ) {
        (self.runtime, self.events, self.memory_result_authorization)
    }
}

impl O3RuntimeState {
    #[cfg(test)]
    pub(crate) fn stage_store_forwarding_overlay_for_checkpoint_test(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) {
        let _ = self.record_store_forwarding_window(execution, None, None);
    }

    #[cfg(test)]
    pub(crate) fn clear_live_checkpoint_lsq_for_test(&mut self) {
        self.snapshot.load_store_queue.clear();
    }

    #[cfg(test)]
    pub(crate) fn rearm_restored_completed_fp_result_for_terminal_injection(
        &mut self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
    ) -> bool {
        let [live] = self.live_data_accesses.as_mut_slice() else {
            return false;
        };
        if live.fetch_request != fetch_request
            || live.data_request != data_request
            || live.outcome != O3LiveDataAccessOutcome::Completed
            || live.event_taken
        {
            return false;
        }
        live.outcome = O3LiveDataAccessOutcome::Resident;
        live.memory_result = None;
        true
    }

    #[cfg(test)]
    pub(crate) const fn completed_fld_peer_collision_for_test(
        result_raw: u64,
        result_admitted: u64,
        result_slot: usize,
        peer_raw: u64,
        peer_admitted: u64,
        peer_slot: usize,
    ) -> bool {
        completed_fld_peer_collision(
            result_raw,
            result_admitted,
            result_slot,
            peer_raw,
            peer_admitted,
            peer_slot,
        )
    }

    pub(crate) fn checkpoint_live_instruction_matches(
        &self,
        sequence: u64,
        instruction: RiscvInstruction,
    ) -> bool {
        self.live_staged_instruction_matches(sequence, instruction)
    }

    #[cfg(test)]
    pub(crate) fn live_issue_resident_sequences_for_checkpoint(&self) -> Vec<u64> {
        self.live_issue.resident_sequences().to_vec()
    }

    #[cfg(test)]
    pub(crate) fn live_issue_packet_requests_for_checkpoint_test(
        &self,
        sequence: u64,
    ) -> Vec<MemoryRequestId> {
        self.live_staged_issue_packet(sequence)
            .map(|packet| packet.consumed_requests().to_vec())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn live_issue_queue_materializes_for_checkpoint(&self) -> bool {
        matches!(
            O3LiveIssueQueue::materialize(self, self.live_issue.resident_sequences()),
            Ok(O3LiveIssueQueueCapture::Ready(_))
        )
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_issue_plan_at_for_test(
        &self,
        tick: u64,
    ) -> Result<(Vec<u64>, Vec<u64>), O3RuntimeError> {
        let queue = match O3LiveIssueQueue::materialize(self, self.live_issue.resident_sequences())?
        {
            O3LiveIssueQueueCapture::Ready(queue) => queue,
            O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
            }
        };
        let dependencies = O3LiveIssueDependencyTable::new(self, queue.entries())?;
        let plan = O3LiveIssueCalendar::capture(self).plan_scoped_at(
            tick,
            dependencies.resolved_scopes_at(tick),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )?;
        Ok((
            plan.issued().iter().map(|row| row.sequence()).collect(),
            plan.dependency_blocked()
                .iter()
                .map(|row| row.sequence())
                .collect(),
        ))
    }

    pub(crate) fn compute_checkpoint_projection(
        &self,
        captured_tick: u64,
    ) -> Result<Option<O3LiveCheckpointRuntimeProjection>, Error> {
        let resident_sequences = self.live_issue.resident_sequences().to_vec();
        let pending_addresses = pending_address::capture(self, captured_tick, &resident_sequences)?;
        let pending_profile = !pending_addresses.is_empty();
        let completed_fp = self.checkpoint_completed_fp_load(captured_tick, &resident_sequences)?;
        let completed_profile = completed_fp.is_some();
        if let Some(completed) = &completed_fp {
            match O3LiveIssueQueue::materialize(self, &resident_sequences)
                .map_err(|_| invalid("completed FP dependent queue does not materialize"))?
            {
                O3LiveIssueQueueCapture::Ready(queue)
                    if exact_completed_fp_dependent(
                        &queue,
                        completed.result.sequence,
                        completed.result.destination,
                        completed.result.width,
                    ) => {}
                _ => return Err(invalid("completed FP dependent queue is unsupported")),
            }
        }
        let resident_set = resident_sequences.iter().copied().collect::<BTreeSet<_>>();
        let normalized_data_provenance = if completed_profile {
            let finalized_set = completed_fp
                .as_ref()
                .expect("completed profile has finalized rows")
                .finalized_rows
                .iter()
                .map(|row| row.sequence);
            self.live_data_access_younger_sequences
                == resident_set.iter().copied().chain(finalized_set).collect()
        } else if pending_profile {
            self.live_data_access_younger_sequences == resident_set
        } else {
            self.live_data_access_younger_sequences.is_empty()
                || self.live_data_access_younger_sequences == resident_set
        };
        if self.live_issue.transaction_active()
            || !self.pending_data_accesses.is_empty()
            || self.store_forwarding_window != Default::default()
            || !self.live_retired_instructions.is_empty()
            || (!completed_profile && !self.live_speculative_executions.is_empty())
            || !self.live_control_lineages.is_empty()
            || !self.live_serializing_control_sequences.is_empty()
            || !self.invalidated_live_staged_fetch_identities.is_empty()
            || !self.committed_live_staged_fetch_identities.is_empty()
            || self.deferred_live_data_access_execution.is_some()
            || (self.has_pending_data_address() && !pending_profile)
            || !normalized_data_provenance
            || (!completed_profile && !self.live_data_accesses.is_empty())
        {
            return Err(invalid("unsupported transient O3 authority"));
        }
        let mut finalized_writeback = self.clone();
        if let Some(completed) = &completed_fp {
            for row in &completed.finalized_rows {
                if completed
                    .normalized_finalized_sequences
                    .contains(&row.sequence)
                {
                    continue;
                }
                let admitted_tick = finalized_writeback
                    .writeback_calendar
                    .reservation(row.sequence)
                    .ok_or(invalid("finalized FP peer has no writeback reservation"))?
                    .admitted_tick();
                finalized_writeback
                    .snapshot
                    .reorder_buffer
                    .iter_mut()
                    .find(|owner| owner.sequence() == row.sequence && owner.is_live_staged())
                    .ok_or(invalid("finalized FP peer has no ROB owner"))?
                    .mark_ready_at(admitted_tick);
                finalized_writeback.finalize_writeback_publication(row.sequence);
                if finalized_writeback
                    .writeback_calendar
                    .remove_sequence(row.sequence)
                    .is_none()
                {
                    return Err(invalid("finalized FP peer has no writeback reservation"));
                }
                finalized_writeback
                    .published_writeback_sequences
                    .remove(&row.sequence);
            }
        }
        if let Some(pending_root) = pending_addresses.first() {
            let reservations = finalized_writeback
                .writeback_calendar
                .by_tick
                .values()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            match reservations.as_slice() {
                [] if finalized_writeback.published_writeback_sequences.is_empty() => {}
                [reservation]
                    if reservation.sequence() == pending_root.root_sequence
                        && reservation.admitted_tick() <= captured_tick
                        && finalized_writeback.published_writeback_sequences
                            == BTreeSet::from([pending_root.root_sequence]) =>
                {
                    finalized_writeback
                        .writeback_calendar
                        .remove_sequence(pending_root.root_sequence);
                    finalized_writeback
                        .published_writeback_sequences
                        .remove(&pending_root.root_sequence);
                }
                _ => {
                    return Err(invalid(
                        "pending store retains unsupported writeback ownership",
                    ));
                }
            }
        }
        let prune_tick = if completed_profile {
            captured_tick
                .checked_add(1)
                .ok_or(invalid("live capture tick overflows"))?
        } else {
            captured_tick
        };
        finalized_writeback.prune_writeback_calendar_before(prune_tick);
        if completed_profile {
            let completed = completed_fp
                .as_ref()
                .expect("completed profile has a result and reservation");
            let result = &completed.result;
            let reservation = completed.reservation;
            let reservations = finalized_writeback
                .writeback_calendar
                .by_tick
                .values()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            if reservations.as_slice() != [reservation]
                || !finalized_writeback.published_writeback_sequences.is_empty()
                || finalized_writeback.live_writeback_counted_sequences
                    != BTreeSet::from([result.sequence])
            {
                return Err(invalid(
                    "completed FP result does not own exactly one live writeback row",
                ));
            }
        } else if !finalized_writeback.writeback_calendar.by_tick.is_empty()
            || !finalized_writeback.published_writeback_sequences.is_empty()
            || !finalized_writeback
                .live_writeback_counted_sequences
                .is_empty()
        {
            return Err(invalid("unsupported live writeback ownership"));
        }
        if self.live_issue.resident_sequences().is_empty()
            && self.live_issue.requested_service_tick().is_none()
            && !completed_profile
            && !pending_profile
        {
            return Ok(None);
        }
        if completed_profile && resident_sequences.is_empty() {
            return Err(invalid("completed FP result has no dependent issue row"));
        }
        if resident_sequences.windows(2).any(|rows| rows[0] >= rows[1]) {
            return Err(invalid(
                "resident issue sequences are not unique and ordered",
            ));
        }
        let mut issue_rows = Vec::with_capacity(resident_sequences.len());
        for sequence in &resident_sequences {
            let packet = self
                .live_staged_issue_packet(*sequence)
                .ok_or(invalid("resident row has no bound issue packet"))?;
            let [fetch_request] = packet.consumed_requests() else {
                return Err(invalid("issue packet is not one completed fetch"));
            };
            issue_rows.push(RiscvO3LiveCheckpointIssueRow {
                sequence: *sequence,
                fetch_request: *fetch_request,
            });
        }
        let requested_tick = self
            .live_issue
            .requested_service_tick()
            .ok_or(invalid("resident issue queue has no service request"))?;
        let telemetry = self.live_issue.telemetry();
        let (profile, completed_result, reservation, finalized_rows) = match completed_fp {
            Some(completed) => {
                let checkpoint_reservation = checkpoint_reservation(
                    completed.reservation,
                    completed.result.raw_ready_tick,
                    RiscvO3LiveCheckpointWritebackSource::MemoryResult,
                    true,
                )?;
                (
                    RiscvO3LiveCheckpointProfile::CompletedFpLoad,
                    Some(completed.result),
                    Some(checkpoint_reservation),
                    completed.finalized_rows,
                )
            }
            None if pending_profile => (
                RiscvO3LiveCheckpointProfile::PendingDataAddress,
                None,
                None,
                Vec::new(),
            ),
            None => (
                RiscvO3LiveCheckpointProfile::ComputeQueue,
                None,
                None,
                Vec::new(),
            ),
        };
        Ok(Some(O3LiveCheckpointRuntimeProjection {
            stable: finalized_writeback.checkpoint_payload_with_projected_stats(self.stats()),
            profile,
            issue_rows,
            rename_rows: if pending_profile {
                Vec::new()
            } else {
                self.snapshot().rename_map().to_vec()
            },
            resident_sequences,
            service: RiscvO3LiveCheckpointService {
                requested_tick,
                mutation_generation: self.live_issue.mutation_generation(),
                last_service_generation: self.live_issue.last_service_generation(),
                telemetry: checkpoint_telemetry(telemetry),
            },
            finalized_writeback: finalized_writeback.checkpoint_finalized_writeback(),
            writeback_counted_sequences: if completed_result.is_some() {
                finalized_writeback
                    .live_writeback_counted_sequences
                    .iter()
                    .copied()
                    .collect()
            } else {
                Vec::new()
            },
            writeback_published_sequences: Vec::new(),
            reservation,
            completed_result,
            pending_addresses,
            finalized_rows,
        }))
    }

    fn checkpoint_completed_fp_load(
        &self,
        captured_tick: u64,
        resident_sequences: &[u64],
    ) -> Result<Option<O3CompletedFpCheckpointProjection>, Error> {
        let [] = self.live_data_accesses.as_slice() else {
            let [live] = self.live_data_accesses.as_slice() else {
                return Err(invalid(
                    "completed checkpoint has multiple live data accesses",
                ));
            };
            if live.outcome != O3LiveDataAccessOutcome::Completed
                || live.event_taken
                || live.younger_window_policy != O3DataAccessWindowPolicy::MemoryResultWindow
                || live.lsq_sequence_span != 1
                || live.forwarding_plan.is_some()
                || live.commit_tick.is_some()
            {
                return Err(invalid("live data access is not a completed FP result"));
            }
            let (destination, address, width) = match live.execution.execution().memory_access() {
                Some(MemoryAccessKind::FloatLoad { rd, address, width })
                    if matches!(width, MemoryWidth::Word | MemoryWidth::Doubleword) =>
                {
                    (*rd, *address, *width)
                }
                _ => return Err(invalid("completed result is not FLW or FLD")),
            };
            if live.execution.data_access_event_kind()
                != Some(crate::RiscvDataAccessEventKind::Completed)
            {
                return Err(invalid(
                    "completed FP result event is not response-complete",
                ));
            }
            let completion = live
                .memory_result
                .as_ref()
                .ok_or(invalid("completed FP result has no CPU completion"))?;
            let response_bytes = completion
                .bytes()
                .ok_or(invalid("completed FP result has no response bytes"))?;
            let response_tick = live
                .response_tick
                .ok_or(invalid("completed FP result has no response tick"))?;
            let latency_ticks = live
                .latency_ticks
                .ok_or(invalid("completed FP result has no response latency"))?;
            let reservation = self
                .memory_result_writeback_reservation(live.sequence)
                .ok_or(invalid("completed FP result has no memory reservation"))?;
            let raw_ready_tick = response_tick
                .checked_add(1)
                .ok_or(invalid("completed FP result raw-ready tick overflows"))?;
            let live_rob = self
                .snapshot
                .reorder_buffer
                .iter()
                .filter(|row| row.is_live_staged())
                .collect::<Vec<_>>();
            let lsq = self.snapshot.load_store_queue.as_slice();
            let load_owner = live_rob.first().copied();
            let resident_set = resident_sequences.iter().copied().collect::<BTreeSet<_>>();
            let finalized_owners = live_rob
                .iter()
                .copied()
                .skip(1)
                .filter(|row| !resident_set.contains(&row.sequence()))
                .collect::<Vec<_>>();
            if finalized_owners.len() > 1
                || (!finalized_owners.is_empty()
                    && (width != MemoryWidth::Doubleword
                        || live.sequence.checked_add(1) != Some(finalized_owners[0].sequence())))
            {
                return Err(invalid(
                    "completed FP result has unsupported finalized peers",
                ));
            }
            let mut normalized_finalized_sequences = BTreeSet::new();
            let finalized_rows = finalized_owners
                .iter()
                .map(|owner| {
                    let issued = self
                        .live_speculative_executions
                        .iter()
                        .find(|issued| issued.sequence == owner.sequence())
                        .ok_or(invalid("finalized FP peer has no speculative execution"))?;
                    let [fetch_request] = issued.consumed_requests.as_slice() else {
                        return Err(invalid("finalized FP peer is not one completed fetch"));
                    };
                    let raw_ready_tick = issued
                        .issue_tick
                        .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                            issued.execution.instruction(),
                        ))
                        .ok_or(invalid("finalized FP peer ready tick overflows"))?;
                    let peer_reservation = self.writeback_calendar.reservation(owner.sequence());
                    let source_peer = peer_reservation.is_some_and(|peer_reservation| {
                        completed_fld_peer_collision(
                            raw_ready_tick,
                            reservation.admitted_tick(),
                            reservation.slot(),
                            issued.raw_ready_tick,
                            issued.admitted_writeback_tick,
                            peer_reservation.slot(),
                        ) && peer_reservation
                            .matches_fixed_fu(owner.sequence(), issued.raw_ready_tick)
                            && issued.writeback_slot == Some(peer_reservation.slot())
                            && self
                                .live_writeback_counted_sequences
                                .contains(&owner.sequence())
                    });
                    let normalized_peer = peer_reservation.is_none()
                        && completed_fld_peer_collision(
                            raw_ready_tick,
                            reservation.admitted_tick(),
                            reservation.slot(),
                            issued.raw_ready_tick,
                            issued.admitted_writeback_tick,
                            1,
                        )
                        && owner.is_ready()
                        && owner.ready_tick() == issued.admitted_writeback_tick
                        && issued.writeback_slot.is_none()
                        && !self
                            .live_writeback_counted_sequences
                            .contains(&owner.sequence())
                        && has_exact_normalized_fld_peer_stats(
                            &self.checkpoint_finalized_writeback(),
                            captured_tick,
                            issued.admitted_writeback_tick,
                        );
                    if (owner.is_ready() && owner.ready_tick() != issued.admitted_writeback_tick)
                        || self.issue_width() != 2
                        || self.writeback_width() != 2
                        || !(source_peer || normalized_peer)
                        || issued.admitted_writeback_tick > captured_tick
                        || issued.producer_sequences.len() != 0
                        || !exact_fld_collision_peer(&issued.execution.instruction())
                        || owner.pc().get() != live.execution.execution().pc() + 4
                        || fetch_request.agent() != live.fetch_request.agent()
                        || fetch_request.sequence()
                            != live.fetch_request.sequence().saturating_add(1)
                        || issued.execution.memory_access().is_some()
                        || raw_ready_tick != issued.raw_ready_tick
                        || issued.raw_ready_tick != issued.admitted_writeback_tick
                    {
                        return Err(invalid("finalized FP peer ownership is inconsistent"));
                    }
                    if normalized_peer {
                        normalized_finalized_sequences.insert(owner.sequence());
                    }
                    Ok(RiscvO3LiveCheckpointIssueRow {
                        sequence: owner.sequence(),
                        fetch_request: *fetch_request,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let younger_sequences = live_rob
                .iter()
                .skip(1)
                .map(|row| row.sequence())
                .collect::<BTreeSet<_>>();
            let lsq_matches = matches!(lsq, [entry]
                if entry.sequence() == live.sequence
                    && entry.address() == Some(completion.physical_address())
                    && entry.bytes() == width.bytes() as u32
                    && entry.kind() == O3LoadStoreQueueKind::Load
                    && entry.is_completed());
            if response_tick > captured_tick
                || captured_tick > reservation.admitted_tick()
                || live.issue_tick.checked_add(latency_ticks) != Some(response_tick)
                || live.load_data.as_deref() != Some(response_bytes)
                || !completion.matches_issued_request(
                    live.fetch_request,
                    live.execution
                        .execution()
                        .memory_access()
                        .expect("validated FP load access"),
                    completion.physical_address(),
                    completion.size(),
                    completion.request_byte_offset(),
                )
                || completion.data_event_kind() != crate::RiscvDataAccessEventKind::Completed
                || completion.physical_address().get() != address
                || completion.size().bytes() != width.bytes() as u64
                || response_bytes.len() != width.bytes()
                || load_owner.map(|row| row.sequence()) != Some(live.sequence)
                || load_owner.is_some_and(|row| row.is_ready())
                || live_rob
                    .iter()
                    .skip(1)
                    .filter(|row| resident_set.contains(&row.sequence()))
                    .map(|row| row.sequence())
                    .ne(resident_sequences.iter().copied())
                || live_rob
                    .iter()
                    .skip(1)
                    .any(|row| resident_set.contains(&row.sequence()) && row.is_ready())
                || younger_sequences != self.live_data_access_younger_sequences
                || finalized_rows.len() != self.live_speculative_executions.len()
                || load_owner.and_then(|row| row.rename_destination())
                    != Some((
                        O3RegisterClass::FloatingPoint,
                        u32::from(destination.index()),
                    ))
                || load_owner.and_then(|row| row.destination()).is_none()
                || !lsq_matches
            {
                return Err(invalid("completed FP result ownership is inconsistent"));
            }
            let request_byte_offset = u32::try_from(completion.request_byte_offset())
                .map_err(|_| invalid("completed FP byte offset does not fit O3LC"))?;
            let rob_last_sequence = resident_sequences.last().copied().unwrap_or(live.sequence);
            return Ok(Some(O3CompletedFpCheckpointProjection {
                result: RiscvO3LiveCheckpointCompletedFpLoad {
                    fetch_request: live.fetch_request,
                    data_request: live.data_request,
                    sequence: live.sequence,
                    lsq_sequence: live.sequence,
                    rob_first_sequence: live.sequence,
                    rob_last_sequence,
                    issue_tick: live.issue_tick,
                    response_tick,
                    raw_ready_tick,
                    admitted_tick: reservation.admitted_tick(),
                    latency_ticks,
                    physical_address: completion.physical_address(),
                    access_size: completion.size(),
                    request_byte_offset,
                    response_bytes: response_bytes.to_vec(),
                    destination,
                    width,
                },
                reservation,
                finalized_rows,
                normalized_finalized_sequences,
            }));
        };
        Ok(None)
    }

    pub(crate) fn prepare_live_checkpoint_restore(
        &self,
        stable: O3RuntimeCheckpointPayload,
        live: &RiscvO3LiveCheckpointPayload,
    ) -> Result<PreparedRiscvO3LiveRestore, Error> {
        let pending = if live.profile == RiscvO3LiveCheckpointProfile::PendingDataAddress {
            Some(pending_address::validate_restore_payload(live)?)
        } else {
            None
        };
        let completed = match live.profile {
            RiscvO3LiveCheckpointProfile::ComputeQueue => {
                if live.reservation.is_some()
                    || live.completed_result.is_some()
                    || !live.writeback_counted_sequences.is_empty()
                    || !live.writeback_published_sequences.is_empty()
                    || live.events.iter().any(|event| {
                        event.memory_access.is_some() || event.data_access_event_kind.is_some()
                    })
                    || !stable.snapshot().load_store_queue().is_empty()
                {
                    return Err(invalid("compute profile contains data/writeback ownership"));
                }
                None
            }
            RiscvO3LiveCheckpointProfile::CompletedFpLoad => {
                let (Some(reservation), Some(result)) =
                    (live.reservation, live.completed_result.as_ref())
                else {
                    return Err(invalid("completed FP profile lacks result ownership"));
                };
                if live.writeback_counted_sequences != [result.sequence]
                    || !live.writeback_published_sequences.is_empty()
                    || live.issued_fetch_requests != [result.fetch_request]
                    || reservation.sequence != result.sequence
                    || reservation.raw_ready_tick != result.raw_ready_tick
                    || reservation.admitted_tick != result.admitted_tick
                    || reservation.source != RiscvO3LiveCheckpointWritebackSource::MemoryResult
                    || !reservation.decision_counted
                    || result.response_tick > live.captured_tick
                    || live.captured_tick > result.admitted_tick
                {
                    return Err(invalid("completed FP result ownership is inconsistent"));
                }
                Some((result, reservation))
            }
            RiscvO3LiveCheckpointProfile::PendingDataAddress => None,
        };
        if stable.pending_live_retire_gate().is_some() {
            return Err(invalid(
                "compute profile contains a stable live retire gate",
            ));
        }
        if live.service.has_invalid_identity_at(live.captured_tick) {
            return Err(invalid("last service identity is invalid"));
        }
        let finalized_writeback_is_valid = match live.profile {
            RiscvO3LiveCheckpointProfile::ComputeQueue => live
                .finalized_writeback
                .is_valid_without_live_calendar_at(live.captured_tick),
            RiscvO3LiveCheckpointProfile::CompletedFpLoad => live
                .finalized_writeback
                .is_valid_without_live_calendar_closed_through(live.captured_tick),
            RiscvO3LiveCheckpointProfile::PendingDataAddress => live
                .finalized_writeback
                .is_valid_without_live_calendar_at(live.captured_tick),
        };
        if !finalized_writeback_is_valid {
            return Err(invalid("finalized writeback ownership is inconsistent"));
        }
        let occupancy = u64::try_from(live.resident_sequences.len())
            .map_err(|_| invalid("issue occupancy does not fit telemetry"))?;
        if live.service.telemetry.current_occupancy != occupancy
            || live.service.telemetry.peak_occupancy < occupancy
        {
            return Err(invalid("issue telemetry disagrees with resident queue"));
        }
        validate_unique_values("resident sequence", live.resident_sequences.iter().copied())?;
        validate_unique_values(
            "issue sequence",
            live.issue_rows.iter().map(|row| row.sequence),
        )?;
        validate_unique_requests(
            "issue fetch request",
            live.issue_rows.iter().map(|row| row.fetch_request),
        )?;
        if live.rename_rows.iter().any(|row| {
            row.architectural() >= 32
                || row.physical().is_invalid()
                || !matches!(
                    row.register_class(),
                    O3RegisterClass::Integer | O3RegisterClass::FloatingPoint
                )
        }) {
            return Err(invalid("rename row is not a valid scalar destination"));
        }
        validate_unique_requests(
            "event fetch request",
            live.events.iter().map(|event| event.fetch.request_id()),
        )?;
        validate_unique_requests(
            "executed fetch request",
            live.executed_fetch_requests.iter().copied(),
        )?;
        validate_unique_requests(
            "issued fetch request",
            live.issued_fetch_requests.iter().copied(),
        )?;
        let event_requests = live
            .events
            .iter()
            .map(|event| event.fetch.request_id())
            .collect::<BTreeSet<_>>();
        let executed_requests = live
            .executed_fetch_requests
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if !executed_requests.is_subset(&event_requests)
            || (completed.is_none() && !live.issued_fetch_requests.is_empty())
        {
            return Err(invalid("execution membership differs from replay events"));
        }
        validate_unique_values(
            "rename architectural destination",
            live.rename_rows.iter().map(|row| {
                let class = match row.register_class() {
                    O3RegisterClass::Integer => 0_u64,
                    O3RegisterClass::FloatingPoint => 1,
                    _ => 2,
                };
                (class << 32) | u64::from(row.architectural())
            }),
        )?;
        validate_unique_values(
            "rename physical register",
            live.rename_rows
                .iter()
                .map(|row| u64::from(row.physical().get())),
        )?;
        if live.resident_sequences
            != live
                .issue_rows
                .iter()
                .map(|row| row.sequence)
                .collect::<Vec<_>>()
        {
            return Err(invalid("issue membership differs from resident queue"));
        }
        let stable_stats = stable.stats();
        let mut runtime = self.clone();
        runtime
            .restore_checkpoint_payload(stable)
            .map_err(|_| invalid("stable O3 runtime is invalid"))?;
        if pending.is_none() && runtime.snapshot().rename_map() != live.rename_rows.as_slice() {
            return Err(invalid("live rename projection disagrees with O3RT"));
        }
        let stable_live_rows = runtime
            .snapshot
            .reorder_buffer
            .iter()
            .filter(|row| row.is_live_staged())
            .collect::<Vec<_>>();
        let issue_by_sequence = live
            .issue_rows
            .iter()
            .map(|row| (row.sequence, row.fetch_request))
            .collect::<BTreeMap<_, _>>();
        let finalized_rows = if let Some(pending) = pending {
            pending_address::validate_stable_owner_set(&runtime, live, pending)?;
            Vec::new()
        } else if let Some((result, reservation)) = completed {
            if stable_live_rows.len() != live.events.len()
                || stable_live_rows.first().map(|row| row.sequence()) != Some(result.sequence)
                || stable_live_rows.last().map(|row| row.sequence())
                    != Some(result.rob_last_sequence)
                || stable_live_rows.first().is_some_and(|row| row.is_ready())
            {
                return Err(invalid("completed FP ROB span disagrees with O3RT"));
            }
            let mut finalized_rows = Vec::new();
            for (owner, event) in stable_live_rows.iter().zip(&live.events) {
                let request = event.fetch.request_id();
                if owner.sequence() == result.sequence {
                    if request != result.fetch_request {
                        return Err(invalid("completed FP load event disagrees with O3RT"));
                    }
                } else if let Some(expected_request) = issue_by_sequence.get(&owner.sequence()) {
                    if owner.is_ready() || request != *expected_request {
                        return Err(invalid("resident FP issue row disagrees with O3RT"));
                    }
                } else {
                    if !owner.is_ready() || owner.ready_tick() > live.captured_tick {
                        return Err(invalid("finalized FP peer disagrees with O3RT"));
                    }
                    finalized_rows.push((**owner, request));
                }
            }
            if finalized_rows.len() > 1
                || (!finalized_rows.is_empty()
                    && (result.width != MemoryWidth::Doubleword
                        || result.sequence.checked_add(1) != Some(finalized_rows[0].0.sequence())
                        || runtime.issue_width() != 2
                        || runtime.writeback_width() != 2
                        || reservation.slot != 0
                        || finalized_rows[0].0.ready_tick() != result.admitted_tick))
                || stable_live_rows
                    .iter()
                    .skip(1)
                    .filter(|row| !row.is_ready())
                    .map(|row| row.sequence())
                    .ne(live.resident_sequences.iter().copied())
            {
                return Err(invalid("completed FP finalized peer shape is invalid"));
            }
            finalized_rows
        } else {
            if stable_live_rows.iter().any(|row| row.is_ready())
                || stable_live_rows
                    .iter()
                    .map(|row| row.sequence())
                    .ne(live.resident_sequences.iter().copied())
                || live
                    .events
                    .iter()
                    .map(|event| event.fetch.request_id())
                    .ne(live.issue_rows.iter().map(|row| row.fetch_request))
            {
                return Err(invalid(
                    "live queue does not close over pending live-staged O3RT owners",
                ));
            }
            Vec::new()
        };
        if let Some((result, _)) = completed {
            if live.executed_fetch_requests != [result.fetch_request] {
                return Err(invalid("completed FP execution membership is inconsistent"));
            }
        }
        let rob = runtime
            .snapshot
            .reorder_buffer
            .iter()
            .map(|row| (row.sequence(), *row))
            .collect::<BTreeMap<_, _>>();
        let mut rebuilt_by_request = BTreeMap::new();
        let mut rebuilt_events = Vec::with_capacity(live.events.len());
        for projected in &live.events {
            let rebuilt = projected.rebuild()?;
            rebuilt_by_request.insert(projected.fetch.request_id(), rebuilt.clone());
            rebuilt_events.push(rebuilt);
        }
        let memory_result_authorization = if let Some((result, reservation)) = completed {
            let load = rebuilt_by_request
                .get(&result.fetch_request)
                .ok_or(invalid("completed FP result has no replay event"))?;
            let expected_access = MemoryAccessKind::FloatLoad {
                rd: result.destination,
                address: result.physical_address.get(),
                width: result.width,
            };
            let load_owner = rob
                .get(&result.sequence)
                .filter(|entry| entry.is_live_staged())
                .ok_or(invalid("completed FP result has no O3RT ROB owner"))?;
            let [lsq] = runtime.snapshot.load_store_queue.as_slice() else {
                return Err(invalid("completed FP result does not own one O3RT LSQ row"));
            };
            let result_physical = load_owner
                .destination()
                .ok_or(invalid("completed FP result has no physical destination"))?;
            if result.rob_first_sequence != result.sequence
                || result.rob_last_sequence
                    != live
                        .resident_sequences
                        .last()
                        .copied()
                        .unwrap_or(result.sequence)
                || result.lsq_sequence != result.sequence
                || load.execution().memory_access() != Some(&expected_access)
                || load.data_access_event_kind() != Some(crate::RiscvDataAccessEventKind::Completed)
                || load_owner.pc().get() != load.execution().pc()
                || load_owner.rename_destination()
                    != Some((
                        O3RegisterClass::FloatingPoint,
                        u32::from(result.destination.index()),
                    ))
                || !live.rename_rows.iter().any(|row| {
                    row.register_class() == O3RegisterClass::FloatingPoint
                        && row.architectural() == u32::from(result.destination.index())
                        && row.physical() == result_physical
                })
                || lsq.sequence() != result.lsq_sequence
                || lsq.address() != Some(result.physical_address)
                || lsq.bytes() != result.access_size.bytes() as u32
                || lsq.kind() != O3LoadStoreQueueKind::Load
                || !lsq.is_completed()
                || result.issue_tick.checked_add(result.latency_ticks) != Some(result.response_tick)
                || result.response_tick.checked_add(1) != Some(result.raw_ready_tick)
                || result.width.bytes() != result.response_bytes.len()
                || result.access_size.bytes() != result.response_bytes.len() as u64
            {
                return Err(invalid("completed FP result disagrees with O3RT"));
            }
            let request_byte_offset = usize::try_from(result.request_byte_offset)
                .map_err(|_| invalid("completed FP byte offset does not fit host"))?;
            let completion = RiscvDataCompletion::from_issued_response(
                result.fetch_request,
                expected_access,
                result.physical_address,
                result.access_size,
                request_byte_offset,
                Some(result.response_bytes.clone()),
            );
            runtime.live_staged_fetch_identities.insert(
                result.sequence,
                O3LiveStagedFetchIdentity::new(load.instruction()),
            );
            runtime.live_data_accesses.push(O3LiveDataAccess {
                fetch_request: result.fetch_request,
                data_request: result.data_request,
                execution: load.clone(),
                sequence: result.sequence,
                lsq_sequence_span: 1,
                issue_tick: result.issue_tick,
                issue_rob_occupancy: 1,
                issue_lsq_occupancy: 1,
                younger_window_policy: O3DataAccessWindowPolicy::MemoryResultWindow,
                response_tick: Some(result.response_tick),
                latency_ticks: Some(result.latency_ticks),
                commit_tick: None,
                load_data: Some(result.response_bytes.clone()),
                memory_result: Some(completion),
                forwarding_plan: None,
                outcome: O3LiveDataAccessOutcome::Completed,
                event_taken: false,
            });
            runtime
                .live_data_access_younger_sequences
                .extend(stable_live_rows.iter().skip(1).map(|row| row.sequence()));

            let finalized_stats =
                checkpoint_finalized_stats(stable_stats, &live.finalized_writeback)?;
            runtime
                .restore_compute_checkpoint_writeback(&live.finalized_writeback, finalized_stats)
                .map_err(|_| invalid("finalized writeback ownership is invalid"))?;
            let slot = usize::try_from(reservation.slot)
                .map_err(|_| invalid("completed FP reservation slot does not fit host"))?;
            if reservation.admitted_tick < reservation.raw_ready_tick
                || slot >= runtime.writeback_width()
            {
                return Err(invalid("completed FP reservation calendar is invalid"));
            }
            let restored_reservation = runtime
                .restore_completed_fp_checkpoint_writeback(
                    result.sequence,
                    result.raw_ready_tick,
                    result.admitted_tick,
                    slot,
                )
                .map_err(|_| invalid("completed FP reservation cannot be restored"))?;
            if checkpoint_reservation(
                restored_reservation,
                result.raw_ready_tick,
                RiscvO3LiveCheckpointWritebackSource::MemoryResult,
                true,
            )? != reservation
                || runtime.stats != stable_stats
                || runtime.live_writeback_counted_sequences != BTreeSet::from([result.sequence])
                || !runtime.published_writeback_sequences.is_empty()
            {
                return Err(invalid("completed FP writeback split did not recompose"));
            }
            let range = AddressRange::new(result.physical_address, result.access_size)
                .map_err(|_| invalid("completed FP authorization range is invalid"))?;
            Some((
                result.fetch_request,
                O3MemoryResultWindowAuthorization::restored_completed_fp_load(range),
            ))
        } else {
            runtime
                .restore_compute_checkpoint_writeback(&live.finalized_writeback, stable_stats)
                .map_err(|_| invalid("writeback ownership does not recompose O3RT stats"))?;
            None
        };
        for (owner, request) in &finalized_rows {
            let (result, _) = completed.expect("finalized FP peer requires completed result");
            let event = rebuilt_by_request
                .get(request)
                .ok_or(invalid("finalized FP peer has no replay event"))?;
            let latency = crate::riscv_fu_latency::riscv_execute_wait_cycles(event.instruction());
            let issue_tick = owner
                .ready_tick()
                .checked_sub(latency)
                .ok_or(invalid("finalized FP peer issue tick underflows"))?;
            if owner.pc().get() != event.execution().pc()
                || owner.pc().get() != load_owner_pc(&rob, result.sequence)? + 4
                || !exact_fld_collision_peer(&event.instruction())
                || request.agent() != result.fetch_request.agent()
                || request.sequence() != result.fetch_request.sequence().saturating_add(1)
                || event.execution().memory_access().is_some()
                || event.data_access_event_kind().is_some()
            {
                return Err(invalid("finalized FP peer event is unsupported"));
            }
            runtime.live_staged_fetch_identities.insert(
                owner.sequence(),
                O3LiveStagedFetchIdentity::new(event.instruction()),
            );
            runtime
                .live_speculative_executions
                .push(O3LiveSpeculativeExecution {
                    consumed_requests: vec![*request],
                    sequence: owner.sequence(),
                    producer_sequences: Vec::new(),
                    issue_tick,
                    raw_ready_tick: owner.ready_tick(),
                    admitted_writeback_tick: owner.ready_tick(),
                    writeback_slot: None,
                    execution: event.execution().clone(),
                });
        }
        if let Some(pending) = pending {
            pending_address::restore(&mut runtime, live, pending)?;
        } else {
            for row in &live.issue_rows {
                let owner = rob
                    .get(&row.sequence)
                    .filter(|entry| entry.is_live_staged())
                    .ok_or(invalid("issue row has no live-staged O3RT owner"))?;
                let event = rebuilt_by_request
                    .get(&row.fetch_request)
                    .ok_or(invalid("issue row has no replayable event"))?;
                if owner.pc().get() != event.execution().pc() {
                    return Err(invalid("issue event PC disagrees with O3RT owner"));
                }
                let raw = event
                    .fetch()
                    .data()
                    .ok_or(invalid("replayable event has no fetched bytes"))?;
                let mut bytes = [0_u8; 4];
                bytes[..raw.len()].copy_from_slice(raw);
                let decoded = RiscvInstruction::decode_with_length(u32::from_le_bytes(bytes))
                    .map_err(|_| invalid("replayable event instruction does not decode"))?;
                runtime.live_staged_fetch_identities.insert(
                    row.sequence,
                    O3LiveStagedFetchIdentity::new(event.instruction()),
                );
                if !runtime.bind_live_staged_issue_packet_at_sequence(
                    row.sequence,
                    decoded,
                    &[row.fetch_request],
                    live.service.requested_tick,
                ) {
                    return Err(invalid("production issue binder rejected restored row"));
                }
            }
            runtime.live_issue.install_checkpoint_projection(
                live.resident_sequences.clone(),
                live.service.requested_tick,
                live.service.mutation_generation,
                live.service.last_service_generation,
                restore_telemetry(live.service.telemetry),
            );
            match O3LiveIssueQueue::materialize(&runtime, &live.resident_sequences)
                .map_err(|_| invalid("restored issue queue does not materialize"))?
            {
                O3LiveIssueQueueCapture::Ready(queue)
                    if queue
                        .entries()
                        .iter()
                        .map(|row| row.sequence())
                        .eq(live.resident_sequences.iter().copied())
                        && completed.is_none_or(|(result, _)| {
                            exact_completed_fp_dependent(
                                &queue,
                                result.sequence,
                                result.destination,
                                result.width,
                            )
                        }) => {}
                _ => return Err(invalid("restored issue queue membership changed")),
            }
        }
        rebuilt_events.retain(|event| executed_requests.contains(&event.fetch().request_id()));
        Ok(PreparedRiscvO3LiveRestore {
            runtime,
            events: rebuilt_events,
            memory_result_authorization,
        })
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_set_completed_data_request_for_test(
        &mut self,
        data_request: MemoryRequestId,
    ) -> bool {
        let Some(access) = self.live_data_accesses.as_mut_slice().first_mut() else {
            return false;
        };
        if access.outcome != O3LiveDataAccessOutcome::Completed {
            return false;
        }
        access.data_request = data_request;
        true
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_complete_service_and_request_for_test(
        &mut self,
        service_tick: u64,
        requested_tick: u64,
    ) {
        self.live_issue.request_service_at(service_tick);
        assert!(self.live_issue.begin_service_at(service_tick));
        self.live_issue.seal_current_decision();
        self.live_issue.request_service_at(requested_tick);
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_reopen_finalized_writeback_for_test(&mut self, tick: u64) {
        let mut finalized = self.checkpoint_finalized_writeback();
        finalized.cycles -= 1;
        finalized.closed_before_tick = tick;
        finalized.partial_cycle_ticks.insert(tick);
        finalized.partial_ready_rows_by_tick.insert(tick, 1);
        self.finalized_writeback_port_stats =
            O3FinalizedWritebackPortStats::from_checkpoint_projection(&finalized);
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_publish_fixed_writeback_for_test(&mut self, sequence: u64, tick: u64) {
        self.reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(sequence, tick)])
            .unwrap();
        self.finalize_writeback_publication(sequence);
    }

    #[cfg(test)]
    pub(crate) fn checkpoint_mark_data_younger_for_test(
        &mut self,
        sequences: impl IntoIterator<Item = u64>,
    ) {
        self.live_data_access_younger_sequences.extend(sequences);
    }
}

fn exact_completed_fp_dependent(
    queue: &O3LiveIssueQueue,
    result_sequence: u64,
    destination: rem6_isa_riscv::FloatRegister,
    width: MemoryWidth,
) -> bool {
    let [row] = queue.entries() else {
        return false;
    };
    let instruction = row.packet().decoded().instruction();
    let exact_multiply = matches!(
        (width, instruction),
        (MemoryWidth::Word, RiscvInstruction::FloatMulS { .. })
            | (MemoryWidth::Doubleword, RiscvInstruction::FloatMulD { .. })
    );
    let [producer] = row.scheduling().data_producers() else {
        return false;
    };
    row.scheduling().op_class() == crate::o3_pipeline::O3IssueOpClass::Float
        && exact_multiply
        && producer.sequence() == result_sequence
        && producer.source() == O3ArchitecturalRegister::floating_point(destination)
}

fn has_exact_normalized_fld_peer_stats(
    finalized: &RiscvO3LiveCheckpointFinalizedWriteback,
    captured_tick: u64,
    peer_tick: u64,
) -> bool {
    captured_tick == peer_tick
        && finalized.closed_before_tick == peer_tick
        && finalized.partial_cycle_ticks == BTreeSet::from([peer_tick])
        && finalized.partial_ready_rows_by_tick == BTreeMap::from([(peer_tick, 1)])
        && finalized.partial_deferred_rows_by_tick.is_empty()
}

const fn completed_fld_peer_collision(
    result_raw: u64,
    result_admitted: u64,
    result_slot: usize,
    peer_raw: u64,
    peer_admitted: u64,
    peer_slot: usize,
) -> bool {
    result_raw == result_admitted
        && peer_raw == result_raw
        && peer_admitted == result_admitted
        && result_slot == 0
        && peer_slot == 1
}

fn exact_fld_collision_peer(instruction: &RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::FloatSqrtD { rd, rs1, rounding_mode }
            if rd.index() == 6 && rs1.index() == 3
                && *rounding_mode == RiscvFloatRoundingMode::RoundNearestEven
    )
}

fn load_owner_pc(rob: &BTreeMap<u64, O3ReorderBufferEntry>, sequence: u64) -> Result<u64, Error> {
    rob.get(&sequence)
        .map(|owner| owner.pc().get())
        .ok_or(invalid("finalized FP peer has no load owner"))
}

fn checkpoint_reservation(
    value: O3WritebackReservation,
    raw_ready_tick: u64,
    source: RiscvO3LiveCheckpointWritebackSource,
    decision_counted: bool,
) -> Result<RiscvO3LiveCheckpointReservation, Error> {
    Ok(RiscvO3LiveCheckpointReservation {
        sequence: value.sequence(),
        raw_ready_tick,
        admitted_tick: value.admitted_tick(),
        slot: u32::try_from(value.slot())
            .map_err(|_| invalid("writeback slot does not fit O3LC"))?,
        source,
        decision_counted,
    })
}

fn checkpoint_finalized_stats(
    mut stats: O3RuntimeStats,
    finalized: &RiscvO3LiveCheckpointFinalizedWriteback,
) -> Result<O3RuntimeStats, Error> {
    let partial_cycles = u64::try_from(finalized.partial_cycle_ticks.len())
        .map_err(|_| invalid("finalized writeback cycle count does not fit O3RT"))?;
    stats.writeback_port_cycles = finalized
        .cycles
        .checked_add(partial_cycles)
        .ok_or(invalid("finalized writeback cycle count overflows"))?;
    stats.writeback_port_admitted_rows = finalized.admitted_rows;
    stats.writeback_port_deferred_rows = finalized.deferred_rows;
    stats.writeback_port_deferred_row_cycles = finalized.deferred_row_cycles;
    stats.writeback_port_max_ready_rows_per_cycle = finalized
        .partial_ready_rows_by_tick
        .values()
        .copied()
        .fold(finalized.max_ready_rows_per_cycle, u64::max);
    stats.writeback_port_max_deferred_rows = finalized
        .partial_deferred_rows_by_tick
        .values()
        .copied()
        .fold(finalized.max_deferred_rows, u64::max);
    Ok(stats)
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidProfileShape { reason }
}
