use rem6_kernel::{ParallelProgressTransitionRecord, ProgressMonitor, ProgressMonitorError};

use crate::RiscvSystemRun;

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

    pub fn data_cache_parallel_scheduler_progress_transitions(
        &self,
    ) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.data_cache_parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.progress_transitions()),
        )
    }

    pub fn full_system_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.parallel_scheduler_progress_transitions()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_progress_transitions()),
        )
    }

    pub fn parallel_scheduler_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        let mut monitor = ProgressMonitor::with_transition_threshold(threshold)?;
        for epoch in self.parallel_scheduler_epochs() {
            for batch in epoch.batches() {
                for transition in batch.progress_transitions() {
                    monitor.record_transition(
                        transition.subject().clone(),
                        transition.kind(),
                        transition.tick(),
                    )?;
                }
            }
        }
        Ok(monitor.snapshot().diagnostics().len())
    }

    pub fn data_cache_parallel_scheduler_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        let mut monitor = ProgressMonitor::with_transition_threshold(threshold)?;
        for epoch in self.data_cache_parallel_scheduler_epochs() {
            for transition in epoch.progress_transitions() {
                monitor.record_transition(
                    transition.subject().clone(),
                    transition.kind(),
                    transition.tick(),
                )?;
            }
        }
        Ok(monitor.snapshot().diagnostics().len())
    }

    pub fn full_system_livelock_diagnostic_count(
        &self,
        threshold: u64,
    ) -> Result<usize, ProgressMonitorError> {
        Ok(
            self.parallel_scheduler_livelock_diagnostic_count(threshold)?
                + self.data_cache_parallel_scheduler_livelock_diagnostic_count(threshold)?,
        )
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
