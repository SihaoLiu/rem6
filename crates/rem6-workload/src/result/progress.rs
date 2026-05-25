use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    LivelockDiagnostic, LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId,
    Tick, WaitForNode,
};

use super::WorkloadParallelExecutionSummary;
use crate::result_collect::collect_parallel_progress_transitions;

impl WorkloadParallelExecutionSummary {
    pub fn with_parallel_scheduler_progress_transitions(
        mut self,
        transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    ) -> Self {
        self.parallel_scheduler_progress_transitions =
            collect_parallel_progress_transitions(transitions);
        self.scheduler_progress_transition_count = self
            .scheduler_progress_transition_count
            .max(self.parallel_scheduler_progress_transitions.len());
        self
    }

    pub fn with_data_cache_parallel_scheduler_progress_transitions(
        mut self,
        transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_progress_transitions =
            collect_parallel_progress_transitions(transitions);
        self.data_cache_parallel_scheduler_progress_transition_count = self
            .data_cache_parallel_scheduler_progress_transition_count
            .max(
                self.data_cache_parallel_scheduler_progress_transitions
                    .len(),
            );
        self
    }

    pub fn with_parallel_scheduler_livelock_diagnostic_records(
        mut self,
        progress_transition_count: usize,
        diagnostics: impl IntoIterator<Item = LivelockDiagnostic>,
    ) -> Self {
        self.scheduler_progress_transition_count = self
            .scheduler_progress_transition_count
            .max(progress_transition_count);
        self.scheduler_livelock_diagnostics = diagnostics.into_iter().collect();
        self.scheduler_livelock_diagnostic_count = self
            .scheduler_livelock_diagnostic_count
            .max(self.scheduler_livelock_diagnostics.len());
        self
    }

    pub fn with_data_cache_parallel_scheduler_livelock_diagnostic_records(
        mut self,
        progress_transition_count: usize,
        diagnostics: impl IntoIterator<Item = LivelockDiagnostic>,
    ) -> Self {
        self.data_cache_parallel_scheduler_progress_transition_count = self
            .data_cache_parallel_scheduler_progress_transition_count
            .max(progress_transition_count);
        self.data_cache_parallel_scheduler_livelock_diagnostics = diagnostics.into_iter().collect();
        self.data_cache_parallel_scheduler_livelock_diagnostic_count = self
            .data_cache_parallel_scheduler_livelock_diagnostic_count
            .max(
                self.data_cache_parallel_scheduler_livelock_diagnostics
                    .len(),
            );
        self
    }

    pub fn with_full_system_livelock_diagnostic_records(
        mut self,
        diagnostics: impl IntoIterator<Item = LivelockDiagnostic>,
    ) -> Self {
        self.merged_full_system_livelock_diagnostics = diagnostics.into_iter().collect();
        self.merged_full_system_livelock_diagnostic_count =
            self.merged_full_system_livelock_diagnostics.len();
        self.has_merged_full_system_livelock_diagnostic_count = true;
        self
    }

    pub fn parallel_scheduler_progress_transitions(&self) -> &[ParallelProgressTransitionRecord] {
        &self.parallel_scheduler_progress_transitions
    }

    pub fn parallel_scheduler_livelock_diagnostics(&self) -> &[LivelockDiagnostic] {
        &self.scheduler_livelock_diagnostics
    }

    pub fn data_cache_parallel_scheduler_livelock_diagnostics(&self) -> &[LivelockDiagnostic] {
        &self.data_cache_parallel_scheduler_livelock_diagnostics
    }

    pub fn full_system_livelock_diagnostics(&self) -> Vec<LivelockDiagnostic> {
        if self.has_merged_full_system_livelock_diagnostic_count {
            return self.merged_full_system_livelock_diagnostics.clone();
        }
        self.scheduler_livelock_diagnostics
            .iter()
            .chain(&self.data_cache_parallel_scheduler_livelock_diagnostics)
            .cloned()
            .collect()
    }

    pub fn parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count_by_kind(&self.parallel_scheduler_progress_transitions, kind)
    }

    pub fn parallel_scheduler_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.kind() == kind,
        )
    }

    pub fn parallel_scheduler_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.kind(),
        )
    }

    pub fn parallel_scheduler_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(&self.parallel_scheduler_progress_transitions)
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.kind() == kind,
        )
    }

    pub fn parallel_scheduler_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count_by_partition(
            &self.parallel_scheduler_progress_transitions,
            partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.partition() == partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.partition(),
        )
    }

    pub fn parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(&self.parallel_scheduler_progress_transitions)
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.partition() == partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count_by_subject(&self.parallel_scheduler_progress_transitions, subject)
    }

    pub fn parallel_scheduler_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.subject() == subject,
        )
    }

    pub fn parallel_scheduler_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.subject().clone(),
        )
    }

    pub fn parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(&self.parallel_scheduler_progress_transitions)
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.parallel_scheduler_progress_transitions,
            |transition| transition.subject() == subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transitions(
        &self,
    ) -> &[ParallelProgressTransitionRecord] {
        &self.data_cache_parallel_scheduler_progress_transitions
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count_by_kind(
            &self.data_cache_parallel_scheduler_progress_transitions,
            kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.kind() == kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.kind(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_kinds(
        &self,
    ) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(&self.data_cache_parallel_scheduler_progress_transitions)
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.kind() == kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count_by_partition(
            &self.data_cache_parallel_scheduler_progress_transitions,
            partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.partition() == partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.partition(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(
            &self.data_cache_parallel_scheduler_progress_transitions,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.partition() == partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count_by_subject(
            &self.data_cache_parallel_scheduler_progress_transitions,
            subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.subject() == subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.subject().clone(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(
            &self.data_cache_parallel_scheduler_progress_transitions,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            &self.data_cache_parallel_scheduler_progress_transitions,
            |transition| transition.subject() == subject,
        )
    }

    pub fn full_system_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.parallel_scheduler_progress_transitions
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_progress_transitions
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn full_system_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count_by_kind(self.full_system_progress_transition_iter(), kind)
    }

    pub fn full_system_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.full_system_progress_transition_iter(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn full_system_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transition_iter(),
            |transition| transition.kind(),
        )
    }

    pub fn full_system_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.full_system_progress_transition_iter())
    }

    pub fn full_system_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transition_iter(), |transition| {
            transition.kind() == kind
        })
    }

    pub fn full_system_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count_by_partition(
            self.full_system_progress_transition_iter(),
            partition,
        )
    }

    pub fn full_system_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.full_system_progress_transition_iter(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn full_system_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transition_iter(),
            |transition| transition.partition(),
        )
    }

    pub fn full_system_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(self.full_system_progress_transition_iter())
    }

    pub fn full_system_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transition_iter(), |transition| {
            transition.partition() == partition
        })
    }

    pub fn full_system_progress_transition_count_by_subject(&self, subject: &WaitForNode) -> usize {
        progress_transition_count_by_subject(self.full_system_progress_transition_iter(), subject)
    }

    pub fn full_system_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.full_system_progress_transition_iter(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn full_system_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transition_iter(),
            |transition| transition.subject().clone(),
        )
    }

    pub fn full_system_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(self.full_system_progress_transition_iter())
    }

    pub fn full_system_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transition_iter(), |transition| {
            transition.subject() == subject
        })
    }

    pub fn has_parallel_scheduler_progress_transitions(&self) -> bool {
        !self.parallel_scheduler_progress_transitions.is_empty()
    }

    pub fn has_data_cache_parallel_scheduler_progress_transitions(&self) -> bool {
        !self
            .data_cache_parallel_scheduler_progress_transitions
            .is_empty()
    }

    pub fn has_full_system_progress_transitions(&self) -> bool {
        self.has_parallel_scheduler_progress_transitions()
            || self.has_data_cache_parallel_scheduler_progress_transitions()
    }

    fn full_system_progress_transition_iter(
        &self,
    ) -> impl Iterator<Item = &ParallelProgressTransitionRecord> {
        self.parallel_scheduler_progress_transitions.iter().chain(
            self.data_cache_parallel_scheduler_progress_transitions
                .iter(),
        )
    }
}

fn progress_transition_count_by_kind<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    kind: LivelockTransitionKind,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.kind() == kind)
        .count()
}

fn progress_transition_count_by_partition<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    partition: PartitionId,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.partition() == partition)
        .count()
}

fn progress_transition_count_by_subject<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    subject: &WaitForNode,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.subject() == subject)
        .count()
}

fn progress_transition_tick_window<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    mut predicate: impl FnMut(&ParallelProgressTransitionRecord) -> bool,
) -> Option<(Tick, Tick)> {
    let mut window: Option<(Tick, Tick)> = None;
    for transition in transitions {
        if predicate(transition) {
            window = Some(match window {
                Some((first_tick, last_tick)) => (
                    first_tick.min(transition.tick()),
                    last_tick.max(transition.tick()),
                ),
                None => (transition.tick(), transition.tick()),
            });
        }
    }
    window
}

fn collect_progress_transition_records<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    mut predicate: impl FnMut(&ParallelProgressTransitionRecord) -> bool,
) -> Vec<ParallelProgressTransitionRecord> {
    collect_parallel_progress_transitions(
        transitions
            .into_iter()
            .filter(|transition| predicate(transition))
            .cloned(),
    )
}

fn collect_progress_transition_summaries<'a, K>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
    mut key: impl FnMut(&ParallelProgressTransitionRecord) -> K,
) -> Vec<(K, usize, Tick, Tick)>
where
    K: Ord,
{
    let mut summaries = BTreeMap::<K, (usize, Tick, Tick)>::new();
    for transition in transitions {
        summaries
            .entry(key(transition))
            .and_modify(|summary| {
                summary.0 += 1;
                summary.1 = summary.1.min(transition.tick());
                summary.2 = summary.2.max(transition.tick());
            })
            .or_insert((1, transition.tick(), transition.tick()));
    }
    summaries
        .into_iter()
        .map(|(dimension, (count, first_tick, last_tick))| {
            (dimension, count, first_tick, last_tick)
        })
        .collect()
}

fn collect_progress_transition_kinds<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
) -> Vec<LivelockTransitionKind> {
    transitions
        .into_iter()
        .map(ParallelProgressTransitionRecord::kind)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_progress_transition_partitions<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
) -> Vec<PartitionId> {
    transitions
        .into_iter()
        .map(ParallelProgressTransitionRecord::partition)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_progress_transition_subjects<'a>(
    transitions: impl IntoIterator<Item = &'a ParallelProgressTransitionRecord>,
) -> Vec<WaitForNode> {
    transitions
        .into_iter()
        .map(|transition| transition.subject().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
