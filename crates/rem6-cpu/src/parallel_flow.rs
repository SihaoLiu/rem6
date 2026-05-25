use std::collections::BTreeMap;

use rem6_kernel::{ParallelRemoteFlowRecord, PartitionId};

pub(crate) fn merge_parallel_remote_flow_records<I>(flows: I) -> Vec<ParallelRemoteFlowRecord>
where
    I: IntoIterator<Item = ParallelRemoteFlowRecord>,
{
    let mut merged: BTreeMap<(PartitionId, PartitionId), ParallelRemoteFlowRecord> =
        BTreeMap::new();
    for flow in flows {
        merged
            .entry((flow.source(), flow.target()))
            .and_modify(|stored| {
                *stored = ParallelRemoteFlowRecord::new(
                    stored.source(),
                    stored.target(),
                    stored.send_count() + flow.send_count(),
                    stored.first_tick().min(flow.first_tick()),
                    stored.last_tick().max(flow.last_tick()),
                );
            })
            .or_insert(flow);
    }
    merged.into_values().collect()
}
