use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, Tick, WaitForNode,
};

use crate::result_collect::collect_parallel_progress_transitions;

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn dma_scheduler_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.gpu_dma_scheduler_progress_transitions()
                .iter()
                .cloned()
                .chain(
                    self.accelerator_dma_scheduler_progress_transitions()
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn dma_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count_by_kind(self.dma_scheduler_progress_transitions(), kind)
    }

    pub fn dma_scheduler_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.dma_scheduler_progress_transitions()
                .into_iter()
                .filter(|transition| transition.kind() == kind),
        )
    }

    pub fn dma_scheduler_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(self.dma_scheduler_progress_transitions(), |record| {
            record.kind()
        })
    }

    pub fn dma_scheduler_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.dma_scheduler_progress_transitions())
    }

    pub fn dma_scheduler_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.dma_scheduler_progress_transitions(), |record| {
            record.kind() == kind
        })
    }

    pub fn dma_scheduler_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count_by_partition(self.dma_scheduler_progress_transitions(), partition)
    }

    pub fn dma_scheduler_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.dma_scheduler_progress_transitions()
                .into_iter()
                .filter(|transition| transition.partition() == partition),
        )
    }

    pub fn dma_scheduler_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(self.dma_scheduler_progress_transitions(), |record| {
            record.partition()
        })
    }

    pub fn dma_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(self.dma_scheduler_progress_transitions())
    }

    pub fn dma_scheduler_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.dma_scheduler_progress_transitions(), |record| {
            record.partition() == partition
        })
    }

    pub fn dma_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count_by_subject(self.dma_scheduler_progress_transitions(), subject)
    }

    pub fn dma_scheduler_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.dma_scheduler_progress_transitions()
                .into_iter()
                .filter(|transition| transition.subject() == subject),
        )
    }

    pub fn dma_scheduler_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(self.dma_scheduler_progress_transitions(), |record| {
            record.subject().clone()
        })
    }

    pub fn dma_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(self.dma_scheduler_progress_transitions())
    }

    pub fn dma_scheduler_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.dma_scheduler_progress_transitions(), |record| {
            record.subject() == subject
        })
    }

    pub fn has_dma_scheduler_progress_transitions(&self) -> bool {
        !self.dma_scheduler_progress_transitions().is_empty()
    }
}

fn progress_transition_count_by_kind(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    kind: LivelockTransitionKind,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.kind() == kind)
        .count()
}

fn progress_transition_count_by_partition(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    partition: PartitionId,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.partition() == partition)
        .count()
}

fn progress_transition_count_by_subject(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    subject: &WaitForNode,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| transition.subject() == subject)
        .count()
}

fn progress_transition_tick_window(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    mut predicate: impl FnMut(&ParallelProgressTransitionRecord) -> bool,
) -> Option<(Tick, Tick)> {
    let mut window: Option<(Tick, Tick)> = None;
    for transition in transitions {
        if predicate(&transition) {
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

fn collect_progress_transition_summaries<K>(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    mut key: impl FnMut(&ParallelProgressTransitionRecord) -> K,
) -> Vec<(K, usize, Tick, Tick)>
where
    K: Ord,
{
    let mut summaries = BTreeMap::<K, (usize, Tick, Tick)>::new();
    for transition in transitions {
        summaries
            .entry(key(&transition))
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

fn collect_progress_transition_kinds(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Vec<LivelockTransitionKind> {
    transitions
        .into_iter()
        .map(|transition| transition.kind())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_progress_transition_partitions(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Vec<PartitionId> {
    transitions
        .into_iter()
        .map(|transition| transition.partition())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_progress_transition_subjects(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Vec<WaitForNode> {
    transitions
        .into_iter()
        .map(|transition| transition.subject().clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
