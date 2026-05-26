use rem6_kernel::PartitionId;

use crate::result_collect::{
    collect_parallel_remote_source_partitions, collect_parallel_remote_target_partitions,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn parallel_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_source_partitions(
            self.parallel_scheduler_remote_flow_evidence(),
            self.parallel_scheduler_remote_sends.iter().copied(),
        )
    }

    pub fn parallel_scheduler_remote_source_partition_count(&self) -> usize {
        self.parallel_scheduler_remote_source_partitions().len()
    }

    pub fn parallel_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_target_partitions(
            self.parallel_scheduler_remote_flow_evidence(),
            self.parallel_scheduler_remote_sends.iter().copied(),
        )
    }

    pub fn parallel_scheduler_remote_target_partition_count(&self) -> usize {
        self.parallel_scheduler_remote_target_partitions().len()
    }

    pub fn data_cache_parallel_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_source_partitions(
            self.data_cache_parallel_scheduler_remote_flow_evidence(),
            self.data_cache_parallel_scheduler_remote_sends
                .iter()
                .copied(),
        )
    }

    pub fn data_cache_parallel_scheduler_remote_source_partition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_remote_source_partitions()
            .len()
    }

    pub fn data_cache_parallel_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_target_partitions(
            self.data_cache_parallel_scheduler_remote_flow_evidence(),
            self.data_cache_parallel_scheduler_remote_sends
                .iter()
                .copied(),
        )
    }

    pub fn data_cache_parallel_scheduler_remote_target_partition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_remote_target_partitions()
            .len()
    }

    pub fn gpu_dma_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_source_partitions(
            self.gpu_dma_scheduler_remote_flow_evidence(),
            self.gpu_dma_scheduler_remote_sends().iter().copied(),
        )
    }

    pub fn gpu_dma_scheduler_remote_source_partition_count(&self) -> usize {
        self.gpu_dma_scheduler_remote_source_partitions().len()
    }

    pub fn gpu_dma_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_target_partitions(
            self.gpu_dma_scheduler_remote_flow_evidence(),
            self.gpu_dma_scheduler_remote_sends().iter().copied(),
        )
    }

    pub fn gpu_dma_scheduler_remote_target_partition_count(&self) -> usize {
        self.gpu_dma_scheduler_remote_target_partitions().len()
    }

    pub fn accelerator_dma_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_source_partitions(
            self.accelerator_dma_scheduler_remote_flow_evidence(),
            self.accelerator_dma_scheduler_remote_sends()
                .iter()
                .copied(),
        )
    }

    pub fn accelerator_dma_scheduler_remote_source_partition_count(&self) -> usize {
        self.accelerator_dma_scheduler_remote_source_partitions()
            .len()
    }

    pub fn accelerator_dma_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_target_partitions(
            self.accelerator_dma_scheduler_remote_flow_evidence(),
            self.accelerator_dma_scheduler_remote_sends()
                .iter()
                .copied(),
        )
    }

    pub fn accelerator_dma_scheduler_remote_target_partition_count(&self) -> usize {
        self.accelerator_dma_scheduler_remote_target_partitions()
            .len()
    }

    pub fn full_system_parallel_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_source_partitions(
            self.full_system_parallel_scheduler_remote_flows(),
            self.full_system_parallel_scheduler_remote_sends(),
        )
    }

    pub fn full_system_parallel_scheduler_remote_source_partition_count(&self) -> usize {
        self.full_system_parallel_scheduler_remote_source_partitions()
            .len()
    }

    pub fn full_system_parallel_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_parallel_remote_target_partitions(
            self.full_system_parallel_scheduler_remote_flows(),
            self.full_system_parallel_scheduler_remote_sends(),
        )
    }

    pub fn full_system_parallel_scheduler_remote_target_partition_count(&self) -> usize {
        self.full_system_parallel_scheduler_remote_target_partitions()
            .len()
    }
}
