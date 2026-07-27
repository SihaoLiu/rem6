use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::MemoryRequestId;

use super::o3_runtime_issue::queue::{O3LiveIssueQueue, O3LiveIssueQueueCapture};
#[cfg(test)]
use super::o3_runtime_issue::{calendar::O3LiveIssueCalendar, O3LiveIssueDependencyTable};
use super::*;
use crate::{
    RiscvCpuExecutionEvent, RiscvO3LiveCheckpointError as Error,
    RiscvO3LiveCheckpointFinalizedWriteback, RiscvO3LiveCheckpointIssueRow,
    RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile, RiscvO3LiveCheckpointService,
    RiscvO3LiveCheckpointTelemetry,
};

pub(crate) struct O3ComputeCheckpointProjection {
    pub(crate) issue_rows: Vec<RiscvO3LiveCheckpointIssueRow>,
    pub(crate) rename_rows: Vec<O3RenameMapEntry>,
    pub(crate) resident_sequences: Vec<u64>,
    pub(crate) service: RiscvO3LiveCheckpointService,
    pub(crate) finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback,
}

pub(crate) struct PreparedRiscvO3LiveRestore {
    runtime: O3RuntimeState,
    events: Vec<RiscvCpuExecutionEvent>,
}

impl PreparedRiscvO3LiveRestore {
    pub(crate) fn into_parts(self) -> (O3RuntimeState, Vec<RiscvCpuExecutionEvent>) {
        (self.runtime, self.events)
    }
}

impl O3RuntimeState {
    #[cfg(test)]
    pub(crate) fn live_issue_resident_sequences_for_checkpoint(&self) -> Vec<u64> {
        self.live_issue.resident_sequences().to_vec()
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
    ) -> Result<Option<O3ComputeCheckpointProjection>, Error> {
        if self.live_issue.transaction_active()
            || !self.pending_data_accesses.is_empty()
            || self.store_forwarding_window != Default::default()
            || !self.live_retired_instructions.is_empty()
            || !self.live_speculative_executions.is_empty()
            || !self.writeback_calendar.by_tick.is_empty()
            || !self.published_writeback_sequences.is_empty()
            || !self.live_writeback_counted_sequences.is_empty()
            || !self.live_control_lineages.is_empty()
            || !self.live_serializing_control_sequences.is_empty()
            || !self.invalidated_live_staged_fetch_identities.is_empty()
            || !self.committed_live_staged_fetch_identities.is_empty()
            || self.deferred_live_data_access_execution.is_some()
            || !self.live_data_accesses.is_empty()
            || self.has_pending_data_address()
            || !self.live_data_access_younger_sequences.is_empty()
        {
            return Err(invalid("unsupported transient O3 authority"));
        }
        if self.live_issue.resident_sequences().is_empty()
            && self.live_issue.requested_service_tick().is_none()
        {
            return Ok(None);
        }
        let resident_sequences = self.live_issue.resident_sequences().to_vec();
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
        Ok(Some(O3ComputeCheckpointProjection {
            issue_rows,
            rename_rows: self.snapshot().rename_map().to_vec(),
            resident_sequences,
            service: RiscvO3LiveCheckpointService {
                requested_tick,
                mutation_generation: self.live_issue.mutation_generation(),
                last_service_generation: self.live_issue.last_service_generation(),
                telemetry: checkpoint_telemetry(telemetry),
            },
            finalized_writeback: self.checkpoint_finalized_writeback(),
        }))
    }

    pub(crate) fn prepare_live_checkpoint_restore(
        &self,
        stable: O3RuntimeCheckpointPayload,
        live: &RiscvO3LiveCheckpointPayload,
    ) -> Result<PreparedRiscvO3LiveRestore, Error> {
        if live.profile != RiscvO3LiveCheckpointProfile::ComputeQueue
            || live.reservation.is_some()
            || live.completed_result.is_some()
            || !live.writeback_counted_sequences.is_empty()
            || !live.writeback_published_sequences.is_empty()
            || live.events.iter().any(|event| {
                event.memory_access.is_some() || event.data_access_event_kind.is_some()
            })
        {
            return Err(invalid("compute profile contains data/writeback ownership"));
        }
        if !stable.snapshot().load_store_queue().is_empty() {
            return Err(invalid("compute profile contains stable LSQ ownership"));
        }
        if stable.pending_live_retire_gate().is_some() {
            return Err(invalid(
                "compute profile contains a stable live retire gate",
            ));
        }
        if live.service.has_invalid_identity_at(live.captured_tick) {
            return Err(invalid("last service identity is invalid"));
        }
        if !live
            .finalized_writeback
            .is_valid_without_live_calendar_at(live.captured_tick)
        {
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
        if executed_requests != event_requests || !live.issued_fetch_requests.is_empty() {
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
        if live
            .events
            .iter()
            .map(|event| event.fetch.request_id())
            .ne(live.issue_rows.iter().map(|row| row.fetch_request))
        {
            return Err(invalid("replay events differ from issue membership"));
        }

        let stable_stats = stable.stats();
        let mut runtime = self.clone();
        runtime
            .restore_checkpoint_payload(stable)
            .map_err(|_| invalid("stable O3 runtime is invalid"))?;
        if runtime.snapshot().rename_map() != live.rename_rows.as_slice() {
            return Err(invalid("live rename projection disagrees with O3RT"));
        }
        let stable_live_rows = runtime
            .snapshot
            .reorder_buffer
            .iter()
            .filter(|row| row.is_live_staged())
            .collect::<Vec<_>>();
        if stable_live_rows.iter().any(|row| row.is_ready())
            || stable_live_rows
                .iter()
                .map(|row| row.sequence())
                .ne(live.resident_sequences.iter().copied())
        {
            return Err(invalid(
                "live queue does not close over pending live-staged O3RT owners",
            ));
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
                    .eq(live.resident_sequences.iter().copied()) => {}
            _ => return Err(invalid("restored issue queue membership changed")),
        }
        runtime
            .restore_compute_checkpoint_writeback(&live.finalized_writeback, stable_stats)
            .map_err(|_| invalid("writeback ownership does not recompose O3RT stats"))?;
        Ok(PreparedRiscvO3LiveRestore {
            runtime,
            events: rebuilt_events,
        })
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
}

fn checkpoint_telemetry(value: O3LiveIssueTelemetry) -> RiscvO3LiveCheckpointTelemetry {
    RiscvO3LiveCheckpointTelemetry {
        enqueued_rows: value.enqueued_rows(),
        service_turns: value.service_turns(),
        wake_requests: value.wake_requests(),
        current_occupancy: value.current_occupancy(),
        peak_occupancy: value.peak_occupancy(),
        scalar_integer_issued_rows: value.scalar_integer_issued_rows(),
        integer_mul_div_issued_rows: value.integer_mul_div_issued_rows(),
        memory_agu_issued_rows: value.memory_agu_issued_rows(),
        control_issued_rows: value.control_issued_rows(),
        scalar_float_issued_rows: value.scalar_float_issued_rows(),
        vector_to_scalar_issued_rows: value.vector_to_scalar_issued_rows(),
    }
}

fn restore_telemetry(value: RiscvO3LiveCheckpointTelemetry) -> O3LiveIssueTelemetry {
    O3LiveIssueTelemetry::from_checkpoint([
        value.enqueued_rows,
        value.service_turns,
        value.wake_requests,
        value.current_occupancy,
        value.peak_occupancy,
        value.scalar_integer_issued_rows,
        value.integer_mul_div_issued_rows,
        value.memory_agu_issued_rows,
        value.control_issued_rows,
        value.scalar_float_issued_rows,
        value.vector_to_scalar_issued_rows,
    ])
}

fn validate_unique_values(
    field: &'static str,
    values: impl IntoIterator<Item = u64>,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Error::DuplicateValue { field, value });
        }
    }
    Ok(())
}

fn validate_unique_requests(
    field: &'static str,
    values: impl IntoIterator<Item = MemoryRequestId>,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for request in values {
        if !seen.insert(request) {
            return Err(Error::DuplicateValue {
                field,
                value: request.sequence(),
            });
        }
    }
    Ok(())
}

fn invalid(reason: &'static str) -> Error {
    Error::InvalidProfileShape { reason }
}
