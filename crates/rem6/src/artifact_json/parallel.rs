use super::optional_tick_json;

use crate::{
    Rem6ParallelFrontierSummary, Rem6ParallelPartitionSummary, Rem6ParallelReadyPartitionSummary,
};

pub(super) fn empty_parallel_json(worker_limit: usize, min_remote_delay: u64) -> String {
    format!(
        "{{\"scheduler\":{{\"worker_limit\":{},\"min_remote_delay\":{},\"epochs\":0,\"dispatches\":0,\"batches\":0,\"max_workers\":0,\"total_workers\":0,\"active_partitions\":0,\"remote_sends\":0,\"batch_worker_ticks\":0,\"batch_worker_capacity_ticks\":0,\"batch_idle_worker_ticks\":0,\"worker_slots\":[],\"worker_lanes\":[],\"partitions\":[],\"frontiers\":[],\"final_frontiers\":[],\"ready_partitions\":[]}}}}",
        worker_limit, min_remote_delay
    )
}

impl crate::Rem6ParallelWorkerSlotSummary {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"slot\":{},\"active_ticks\":{},\"idle_ticks\":{}}}",
            self.slot, self.active_ticks, self.idle_ticks
        )
    }
}

impl crate::Rem6ParallelWorkerLaneSummary {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"lane\":{},\"partition\":{},\"active_ticks\":{}}}",
            self.lane, self.partition, self.active_ticks
        )
    }
}

impl Rem6ParallelPartitionSummary {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"workers\":{},\"dispatches\":{},\"remote_sends\":{},\"remote_receives\":{},\"max_pending_events\":{}}}",
            self.partition,
            self.workers,
            self.dispatches,
            self.remote_sends,
            self.remote_receives,
            self.max_pending_events,
        )
    }
}

impl Rem6ParallelFrontierSummary {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"now\":{},\"safe_until\":{},\"next_tick\":{},\"pending_events\":{}}}",
            self.partition,
            self.now,
            self.safe_until,
            optional_tick_json(self.next_tick),
            self.pending_events,
        )
    }
}

impl Rem6ParallelReadyPartitionSummary {
    pub(super) fn to_json(&self) -> String {
        format!(
            "{{\"partition\":{},\"next_tick\":{}}}",
            self.partition, self.next_tick
        )
    }
}
