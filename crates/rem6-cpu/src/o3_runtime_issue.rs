use std::collections::BTreeSet;

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
    pub(crate) fn record_live_issue_head_execution(
        &mut self,
        head: O3LiveIssueHeadReservation,
        consumed_requests: &[MemoryRequestId],
        execution: RiscvExecutionRecord,
    ) -> bool {
        if let Some(recorded) = self
            .live_speculative_executions
            .iter()
            .find(|recorded| recorded.sequence == head.sequence)
        {
            return recorded.consumed_requests == consumed_requests
                && recorded.issue_tick == head.issue_tick
                && recorded.execution == execution;
        }
        let Some(entry) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .find(|entry| entry.is_live_staged() && entry.sequence() == head.sequence)
        else {
            return false;
        };
        if !valid_live_speculative_fetch_identity(consumed_requests)
            || entry.pc() != Address::new(execution.pc())
            || live_issue_op_class(execution.instruction()) != head.op_class
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
            || staged_rename_entry(entry).is_some_and(|destination| {
                !execution_writes_rename_destination(&execution, destination)
            })
        {
            return false;
        }
        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: head.sequence,
                producer_sequences: Vec::new(),
                issue_tick: head.issue_tick,
                execution,
            });
        self.live_speculative_executions
            .sort_by_key(|recorded| recorded.sequence);
        true
    }

    pub(crate) fn schedule_live_speculative_issues(
        &mut self,
        hart: &RiscvHartState,
        head: O3LiveIssueHeadReservation,
        earliest_tick: u64,
        requests: &[O3LiveIssueRequest],
    ) {
        if !self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence)
        {
            return;
        }
        let mut tick = earliest_tick;
        loop {
            if requests
                .iter()
                .all(|request| self.live_issue_request_is_recorded(request))
            {
                break;
            }

            let mut candidate_sequences = BTreeSet::new();
            let candidates = requests
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
                .filter(|(_, candidate, _)| candidate_sequences.insert(candidate.sequence()))
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                break;
            }

            let reservations = self.live_issue_reservations_at(head, tick);
            if reservations.width >= self.issue_width {
                let resource_blocked_rows = candidates
                    .iter()
                    .filter(|(_, candidate, _)| candidate.issue_tick(tick) == tick)
                    .count();
                let dependency_blocked_rows =
                    candidates.len().saturating_sub(resource_blocked_rows);
                self.record_live_issue_decision(
                    tick,
                    0,
                    resource_blocked_rows,
                    dependency_blocked_rows,
                    reservations.width,
                );
                if resource_blocked_rows != 0 {
                    let next_tick = tick.saturating_add(1);
                    if next_tick == tick {
                        break;
                    }
                    tick = next_tick;
                } else if let Some(next_tick) = earliest_dependency_tick(&candidates, tick) {
                    tick = next_tick;
                } else {
                    break;
                }
                continue;
            }

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
            let resolved_scopes = candidates
                .iter()
                .flat_map(|(_, candidate, _)| candidate.data_dependencies())
                .filter(|dependency| dependency.ready_tick <= tick)
                .map(|dependency| O3DependencyScopeId::new(dependency.sequence));
            let ready = candidates.iter().map(|(_, candidate, op_class)| {
                O3ScopedReadyInstruction::new(candidate.sequence(), LIVE_ISSUE_QUEUE, *op_class)
                    .with_waits_on(
                        candidate
                            .data_dependencies()
                            .iter()
                            .map(|dependency| O3DependencyScopeId::new(dependency.sequence)),
                    )
                    .with_produces([O3DependencyScopeId::new(candidate.sequence())])
            });
            let plan = scheduler
                .try_plan(resolved_scopes, ready)
                .expect("live O3 issue candidates have unique producer scopes");
            let issued_sequences = plan.issued_sequences().collect::<BTreeSet<_>>();
            let resource_blocked_rows = plan.resource_blocked().len();
            let dependency_blocked_rows = plan.dependency_blocked().len();
            let dependency_blocked_sequences = plan
                .dependency_blocked()
                .iter()
                .map(O3ScopedReadyInstruction::sequence)
                .collect::<BTreeSet<_>>();

            let mut selected = candidates
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
                );
                if self.live_issue_request_is_recorded(request) {
                    recorded_sequences.insert(sequence);
                }
            }
            let recorded_rows = recorded_sequences.len();
            self.record_live_issue_decision(
                tick,
                recorded_rows,
                resource_blocked_rows,
                dependency_blocked_rows,
                reservations.width.saturating_add(recorded_rows),
            );
            if recorded_sequences != issued_sequences {
                break;
            }

            if resource_blocked_rows != 0 {
                let next_tick = tick.saturating_add(1);
                if next_tick == tick {
                    break;
                }
                tick = next_tick;
            } else if !dependency_blocked_sequences.is_empty() {
                let next_tick = requests
                    .iter()
                    .filter(|request| !self.live_issue_request_is_recorded(request))
                    .filter_map(|request| {
                        self.live_speculative_issue_candidate(request.pc, request.instruction())
                    })
                    .filter(|candidate| {
                        dependency_blocked_sequences.contains(&candidate.sequence())
                    })
                    .flat_map(|candidate| {
                        candidate
                            .data_dependencies()
                            .iter()
                            .map(|dependency| dependency.ready_tick)
                            .collect::<Vec<_>>()
                    })
                    .filter(|ready_tick| *ready_tick > tick)
                    .min();
                let Some(next_tick) = next_tick else {
                    break;
                };
                tick = next_tick;
            } else if issued_sequences.is_empty() {
                break;
            }
        }
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
    tick: u64,
) -> Option<u64> {
    candidates
        .iter()
        .flat_map(|(_, candidate, _)| candidate.data_dependencies())
        .map(|dependency| dependency.ready_tick)
        .filter(|ready_tick| *ready_tick > tick)
        .min()
}

fn live_issue_op_class(instruction: RiscvInstruction) -> O3IssueOpClass {
    if o3_direct_conditional_sources(instruction).is_some() {
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
