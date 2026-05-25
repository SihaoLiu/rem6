use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    LivelockDiagnostic, LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId,
    ProgressMonitor, ProgressMonitorError, Tick, WaitForNode,
};

use crate::RiscvSystemRun;

pub type LivelockDiagnosticTransitionKindWindowSummary =
    (LivelockTransitionKind, usize, u64, Tick, Tick);

impl RiscvSystemRun {
    pub fn parallel_scheduler_progress_transition_count(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| {
                epoch
                    .batches()
                    .iter()
                    .map(|batch| batch.progress_transition_count())
                    .sum::<usize>()
            })
            .sum()
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.progress_transition_count())
            .sum()
    }

    pub fn full_system_progress_transition_count(&self) -> usize {
        self.parallel_scheduler_progress_transition_count()
            + self.data_cache_parallel_scheduler_progress_transition_count()
    }

    pub fn parallel_scheduler_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| {
                    epoch
                        .batches()
                        .iter()
                        .flat_map(|batch| batch.progress_transitions().iter().cloned())
                }),
        )
    }

    pub fn parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn parallel_scheduler_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn parallel_scheduler_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.kind(),
        )
    }

    pub fn parallel_scheduler_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.parallel_scheduler_progress_transitions())
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn parallel_scheduler_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.partition(),
        )
    }

    pub fn parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(self.parallel_scheduler_progress_transitions())
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn parallel_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn parallel_scheduler_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn parallel_scheduler_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.subject().clone(),
        )
    }

    pub fn parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(self.parallel_scheduler_progress_transitions())
    }

    pub fn parallel_scheduler_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transitions(
        &self,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.data_cache_parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.progress_transitions()),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.kind(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_kinds(
        &self,
    ) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.data_cache_parallel_scheduler_progress_transitions())
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.kind() == kind,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.partition(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(
            self.data_cache_parallel_scheduler_progress_transitions(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.partition() == partition,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_count_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> usize {
        progress_transition_count(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.subject().clone(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(
            self.data_cache_parallel_scheduler_progress_transitions(),
        )
    }

    pub fn data_cache_parallel_scheduler_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(
            self.data_cache_parallel_scheduler_progress_transitions(),
            |transition| transition.subject() == subject,
        )
    }

    pub fn full_system_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.parallel_scheduler_progress_transitions()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_progress_transitions()),
        )
    }

    pub fn full_system_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        progress_transition_count(self.full_system_progress_transitions(), |transition| {
            transition.kind() == kind
        })
    }

    pub fn full_system_progress_transition_records_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(self.full_system_progress_transitions(), |transition| {
            transition.kind() == kind
        })
    }

    pub fn full_system_progress_transition_kind_summaries(
        &self,
    ) -> Vec<(LivelockTransitionKind, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transitions(),
            |transition| transition.kind(),
        )
    }

    pub fn full_system_progress_transition_kinds(&self) -> Vec<LivelockTransitionKind> {
        collect_progress_transition_kinds(self.full_system_progress_transitions())
    }

    pub fn full_system_progress_transition_tick_window_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transitions(), |transition| {
            transition.kind() == kind
        })
    }

    pub fn full_system_progress_transition_count_by_partition(
        &self,
        partition: PartitionId,
    ) -> usize {
        progress_transition_count(self.full_system_progress_transitions(), |transition| {
            transition.partition() == partition
        })
    }

    pub fn full_system_progress_transition_records_by_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(self.full_system_progress_transitions(), |transition| {
            transition.partition() == partition
        })
    }

    pub fn full_system_progress_transition_partition_summaries(
        &self,
    ) -> Vec<(PartitionId, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transitions(),
            |transition| transition.partition(),
        )
    }

    pub fn full_system_progress_transition_partitions(&self) -> Vec<PartitionId> {
        collect_progress_transition_partitions(self.full_system_progress_transitions())
    }

    pub fn full_system_progress_transition_tick_window_by_partition(
        &self,
        partition: PartitionId,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transitions(), |transition| {
            transition.partition() == partition
        })
    }

    pub fn full_system_progress_transition_count_by_subject(&self, subject: &WaitForNode) -> usize {
        progress_transition_count(self.full_system_progress_transitions(), |transition| {
            transition.subject() == subject
        })
    }

    pub fn full_system_progress_transition_records_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_progress_transition_records(self.full_system_progress_transitions(), |transition| {
            transition.subject() == subject
        })
    }

    pub fn full_system_progress_transition_subject_summaries(
        &self,
    ) -> Vec<(WaitForNode, usize, Tick, Tick)> {
        collect_progress_transition_summaries(
            self.full_system_progress_transitions(),
            |transition| transition.subject().clone(),
        )
    }

    pub fn full_system_progress_transition_subjects(&self) -> Vec<WaitForNode> {
        collect_progress_transition_subjects(self.full_system_progress_transitions())
    }

    pub fn full_system_progress_transition_tick_window_by_subject(
        &self,
        subject: &WaitForNode,
    ) -> Option<(Tick, Tick)> {
        progress_transition_tick_window(self.full_system_progress_transitions(), |transition| {
            transition.subject() == subject
        })
    }

    pub fn parallel_scheduler_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        Ok(self
            .parallel_scheduler_livelock_diagnostics(threshold)?
            .len())
    }

    pub fn parallel_scheduler_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnostic>, ProgressMonitorError> {
        livelock_diagnostics_from_progress_transitions(
            threshold,
            self.parallel_scheduler_progress_transitions(),
        )
    }

    pub fn parallel_scheduler_livelock_diagnostic_transition_kind_window_summaries(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnosticTransitionKindWindowSummary>, ProgressMonitorError> {
        let diagnostics = self.parallel_scheduler_livelock_diagnostics(threshold)?;
        Ok(collect_livelock_diagnostic_transition_kind_window_summaries(&diagnostics))
    }

    pub fn data_cache_parallel_scheduler_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        Ok(self
            .data_cache_parallel_scheduler_livelock_diagnostics(threshold)?
            .len())
    }

    pub fn data_cache_parallel_scheduler_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnostic>, ProgressMonitorError> {
        livelock_diagnostics_from_progress_transitions(
            threshold,
            self.data_cache_parallel_scheduler_progress_transitions(),
        )
    }

    pub fn data_cache_parallel_scheduler_livelock_diagnostic_transition_kind_window_summaries(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnosticTransitionKindWindowSummary>, ProgressMonitorError> {
        let diagnostics = self.data_cache_parallel_scheduler_livelock_diagnostics(threshold)?;
        Ok(collect_livelock_diagnostic_transition_kind_window_summaries(&diagnostics))
    }

    pub fn full_system_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        Ok(self.full_system_livelock_diagnostics(threshold)?.len())
    }

    pub fn full_system_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnostic>, ProgressMonitorError> {
        livelock_diagnostics_from_progress_transitions(
            threshold,
            self.full_system_progress_transitions(),
        )
    }

    pub fn full_system_livelock_diagnostic_transition_kind_window_summaries(
        &self,
        threshold: u64,
    ) -> Result<Vec<LivelockDiagnosticTransitionKindWindowSummary>, ProgressMonitorError> {
        let diagnostics = self.full_system_livelock_diagnostics(threshold)?;
        Ok(collect_livelock_diagnostic_transition_kind_window_summaries(&diagnostics))
    }

    pub fn has_parallel_scheduler_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<bool, ProgressMonitorError> {
        Ok(self.parallel_scheduler_livelock_diagnostic_count(threshold)? != 0)
    }

    pub fn has_data_cache_parallel_scheduler_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<bool, ProgressMonitorError> {
        Ok(self.data_cache_parallel_scheduler_livelock_diagnostic_count(threshold)? != 0)
    }

    pub fn has_full_system_livelock_diagnostics(
        &self,
        threshold: u64,
    ) -> Result<bool, ProgressMonitorError> {
        Ok(self.full_system_livelock_diagnostic_count(threshold)? != 0)
    }
}

fn collect_parallel_progress_transitions<I>(transitions: I) -> Vec<ParallelProgressTransitionRecord>
where
    I: IntoIterator<Item = ParallelProgressTransitionRecord>,
{
    let mut transitions = transitions.into_iter().collect::<Vec<_>>();
    transitions.sort_by_key(|transition| {
        (
            transition.partition(),
            transition.tick(),
            transition.order(),
            transition.kind(),
            transition.subject().clone(),
        )
    });
    transitions
}

fn progress_transition_count(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    mut predicate: impl FnMut(&ParallelProgressTransitionRecord) -> bool,
) -> usize {
    transitions
        .into_iter()
        .filter(|transition| predicate(transition))
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

fn livelock_diagnostics_from_progress_transitions(
    threshold: u64,
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Result<Vec<LivelockDiagnostic>, ProgressMonitorError> {
    let mut monitor = ProgressMonitor::with_transition_threshold(threshold)?;
    for transition in transitions {
        monitor.record_transition(
            transition.subject().clone(),
            transition.kind(),
            transition.tick(),
        )?;
    }
    Ok(monitor.snapshot().diagnostics().to_vec())
}

fn collect_livelock_diagnostic_transition_kind_window_summaries<'a>(
    diagnostics: impl IntoIterator<Item = &'a LivelockDiagnostic>,
) -> Vec<LivelockDiagnosticTransitionKindWindowSummary> {
    let mut summaries = BTreeMap::<LivelockTransitionKind, (usize, u64, Tick, Tick)>::new();
    for diagnostic in diagnostics {
        for count in diagnostic.transition_kind_counts() {
            summaries
                .entry(count.kind())
                .and_modify(|summary| {
                    summary.0 += 1;
                    summary.1 += count.count();
                    summary.2 = summary.2.min(count.first_transition_tick());
                    summary.3 = summary.3.max(count.last_transition_tick());
                })
                .or_insert((
                    1,
                    count.count(),
                    count.first_transition_tick(),
                    count.last_transition_tick(),
                ));
        }
    }
    summaries
        .into_iter()
        .map(
            |(kind, (diagnostic_count, transition_count, first_tick, last_tick))| {
                (
                    kind,
                    diagnostic_count,
                    transition_count,
                    first_tick,
                    last_tick,
                )
            },
        )
        .collect()
}

fn collect_progress_transition_records(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
    mut predicate: impl FnMut(&ParallelProgressTransitionRecord) -> bool,
) -> Vec<ParallelProgressTransitionRecord> {
    collect_parallel_progress_transitions(
        transitions
            .into_iter()
            .filter(|transition| predicate(transition)),
    )
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
