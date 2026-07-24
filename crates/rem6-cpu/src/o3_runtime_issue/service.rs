use rem6_isa_riscv::RiscvHartState;

use super::*;
use super::{
    calendar::O3LiveIssueCalendar,
    queue::{O3LiveIssueQueue, O3LiveIssueQueueCapture},
};
use crate::o3_pipeline::O3ScopedReadyInstruction;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct O3LiveIssueServiceOutcome {
    issued_rows: usize,
    next_service_tick: Option<u64>,
    replay_boundary: Option<u64>,
    waits_for_pending_dependency: bool,
}

impl O3LiveIssueServiceOutcome {
    #[cfg(test)]
    pub(in crate::o3_runtime) const fn issued_rows(self) -> usize {
        self.issued_rows
    }

    pub(in crate::o3_runtime) const fn next_service_tick(self) -> Option<u64> {
        self.next_service_tick
    }

    pub(in crate::o3_runtime) const fn replay_boundary(self) -> Option<u64> {
        self.replay_boundary
    }

    const fn waits_for_pending_dependency(self) -> bool {
        self.waits_for_pending_dependency
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3LiveIssuePostService {
    observed_decision: bool,
    resource_blocked_sequences: Vec<u64>,
    dependency_blocked_sequences: Vec<u64>,
    max_rows_at_tick: usize,
    next_service_tick: Option<u64>,
    replay_boundary: Option<u64>,
    no_wake_sequence: Option<u64>,
    waits_for_pending_dependency: bool,
}

impl O3RuntimeState {
    pub const fn stats(&self) -> O3RuntimeStats {
        match self.live_issue.projected_decision() {
            Some(delta) => self.stats.project_issue_cycle(
                delta.new_cycle,
                delta.issued_rows,
                delta.resource_blocked_rows,
                delta.dependency_blocked_rows,
                delta.max_rows_at_tick,
            ),
            None => self.stats,
        }
    }

    pub(crate) fn live_issue_service_tick(&self) -> Option<u64> {
        self.live_issue.requested_service_tick()
    }

    pub(crate) fn live_issue_is_quiescent(&self) -> bool {
        self.live_issue.is_quiescent()
    }

    pub fn live_issue_telemetry(&self) -> O3LiveIssueTelemetry {
        self.live_issue.telemetry()
    }

    pub fn live_issue_trace_records(&self) -> &[O3LiveIssueTraceRecord] {
        self.live_issue.trace_records()
    }

    pub(in crate::o3_runtime) fn seal_live_issue_decision_before(&mut self, tick: u64) {
        if let Some(delta) = self.live_issue.take_decision_before(tick) {
            self.stats.record_issue_cycle(
                delta.new_cycle,
                delta.issued_rows,
                delta.resource_blocked_rows,
                delta.dependency_blocked_rows,
                delta.max_rows_at_tick,
            );
        }
    }

    pub(crate) fn seal_live_issue_decision(&mut self) {
        if let Some(delta) = self.live_issue.take_current_decision() {
            self.stats.record_issue_cycle(
                delta.new_cycle,
                delta.issued_rows,
                delta.resource_blocked_rows,
                delta.dependency_blocked_rows,
                delta.max_rows_at_tick,
            );
        }
    }

    pub(crate) fn service_live_issue_queue_at(
        &mut self,
        hart: &RiscvHartState,
        now: u64,
    ) -> Result<O3LiveIssueServiceOutcome, O3RuntimeError> {
        self.seal_live_issue_decision_before(now);
        if !self.live_issue.begin_service_at(now) {
            return Ok(O3LiveIssueServiceOutcome::default());
        }
        let queue = match O3LiveIssueQueue::materialize(self, self.live_issue.resident_sequences())?
        {
            O3LiveIssueQueueCapture::Ready(queue) => queue,
            O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                return self.finish_live_issue_replay_at(now, sequence, &[], 0, false, 0);
            }
        };
        if queue.entries().is_empty() {
            self.live_issue.clear_requested_service_tick();
            return Ok(O3LiveIssueServiceOutcome::default());
        }
        let dependencies = O3LiveIssueDependencyTable::new(self, queue.entries())?;
        let calendar = O3LiveIssueCalendar::capture(self);
        let plan = calendar.plan_at(now, &dependencies, queue.entries())?;
        let issued_sequences = plan
            .issued()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>();
        let max_rows_at_tick = plan.reserved_width().saturating_add(plan.issued().len());
        let prepared = self.prepare_live_issue_batch(hart, &queue, plan.issued(), now)?;
        let issued_rows = match prepared {
            O3PreparedLiveIssueBatch::Prepared(rows) => {
                match O3LiveIssueTransaction::record(self, rows) {
                    Ok(O3LiveIssueBatchOutcome::Recorded) => plan.issued().len(),
                    Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence)) => {
                        return self.finish_live_issue_replay_at(
                            now,
                            sequence,
                            &[],
                            0,
                            true,
                            max_rows_at_tick,
                        );
                    }
                    Err(O3LiveIssueTransactionError::Runtime(error)) => return Err(error),
                    Err(O3LiveIssueTransactionError::AlreadyActive) => {
                        unreachable!("live issue service cannot nest issue transactions")
                    }
                }
            }
            O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
                return self.finish_live_issue_replay_at(
                    now,
                    sequence,
                    &[],
                    0,
                    true,
                    max_rows_at_tick,
                );
            }
        };
        let post = self.classify_live_issue_queue_after_service(now)?;
        if let Some(sequence) = post.replay_boundary {
            return self.finish_live_issue_replay_at(
                now,
                sequence,
                &issued_sequences,
                issued_rows,
                true,
                max_rows_at_tick,
            );
        }
        self.finish_live_issue_service_at(
            now,
            &issued_sequences,
            issued_rows,
            None,
            true,
            max_rows_at_tick,
            post,
        )
    }

    fn finish_live_issue_replay_at(
        &mut self,
        now: u64,
        sequence: u64,
        issued_sequences: &[u64],
        issued_rows: usize,
        arbitrated: bool,
        max_rows_at_tick: usize,
    ) -> Result<O3LiveIssueServiceOutcome, O3RuntimeError> {
        self.discard_pending_data_address_from(sequence);
        let post = self.classify_live_issue_queue_after_service(now)?;
        if let Some(sequence) = post.replay_boundary {
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        self.finish_live_issue_service_at(
            now,
            issued_sequences,
            issued_rows,
            Some(sequence),
            arbitrated,
            max_rows_at_tick,
            post,
        )
    }

    fn finish_live_issue_service_at(
        &mut self,
        now: u64,
        issued_sequences: &[u64],
        issued_rows: usize,
        replay_boundary: Option<u64>,
        arbitrated: bool,
        max_rows_at_tick: usize,
        mut post: O3LiveIssuePostService,
    ) -> Result<O3LiveIssueServiceOutcome, O3RuntimeError> {
        post.max_rows_at_tick = post.max_rows_at_tick.max(max_rows_at_tick);
        if arbitrated || post.observed_decision {
            self.live_issue.observe_sequences(
                now,
                issued_sequences,
                &post.resource_blocked_sequences,
                &post.dependency_blocked_sequences,
                post.max_rows_at_tick,
            );
        }
        if let Some(tick) = post.next_service_tick {
            self.live_issue.request_service_at(tick);
        }
        if let Some(sequence) = post.no_wake_sequence {
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        Ok(O3LiveIssueServiceOutcome {
            issued_rows,
            next_service_tick: post.next_service_tick,
            replay_boundary,
            waits_for_pending_dependency: post.waits_for_pending_dependency,
        })
    }

    fn classify_live_issue_queue_after_service(
        &mut self,
        now: u64,
    ) -> Result<O3LiveIssuePostService, O3RuntimeError> {
        if self.live_issue.resident_sequences().is_empty() {
            self.live_issue.clear_requested_service_tick();
            return Ok(O3LiveIssuePostService::default());
        }
        let queue = match O3LiveIssueQueue::materialize(self, self.live_issue.resident_sequences())?
        {
            O3LiveIssueQueueCapture::Ready(queue) => queue,
            O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                return Ok(O3LiveIssuePostService {
                    replay_boundary: Some(sequence),
                    ..O3LiveIssuePostService::default()
                });
            }
        };
        if queue.entries().is_empty() {
            self.live_issue.clear_requested_service_tick();
            return Ok(O3LiveIssuePostService::default());
        }
        let dependency_table = O3LiveIssueDependencyTable::new(self, queue.entries())?;
        let calendar = O3LiveIssueCalendar::capture(self);
        let post_plan = calendar.plan_at(now, &dependency_table, queue.entries())?;
        let same_tick = (!post_plan.issued().is_empty()).then_some(now);
        let resource_tick = (!post_plan.resource_blocked().is_empty())
            .then(|| now.checked_add(1))
            .flatten();
        let dependency_tick =
            dependency_table.earliest_resolution_after(now, post_plan.dependency_blocked());
        let pending_tick = post_plan
            .resource_blocked()
            .iter()
            .find_map(|row| self.pending_data_address_sequence_for_replay(row.sequence()))
            .and_then(|sequence| {
                self.record_pending_data_address_resource_blocked(sequence, now);
                self.pending_data_address_wake_tick()
            });
        let next_service_tick = [same_tick, resource_tick, dependency_tick, pending_tick]
            .into_iter()
            .flatten()
            .min();
        let only_dependency_blocked =
            post_plan.issued().is_empty() && post_plan.resource_blocked().is_empty();
        let waits_for_pending_dependency = only_dependency_blocked
            && post_plan.dependency_blocked().iter().any(|row| {
                self.pending_data_address_sequence_for_replay(row.sequence())
                    .is_some()
                    || queue.entry(row.sequence()).is_some_and(|entry| {
                        entry.scheduling().data_producers().iter().any(|producer| {
                            self.pending_data_address_sequence_for_replay(producer.sequence())
                                .is_some()
                        })
                    })
            });
        let waits_for_live_data_dependency = only_dependency_blocked
            && post_plan.dependency_blocked().iter().any(|row| {
                queue.entry(row.sequence()).is_some_and(|entry| {
                    entry.scheduling().data_producers().iter().any(|producer| {
                        self.live_data_accesses
                            .iter()
                            .any(|live| live.sequence == producer.sequence())
                    })
                })
            });
        let waits_for_external_dependency =
            waits_for_pending_dependency || waits_for_live_data_dependency;
        let no_wake_sequence = (next_service_tick.is_none() && !waits_for_external_dependency)
            .then(|| self.live_issue.resident_sequences()[0]);
        Ok(O3LiveIssuePostService {
            observed_decision: true,
            resource_blocked_sequences: post_plan
                .resource_blocked()
                .iter()
                .map(O3ScopedReadyInstruction::sequence)
                .collect(),
            dependency_blocked_sequences: post_plan
                .dependency_blocked()
                .iter()
                .map(O3ScopedReadyInstruction::sequence)
                .collect(),
            max_rows_at_tick: post_plan
                .reserved_width()
                .saturating_add(post_plan.issued().len()),
            next_service_tick,
            replay_boundary: None,
            no_wake_sequence,
            waits_for_pending_dependency,
        })
    }

    pub(in crate::o3_runtime) fn prepare_live_issue_batch(
        &self,
        hart: &RiscvHartState,
        queue: &O3LiveIssueQueue,
        issued: &[O3ScopedReadyInstruction],
        issue_tick: u64,
    ) -> Result<O3PreparedLiveIssueBatch, O3RuntimeError> {
        let mut selected = Vec::with_capacity(issued.len());
        for issued in issued {
            let Some(entry) = queue.entry(issued.sequence()) else {
                return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                    sequence: issued.sequence(),
                });
            };
            selected.push(entry);
        }
        selected.sort_by_key(|entry| {
            (
                !entry.scheduling().is_pending_data_address(),
                entry.sequence(),
            )
        });

        let mut prepared = Vec::with_capacity(selected.len());
        for entry in selected {
            let packet = entry.packet();
            let Some(candidate) =
                self.materialize_live_speculative_issue_candidate(entry.scheduling())
            else {
                return if entry.scheduling().is_pending_data_address() {
                    Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()))
                } else {
                    Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                        sequence: entry.sequence(),
                    })
                };
            };
            let mut speculative_hart = hart.clone();
            for write in candidate.forwarded_register_writes() {
                speculative_hart.write(write.register(), write.value());
            }
            speculative_hart.set_pc(entry.scheduling().pc().get());
            let execution = match speculative_hart.execute_decoded(packet.decoded()) {
                Ok(execution) => execution,
                Err(_) if entry.scheduling().is_pending_data_address() => {
                    return Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()));
                }
                Err(_) => {
                    return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                        sequence: entry.sequence(),
                    });
                }
            };
            prepared.push(O3PreparedLiveIssue {
                candidate,
                consumed_requests: packet.consumed_requests().to_vec(),
                issue_tick,
                execution,
            });
        }
        Ok(O3PreparedLiveIssueBatch::Prepared(prepared))
    }

    pub(crate) fn schedule_live_speculative_issues(
        &mut self,
        hart: &RiscvHartState,
        head: O3LiveIssueHeadReservation,
        earliest_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        if !self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
            && !self.pending_data_address_has_producer_sequence(head.sequence())
        {
            return Ok(());
        }
        let start_tick = self
            .live_issue
            .service_floor_tick()
            .map_or(earliest_tick, |floor| earliest_tick.max(floor));
        self.live_issue.request_service_at(start_tick);
        let mut tick = start_tick;
        loop {
            let outcome = self.service_live_issue_queue_at(hart, tick)?;
            if outcome.replay_boundary().is_some() {
                break;
            }
            let Some(next_tick) = outcome.next_service_tick() else {
                break;
            };
            if outcome.waits_for_pending_dependency() && next_tick > start_tick {
                break;
            }
            if self.pending_data_address_wake_tick() == Some(next_tick) {
                break;
            }
            tick = next_tick;
        }
        if self.live_issue_is_quiescent() {
            self.seal_live_issue_decision();
        }
        Ok(())
    }
}
