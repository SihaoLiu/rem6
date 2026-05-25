use rem6_kernel::{ProgressMonitor, ProgressMonitorError};

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
