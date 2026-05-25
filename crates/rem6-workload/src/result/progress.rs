use rem6_kernel::ParallelProgressTransitionRecord;

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

    pub fn data_cache_parallel_scheduler_progress_transitions(
        &self,
    ) -> &[ParallelProgressTransitionRecord] {
        &self.data_cache_parallel_scheduler_progress_transitions
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
}
