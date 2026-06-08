use crate::WorkloadTrafficTraceReplayRun;

use super::{hash_str, hash_u64};

pub(super) fn hash_traffic_trace_replay_run(hash: &mut u64, run: &WorkloadTrafficTraceReplayRun) {
    hash_str(hash, run.route().as_str());
    hash_str(hash, run.resource().as_str());
    hash_u64(hash, run.tick_frequency());
    hash_u64(hash, run.agent() as u64);
    hash_u64(hash, run.line_bytes());
    hash_u64(hash, run.duration());
    hash_u64(hash, run.control_partition() as u64);
    hash_u64(hash, run.retry_delay());
}
