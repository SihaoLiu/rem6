use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::RiscvClusterSchedulerEpoch;
use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.remote_flow_count(source, target))
            .sum()
    }

    pub fn parallel_scheduler_total_remote_send_count(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::total_remote_send_count)
            .sum()
    }

    pub fn parallel_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.remote_send_count(source, target))
            .sum()
    }

    pub fn parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        merge_parallel_remote_flow_records(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::remote_flows),
        )
    }

    pub fn parallel_scheduler_remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        collect_parallel_remote_sends(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::remote_sends),
        )
    }

    pub fn parallel_scheduler_remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_remote_source_partitions(self.parallel_scheduler_remote_sends())
    }

    pub fn parallel_scheduler_remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_remote_target_partitions(self.parallel_scheduler_remote_sends())
    }
}

pub(crate) fn merge_parallel_remote_flow_records<I>(flows: I) -> Vec<ParallelRemoteFlowRecord>
where
    I: IntoIterator<Item = ParallelRemoteFlowRecord>,
{
    let mut merged: BTreeMap<(PartitionId, PartitionId), ParallelRemoteFlowRecord> =
        BTreeMap::new();
    for flow in flows {
        merged
            .entry((flow.source(), flow.target()))
            .and_modify(|stored| *stored = stored.merged_with(flow))
            .or_insert(flow);
    }
    merged.into_values().collect()
}

pub(crate) fn collect_parallel_remote_sends<I>(sends: I) -> Vec<ParallelRemoteSendRecord>
where
    I: IntoIterator<Item = ParallelRemoteSendRecord>,
{
    let mut sends = sends.into_iter().collect::<Vec<_>>();
    sends.sort_by_key(|send| {
        (
            send.source(),
            send.target(),
            send.source_tick(),
            send.delivery_tick(),
            send.order(),
        )
    });
    sends
}

fn collect_remote_source_partitions(
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<PartitionId> {
    sends
        .into_iter()
        .map(|send| send.source())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_remote_target_partitions(
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<PartitionId> {
    sends
        .into_iter()
        .map(|send| send.target())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
