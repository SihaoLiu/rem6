use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::{
    RiscvDecodedInstruction, RiscvExecutionRecord, RiscvHartState, RiscvInstruction,
};

use super::*;
use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueCapacity, O3IssueQueueId,
    O3ScopedIssueScheduler, O3ScopedReadyInstruction,
};
use crate::O3RuntimeFuLatencyClass;

const LIVE_ISSUE_QUEUE: O3IssueQueueId = O3IssueQueueId::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueRequest {
    pc: Address,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
}

impl O3LiveIssueRequest {
    pub(crate) fn new(
        pc: Address,
        consumed_requests: Vec<MemoryRequestId>,
        decoded: RiscvDecodedInstruction,
    ) -> Self {
        Self {
            pc,
            consumed_requests,
            decoded,
        }
    }

    const fn instruction(&self) -> RiscvInstruction {
        self.decoded.instruction()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueHeadReservation {
    sequence: u64,
    issue_tick: u64,
    op_class: O3IssueOpClass,
}

impl O3LiveIssueHeadReservation {
    pub(crate) fn for_instruction(
        sequence: u64,
        issue_tick: u64,
        instruction: RiscvInstruction,
    ) -> Self {
        Self {
            sequence,
            issue_tick,
            op_class: live_issue_op_class(instruction),
        }
    }

    pub(super) const fn memory(sequence: u64, issue_tick: u64) -> Self {
        Self {
            sequence,
            issue_tick,
            op_class: O3IssueOpClass::Memory,
        }
    }
}

impl O3RuntimeState {
    pub(crate) fn live_scalar_memory_head_reservation(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<O3LiveIssueHeadReservation> {
        self.live_data_accesses
            .iter()
            .find(|live| live.fetch_request == fetch_request)
            .map(|live| O3LiveIssueHeadReservation::memory(live.sequence, live.issue_tick))
    }

    pub(crate) fn record_live_issue_head_execution(
        &mut self,
        head: O3LiveIssueHeadReservation,
        consumed_requests: &[MemoryRequestId],
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        if !self.live_staged_fetch_identity_matches(
            head.sequence,
            execution.instruction(),
            consumed_requests,
        ) {
            return Ok(false);
        }
        if let Some(recorded) = self
            .live_speculative_executions
            .iter()
            .find(|recorded| recorded.sequence == head.sequence)
        {
            return Ok(recorded.consumed_requests == consumed_requests
                && recorded.issue_tick == head.issue_tick
                && recorded.execution == execution);
        }
        let Some(entry) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .find(|entry| entry.is_live_staged() && entry.sequence() == head.sequence)
        else {
            return Ok(false);
        };
        if entry.pc() != Address::new(execution.pc())
            || live_issue_op_class(execution.instruction()) != head.op_class
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
            || staged_rename_entry(entry).is_some_and(|destination| {
                !execution_writes_rename_destination(&execution, destination)
            })
        {
            return Ok(false);
        }
        let raw_ready_tick = head
            .issue_tick
            .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                execution.instruction(),
            ))
            .ok_or(O3RuntimeError::WritebackTickOverflow {
                tick: head.issue_tick,
            })?;
        let consumes_writeback_slot = staged_rename_entry(entry).is_some();
        let (admitted_writeback_tick, writeback_slot) = self.reserve_fixed_fu_writeback(
            head.sequence,
            raw_ready_tick,
            consumes_writeback_slot,
        )?;
        if let Some(entry) = self
            .snapshot
            .reorder_buffer
            .iter_mut()
            .find(|entry| entry.is_live_staged() && entry.sequence() == head.sequence)
        {
            entry.mark_ready_at(admitted_writeback_tick);
        }
        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: head.sequence,
                producer_sequences: Vec::new(),
                issue_tick: head.issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
                writeback_slot,
                execution,
            });
        self.live_speculative_executions
            .sort_by_key(|recorded| recorded.sequence);
        Ok(true)
    }

    pub(crate) fn schedule_live_speculative_issues(
        &mut self,
        hart: &RiscvHartState,
        head: O3LiveIssueHeadReservation,
        earliest_tick: u64,
        requests: &[O3LiveIssueRequest],
    ) -> Result<(), O3RuntimeError> {
        if !self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence)
        {
            return Ok(());
        }
        let mut tick = earliest_tick;
        let mut tick_decision = O3LiveIssueTickDecision::default();
        loop {
            if requests
                .iter()
                .all(|request| self.live_issue_request_is_recorded(request))
            {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }

            let candidates = self.live_issue_candidates(requests);
            if candidates.is_empty() {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }
            let serializing_controls = self.live_issue_serializing_controls(&candidates)?;

            let reservations = self.live_issue_reservations_at(head, tick);
            if reservations.width >= self.issue_width {
                let resource_blocked_rows = candidates
                    .iter()
                    .filter(|(_, candidate, _)| {
                        live_issue_dependencies_ready_at(candidate, &serializing_controls, tick)
                    })
                    .count();
                let dependency_blocked_rows =
                    candidates.len().saturating_sub(resource_blocked_rows);
                tick_decision.observe(
                    0,
                    resource_blocked_rows,
                    dependency_blocked_rows,
                    reservations.width,
                );
                if resource_blocked_rows != 0 {
                    let next_tick = tick.saturating_add(1);
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    if next_tick == tick {
                        break;
                    }
                    tick = next_tick;
                } else if let Some(next_tick) =
                    earliest_dependency_tick(&candidates, &serializing_controls, tick)
                {
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    tick = next_tick;
                } else {
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                }
                continue;
            }

            let (dependency_ready_candidates, dependency_blocked_candidates): (Vec<_>, Vec<_>) =
                candidates.into_iter().partition(|(_, candidate, _)| {
                    live_issue_dependencies_ready_at(candidate, &serializing_controls, tick)
                });
            let dependency_blocked_rows = dependency_blocked_candidates.len();
            let dependency_blocked_sequences = dependency_blocked_candidates
                .iter()
                .map(|(_, candidate, _)| candidate.sequence())
                .collect::<BTreeSet<_>>();
            let remaining_width = self.issue_width - reservations.width;
            let scheduler = O3ScopedIssueScheduler::new(
                remaining_width,
                [
                    (
                        O3IssueOpClass::IntAlu,
                        self.issue_width.saturating_sub(reservations.int_alu),
                    ),
                    (
                        O3IssueOpClass::IntMult,
                        1_usize.saturating_sub(reservations.int_mult),
                    ),
                    (
                        O3IssueOpClass::Branch,
                        1_usize.saturating_sub(reservations.branch),
                    ),
                ]
                .into_iter()
                .filter(|(_, slots)| *slots != 0)
                .map(|(op_class, slots)| {
                    O3IssueQueueCapacity::new(LIVE_ISSUE_QUEUE, op_class, slots)
                        .expect("live O3 issue capacities are nonzero")
                }),
            )
            .expect("configured live O3 issue width is nonzero");
            let ready = dependency_ready_candidates
                .iter()
                .map(|(_, candidate, op_class)| {
                    O3ScopedReadyInstruction::new(candidate.sequence(), LIVE_ISSUE_QUEUE, *op_class)
                        .with_produces([O3DependencyScopeId::new(candidate.sequence())])
                });
            let plan = scheduler
                .try_plan(std::iter::empty::<O3DependencyScopeId>(), ready)
                .expect("live O3 issue candidates have unique producer scopes");
            let issued_sequences = plan.issued_sequences().collect::<BTreeSet<_>>();
            let resource_blocked_rows = plan.resource_blocked().len();

            let mut selected = dependency_ready_candidates
                .into_iter()
                .filter(|(_, candidate, _)| issued_sequences.contains(&candidate.sequence()))
                .collect::<Vec<_>>();
            selected.sort_by_key(|(_, candidate, _)| candidate.sequence());
            let mut recorded_sequences = BTreeSet::new();
            for (request_index, candidate, _) in selected {
                let sequence = candidate.sequence();
                let request = &requests[request_index];
                let mut speculative_hart = hart.clone();
                for write in candidate.forwarded_register_writes() {
                    speculative_hart.write(write.register(), write.value());
                }
                speculative_hart.set_pc(request.pc.get());
                let Ok(execution) = speculative_hart.execute_decoded(request.decoded) else {
                    continue;
                };
                self.record_live_speculative_execution(
                    candidate,
                    &request.consumed_requests,
                    tick,
                    execution,
                )?;
                if self.live_issue_request_is_recorded(request) {
                    recorded_sequences.insert(sequence);
                }
            }
            let recorded_rows = recorded_sequences.len();
            tick_decision.observe(
                recorded_rows,
                resource_blocked_rows,
                dependency_blocked_rows,
                reservations.width.saturating_add(recorded_rows),
            );
            if recorded_sequences != issued_sequences {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }

            if resource_blocked_rows != 0 {
                let next_tick = tick.saturating_add(1);
                self.flush_live_issue_decision(tick, &mut tick_decision);
                if next_tick == tick {
                    break;
                }
                tick = next_tick;
            } else if !dependency_blocked_sequences.is_empty() {
                let remaining_candidates = self.live_issue_candidates(requests);
                let remaining_serializing_controls =
                    self.live_issue_serializing_controls(&remaining_candidates)?;
                if remaining_candidates.iter().any(|(_, candidate, _)| {
                    !dependency_blocked_sequences.contains(&candidate.sequence())
                        && live_issue_dependencies_ready_at(
                            candidate,
                            &remaining_serializing_controls,
                            tick,
                        )
                }) {
                    continue;
                }
                let next_tick = remaining_candidates
                    .iter()
                    .filter(|(_, candidate, _)| {
                        dependency_blocked_sequences.contains(&candidate.sequence())
                    })
                    .filter_map(|(_, candidate, _)| {
                        live_issue_dependency_readiness(candidate, &remaining_serializing_controls)
                            .ready_tick()
                    })
                    .filter(|ready_tick| *ready_tick > tick)
                    .min();
                let Some(next_tick) = next_tick else {
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                };
                self.flush_live_issue_decision(tick, &mut tick_decision);
                tick = next_tick;
            } else if issued_sequences.is_empty() {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }
        }
        Ok(())
    }

    fn live_issue_candidates(
        &self,
        requests: &[O3LiveIssueRequest],
    ) -> Vec<(usize, O3LiveSpeculativeIssueCandidate, O3IssueOpClass)> {
        let mut candidate_sequences = BTreeSet::new();
        requests
            .iter()
            .enumerate()
            .filter(|(_, request)| !self.live_issue_request_is_recorded(request))
            .filter_map(|(index, request)| {
                self.live_speculative_issue_candidate(request.pc, request.instruction())
                    .map(|candidate| {
                        let op_class = live_issue_op_class(candidate.instruction());
                        (index, candidate, op_class)
                    })
            })
            .filter(|(index, candidate, _)| {
                let request = &requests[*index];
                self.live_staged_fetch_identity_matches(
                    candidate.sequence(),
                    candidate.instruction(),
                    &request.consumed_requests,
                )
            })
            .filter(|(_, candidate, _)| candidate_sequences.insert(candidate.sequence()))
            .collect()
    }

    fn live_issue_serializing_controls(
        &self,
        candidates: &[(usize, O3LiveSpeculativeIssueCandidate, O3IssueOpClass)],
    ) -> Result<BTreeMap<u64, O3LiveIssueDependencyReadiness>, O3RuntimeError> {
        let mut controls = BTreeMap::new();
        for sequence in &self.live_serializing_control_sequences {
            let readiness = if let Some(execution) = self
                .live_speculative_executions
                .iter()
                .find(|execution| execution.sequence == *sequence)
            {
                let ready_tick = execution.admitted_writeback_tick.checked_add(1).ok_or(
                    O3RuntimeError::WritebackTickOverflow {
                        tick: execution.admitted_writeback_tick,
                    },
                )?;
                O3LiveIssueDependencyReadiness::ReadyAt(ready_tick)
            } else {
                O3LiveIssueDependencyReadiness::Unresolved
            };
            controls.insert(*sequence, readiness);
        }
        let known_sequences = self
            .live_speculative_executions
            .iter()
            .map(|execution| execution.sequence)
            .chain(
                candidates
                    .iter()
                    .map(|(_, candidate, _)| candidate.sequence()),
            )
            .collect::<BTreeSet<_>>();
        for (_, candidate, _) in candidates {
            if let Some(control_sequence) = candidate.control_dependency() {
                if !known_sequences.contains(&control_sequence) {
                    controls
                        .entry(control_sequence)
                        .or_insert(O3LiveIssueDependencyReadiness::Unresolved);
                }
            }
        }
        Ok(controls)
    }

    fn live_issue_request_is_recorded(&self, request: &O3LiveIssueRequest) -> bool {
        self.live_speculative_executions.iter().any(|issued| {
            issued.consumed_requests == request.consumed_requests
                && issued.execution.pc() == request.pc.get()
                && issued.execution.instruction() == request.instruction()
        })
    }

    fn record_live_issue_decision(
        &mut self,
        tick: u64,
        issued_rows: usize,
        resource_blocked_rows: usize,
        dependency_blocked_rows: usize,
        total_rows_at_tick: usize,
    ) {
        let new_cycle = self.live_issue_cycle_ticks.insert(tick);
        self.stats.record_issue_cycle(
            new_cycle,
            issued_rows,
            resource_blocked_rows,
            dependency_blocked_rows,
            total_rows_at_tick,
        );
    }

    fn flush_live_issue_decision(&mut self, tick: u64, decision: &mut O3LiveIssueTickDecision) {
        if !decision.observed {
            return;
        }
        let decision = std::mem::take(decision);
        self.record_live_issue_decision(
            tick,
            decision.issued_rows,
            decision.resource_blocked_rows,
            decision.dependency_blocked_rows,
            decision.max_rows_at_tick,
        );
    }

    fn live_issue_reservations_at(
        &self,
        head: O3LiveIssueHeadReservation,
        tick: u64,
    ) -> O3LiveIssueReservations {
        let mut reservations = O3LiveIssueReservations::default();
        if head.issue_tick == tick {
            reservations.reserve(head.op_class);
        }
        for issued in self.live_speculative_executions.iter().filter(|issued| {
            issued.issue_tick == tick
                && issued.sequence != head.sequence
                && self
                    .snapshot
                    .reorder_buffer
                    .iter()
                    .any(|entry| entry.is_live_staged() && entry.sequence() == issued.sequence)
        }) {
            reservations.reserve(live_issue_op_class(issued.execution.instruction()));
        }
        reservations
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueReservations {
    width: usize,
    int_alu: usize,
    int_mult: usize,
    branch: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueTickDecision {
    issued_rows: usize,
    resource_blocked_rows: usize,
    dependency_blocked_rows: usize,
    max_rows_at_tick: usize,
    observed: bool,
}

impl O3LiveIssueTickDecision {
    fn observe(
        &mut self,
        issued_rows: usize,
        resource_blocked_rows: usize,
        dependency_blocked_rows: usize,
        total_rows_at_tick: usize,
    ) {
        self.issued_rows = self.issued_rows.saturating_add(issued_rows);
        self.resource_blocked_rows = resource_blocked_rows;
        self.dependency_blocked_rows = dependency_blocked_rows;
        self.max_rows_at_tick = self.max_rows_at_tick.max(total_rows_at_tick);
        self.observed = true;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveIssueDependencyReadiness {
    Unresolved,
    ReadyAt(u64),
}

impl O3LiveIssueDependencyReadiness {
    const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unresolved, _) | (_, Self::Unresolved) => Self::Unresolved,
            (Self::ReadyAt(left), Self::ReadyAt(right)) => {
                Self::ReadyAt(if left > right { left } else { right })
            }
        }
    }

    const fn is_ready_at(self, tick: u64) -> bool {
        matches!(self, Self::ReadyAt(ready_tick) if ready_tick <= tick)
    }

    const fn ready_tick(self) -> Option<u64> {
        match self {
            Self::Unresolved => None,
            Self::ReadyAt(ready_tick) => Some(ready_tick),
        }
    }
}

impl O3LiveIssueReservations {
    fn reserve(&mut self, op_class: O3IssueOpClass) {
        self.width = self.width.saturating_add(1);
        match op_class {
            O3IssueOpClass::IntAlu => self.int_alu = self.int_alu.saturating_add(1),
            O3IssueOpClass::IntMult => self.int_mult = self.int_mult.saturating_add(1),
            O3IssueOpClass::Branch => self.branch = self.branch.saturating_add(1),
            O3IssueOpClass::Float | O3IssueOpClass::Memory | O3IssueOpClass::System => {}
        }
    }
}

fn earliest_dependency_tick(
    candidates: &[(usize, O3LiveSpeculativeIssueCandidate, O3IssueOpClass)],
    serializing_controls: &BTreeMap<u64, O3LiveIssueDependencyReadiness>,
    tick: u64,
) -> Option<u64> {
    candidates
        .iter()
        .filter_map(|(_, candidate, _)| {
            live_issue_dependency_readiness(candidate, serializing_controls).ready_tick()
        })
        .filter(|ready_tick| *ready_tick > tick)
        .min()
}

fn live_issue_dependencies_ready_at(
    candidate: &O3LiveSpeculativeIssueCandidate,
    serializing_controls: &BTreeMap<u64, O3LiveIssueDependencyReadiness>,
    tick: u64,
) -> bool {
    live_issue_dependency_readiness(candidate, serializing_controls).is_ready_at(tick)
}

fn live_issue_dependency_readiness(
    candidate: &O3LiveSpeculativeIssueCandidate,
    serializing_controls: &BTreeMap<u64, O3LiveIssueDependencyReadiness>,
) -> O3LiveIssueDependencyReadiness {
    let data_readiness = candidate.data_dependencies().iter().fold(
        O3LiveIssueDependencyReadiness::ReadyAt(0),
        |readiness, dependency| {
            readiness.merge(O3LiveIssueDependencyReadiness::ReadyAt(
                dependency.ready_tick,
            ))
        },
    );
    candidate
        .control_dependency()
        .and_then(|control_sequence| serializing_controls.get(&control_sequence).copied())
        .map_or(data_readiness, |control_readiness| {
            data_readiness.merge(control_readiness)
        })
}

fn live_issue_op_class(instruction: RiscvInstruction) -> O3IssueOpClass {
    if o3_live_control_operands(instruction).is_some() {
        return O3IssueOpClass::Branch;
    }
    if matches!(
        o3_fu_latency_class(instruction),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    ) {
        O3IssueOpClass::IntMult
    } else {
        O3IssueOpClass::IntAlu
    }
}
