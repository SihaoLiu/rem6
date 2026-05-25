use std::collections::BTreeSet;

use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, WaitForNode,
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

    pub fn parallel_scheduler_progress_transitions(&self) -> &[ParallelProgressTransitionRecord] {
        &self.parallel_scheduler_progress_transitions
    }

    pub fn parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count_by_kind(&self.parallel_scheduler_progress_transitions, kind)
    }

    pub fn parallel_scheduler_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(&self.parallel_scheduler_progress_transitions)
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

    pub fn parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(&self.parallel_scheduler_progress_transitions)
    }

    pub fn parallel_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count_by_subject(&self.parallel_scheduler_progress_transitions, subject)
    }

    pub fn parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(&self.parallel_scheduler_progress_transitions)
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

    pub fn data_cache_parallel_scheduler_progress_transition_kinds(
        &self,
    ) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(&self.data_cache_parallel_scheduler_progress_transitions)
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

    pub fn data_cache_parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(
            &self.data_cache_parallel_scheduler_progress_transitions,
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

    pub fn data_cache_parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(
            &self.data_cache_parallel_scheduler_progress_transitions,
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

    pub fn full_system_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.full_system_progress_transition_iter())
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

    pub fn full_system_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(self.full_system_progress_transition_iter())
    }

    pub fn full_system_progress_transition_count_by_subject(&self, subject: &WaitForNode) -> usize {
        progress_transition_count_by_subject(self.full_system_progress_transition_iter(), subject)
    }

    pub fn full_system_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(self.full_system_progress_transition_iter())
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
