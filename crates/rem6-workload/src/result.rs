use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadDataCacheProtocol {
    Msi,
    Mesi,
    Moesi,
}

impl WorkloadDataCacheProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Msi => "msi",
            Self::Mesi => "mesi",
            Self::Moesi => "moesi",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadDataCacheProtocolCount {
    protocol: WorkloadDataCacheProtocol,
    run_count: usize,
}

impl WorkloadDataCacheProtocolCount {
    pub const fn new(protocol: WorkloadDataCacheProtocol, run_count: usize) -> Self {
        Self {
            protocol,
            run_count,
        }
    }

    pub const fn protocol(&self) -> WorkloadDataCacheProtocol {
        self.protocol
    }

    pub const fn run_count(&self) -> usize {
        self.run_count
    }

    pub const fn is_empty(&self) -> bool {
        self.run_count == 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkloadParallelExecutionSummary {
    scheduler_epoch_count: usize,
    scheduler_empty_epoch_count: usize,
    scheduler_dispatch_count: usize,
    scheduler_batch_count: usize,
    active_scheduler_partition_count: usize,
    max_parallel_scheduler_workers: usize,
    data_cache_parallel_run_count: usize,
    data_cache_parallel_scheduler_epoch_count: usize,
    data_cache_parallel_scheduler_dispatch_count: usize,
    data_cache_parallel_scheduler_batch_count: usize,
    data_cache_parallel_scheduler_max_workers: usize,
    attributed_data_cache_parallel_run_count: usize,
    unattributed_data_cache_parallel_run_count: usize,
    data_cache_protocol_counts: Vec<WorkloadDataCacheProtocolCount>,
    data_cache_wait_for_edge_count: usize,
    data_cache_deadlock_diagnostic_count: usize,
}

impl WorkloadParallelExecutionSummary {
    pub const fn with_scheduler_counts(
        mut self,
        epoch_count: usize,
        empty_epoch_count: usize,
        dispatch_count: usize,
        batch_count: usize,
    ) -> Self {
        self.scheduler_epoch_count = epoch_count;
        self.scheduler_empty_epoch_count = empty_epoch_count;
        self.scheduler_dispatch_count = dispatch_count;
        self.scheduler_batch_count = batch_count;
        self
    }

    pub const fn with_scheduler_partitions(
        mut self,
        active_partition_count: usize,
        max_parallel_workers: usize,
    ) -> Self {
        self.active_scheduler_partition_count = active_partition_count;
        self.max_parallel_scheduler_workers = max_parallel_workers;
        self
    }

    pub const fn with_data_cache_parallel_counts(
        mut self,
        run_count: usize,
        scheduler_epoch_count: usize,
        scheduler_dispatch_count: usize,
        scheduler_batch_count: usize,
        scheduler_max_workers: usize,
    ) -> Self {
        self.data_cache_parallel_run_count = run_count;
        self.data_cache_parallel_scheduler_epoch_count = scheduler_epoch_count;
        self.data_cache_parallel_scheduler_dispatch_count = scheduler_dispatch_count;
        self.data_cache_parallel_scheduler_batch_count = scheduler_batch_count;
        self.data_cache_parallel_scheduler_max_workers = scheduler_max_workers;
        self
    }

    pub const fn with_data_cache_run_attribution(
        mut self,
        attributed_run_count: usize,
        unattributed_run_count: usize,
    ) -> Self {
        self.attributed_data_cache_parallel_run_count = attributed_run_count;
        self.unattributed_data_cache_parallel_run_count = unattributed_run_count;
        self
    }

    pub fn with_data_cache_protocol_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadDataCacheProtocolCount>,
    ) -> Self {
        let mut by_protocol = BTreeMap::new();
        for count in counts {
            if count.run_count() != 0 {
                by_protocol.insert(count.protocol(), count.run_count());
            }
        }
        self.data_cache_protocol_counts = by_protocol
            .into_iter()
            .map(|(protocol, run_count)| WorkloadDataCacheProtocolCount::new(protocol, run_count))
            .collect();
        self
    }

    pub const fn with_data_cache_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.data_cache_wait_for_edge_count = wait_for_edge_count;
        self.data_cache_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub const fn scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count
    }

    pub const fn scheduler_empty_epoch_count(&self) -> usize {
        self.scheduler_empty_epoch_count
    }

    pub const fn scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count
    }

    pub const fn scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count
    }

    pub const fn active_scheduler_partition_count(&self) -> usize {
        self.active_scheduler_partition_count
    }

    pub const fn max_parallel_scheduler_workers(&self) -> usize {
        self.max_parallel_scheduler_workers
    }

    pub const fn data_cache_parallel_run_count(&self) -> usize {
        self.data_cache_parallel_run_count
    }

    pub const fn data_cache_parallel_scheduler_epoch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn data_cache_parallel_scheduler_dispatch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_dispatch_count
    }

    pub const fn data_cache_parallel_scheduler_batch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_batch_count
    }

    pub const fn data_cache_parallel_scheduler_max_workers(&self) -> usize {
        self.data_cache_parallel_scheduler_max_workers
    }

    pub const fn attributed_data_cache_parallel_run_count(&self) -> usize {
        self.attributed_data_cache_parallel_run_count
    }

    pub const fn unattributed_data_cache_parallel_run_count(&self) -> usize {
        self.unattributed_data_cache_parallel_run_count
    }

    pub fn data_cache_protocol_counts(&self) -> &[WorkloadDataCacheProtocolCount] {
        &self.data_cache_protocol_counts
    }

    pub fn data_cache_protocols(&self) -> Vec<WorkloadDataCacheProtocol> {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::protocol)
            .collect()
    }

    pub fn data_cache_protocol_count_map(&self) -> BTreeMap<WorkloadDataCacheProtocol, usize> {
        self.data_cache_protocol_counts
            .iter()
            .map(|count| (count.protocol(), count.run_count()))
            .collect()
    }

    pub fn attributed_data_cache_protocol_run_count(&self) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::run_count)
            .sum()
    }

    pub fn data_cache_parallel_run_count_for_protocol(
        &self,
        protocol: WorkloadDataCacheProtocol,
    ) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .find(|count| count.protocol() == protocol)
            .map(WorkloadDataCacheProtocolCount::run_count)
            .unwrap_or(0)
    }

    pub fn has_data_cache_protocol(&self, protocol: WorkloadDataCacheProtocol) -> bool {
        self.data_cache_parallel_run_count_for_protocol(protocol) != 0
    }

    pub const fn has_unattributed_data_cache_parallel_runs(&self) -> bool {
        self.unattributed_data_cache_parallel_run_count != 0
    }

    pub const fn data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_wait_for_edge_count
    }

    pub const fn data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.data_cache_deadlock_diagnostic_count
    }

    pub const fn has_data_cache_diagnostics(&self) -> bool {
        self.data_cache_wait_for_edge_count != 0 || self.data_cache_deadlock_diagnostic_count != 0
    }

    pub const fn full_system_parallel_scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count + self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count + self.data_cache_parallel_scheduler_dispatch_count
    }

    pub const fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count + self.data_cache_parallel_scheduler_batch_count
    }

    pub const fn full_system_parallel_scheduler_max_workers(&self) -> usize {
        if self.max_parallel_scheduler_workers > self.data_cache_parallel_scheduler_max_workers {
            self.max_parallel_scheduler_workers
        } else {
            self.data_cache_parallel_scheduler_max_workers
        }
    }

    pub const fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.has_parallel_scheduler_work() || self.has_data_cache_parallel_work()
    }

    pub const fn has_parallel_scheduler_work(&self) -> bool {
        self.scheduler_dispatch_count != 0
            || self.scheduler_batch_count != 0
            || self.max_parallel_scheduler_workers != 0
    }

    pub const fn has_data_cache_parallel_work(&self) -> bool {
        self.data_cache_parallel_run_count != 0
            || self.data_cache_parallel_scheduler_dispatch_count != 0
            || self.data_cache_parallel_scheduler_max_workers != 0
    }
}
