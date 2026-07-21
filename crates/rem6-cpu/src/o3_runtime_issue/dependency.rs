use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::o3_pipeline::{O3DependencyScopeId, O3ScopedReadyInstruction};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum O3LiveIssueDependencyKey {
    Data(u64),
    Control(u64),
}

pub(crate) struct O3LiveIssueDependencyTable {
    scopes: BTreeMap<O3LiveIssueDependencyKey, O3DependencyScopeId>,
    ready_ticks: BTreeMap<O3DependencyScopeId, u64>,
    control_dependencies: BTreeMap<u64, u64>,
}

impl O3LiveIssueDependencyTable {
    pub(crate) fn new(
        runtime: &O3RuntimeState,
        candidates: &[O3LiveIssueSchedulingCandidate],
    ) -> Result<Self, O3RuntimeError> {
        let known_sequences = runtime
            .live_speculative_executions
            .iter()
            .map(|row| row.sequence)
            .chain(
                candidates
                    .iter()
                    .map(O3LiveIssueSchedulingCandidate::sequence),
            )
            .collect::<BTreeSet<_>>();
        let control_dependencies = candidates
            .iter()
            .filter_map(|candidate| {
                let dependency = candidate.control_dependency()?;
                (runtime
                    .live_serializing_control_sequences
                    .contains(&dependency)
                    || !known_sequences.contains(&dependency))
                .then_some((candidate.sequence(), dependency))
            })
            .collect::<BTreeMap<_, _>>();
        let mut keys = BTreeSet::new();
        for candidate in candidates {
            keys.insert(O3LiveIssueDependencyKey::Data(candidate.sequence()));
            keys.insert(O3LiveIssueDependencyKey::Control(candidate.sequence()));
            keys.extend(
                candidate
                    .data_producers()
                    .iter()
                    .map(|producer| O3LiveIssueDependencyKey::Data(producer.sequence())),
            );
            if let Some(sequence) = control_dependencies.get(&candidate.sequence()).copied() {
                keys.insert(O3LiveIssueDependencyKey::Control(sequence));
            }
        }
        let scopes = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, O3DependencyScopeId::new(index as u64 + 1)))
            .collect::<BTreeMap<_, _>>();
        let mut ready_ticks = BTreeMap::new();
        for (key, scope) in &scopes {
            let ready_tick = match *key {
                O3LiveIssueDependencyKey::Data(sequence) => data_ready_tick(runtime, sequence),
                O3LiveIssueDependencyKey::Control(sequence) => {
                    control_ready_tick(runtime, sequence)?
                }
            };
            if let Some(ready_tick) = ready_tick {
                ready_ticks.insert(*scope, ready_tick);
            }
        }
        Ok(Self {
            scopes,
            ready_ticks,
            control_dependencies,
        })
    }

    pub(crate) fn scoped_instruction(
        &self,
        candidate: &O3LiveIssueSchedulingCandidate,
    ) -> O3ScopedReadyInstruction {
        let produces = [
            self.scope(O3LiveIssueDependencyKey::Data(candidate.sequence())),
            self.scope(O3LiveIssueDependencyKey::Control(candidate.sequence())),
        ];
        let waits_on = candidate
            .data_producers()
            .iter()
            .map(|producer| self.scope(O3LiveIssueDependencyKey::Data(producer.sequence())))
            .chain(
                self.control_dependencies
                    .get(&candidate.sequence())
                    .copied()
                    .map(|sequence| self.scope(O3LiveIssueDependencyKey::Control(sequence))),
            );
        O3ScopedReadyInstruction::new(candidate.sequence(), LIVE_ISSUE_QUEUE, candidate.op_class())
            .with_waits_on(waits_on)
            .with_produces(produces)
    }

    pub(crate) fn resolved_scopes_at(&self, tick: u64) -> BTreeSet<O3DependencyScopeId> {
        self.ready_ticks
            .iter()
            .filter_map(|(scope, ready_tick)| (*ready_tick <= tick).then_some(*scope))
            .collect()
    }

    pub(crate) fn earliest_resolution_after<'a>(
        &self,
        tick: u64,
        blocked: impl IntoIterator<Item = &'a O3ScopedReadyInstruction>,
    ) -> Option<u64> {
        blocked
            .into_iter()
            .flat_map(O3ScopedReadyInstruction::waits_on)
            .filter_map(|scope| self.ready_ticks.get(scope).copied())
            .filter(|ready_tick| *ready_tick > tick)
            .min()
    }

    fn scope(&self, key: O3LiveIssueDependencyKey) -> O3DependencyScopeId {
        self.scopes[&key]
    }
}

fn data_ready_tick(runtime: &O3RuntimeState, sequence: u64) -> Option<u64> {
    runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == sequence)
        .map(|row| row.admitted_writeback_tick)
        .or_else(|| runtime.completed_live_data_access_ready_tick(sequence))
        .or_else(|| runtime.pending_data_address_producer_ready_tick(sequence))
}

fn control_ready_tick(
    runtime: &O3RuntimeState,
    sequence: u64,
) -> Result<Option<u64>, O3RuntimeError> {
    runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == sequence)
        .map(|row| {
            row.admitted_writeback_tick.checked_add(1).ok_or(
                O3RuntimeError::WritebackTickOverflow {
                    tick: row.admitted_writeback_tick,
                },
            )
        })
        .transpose()
}
