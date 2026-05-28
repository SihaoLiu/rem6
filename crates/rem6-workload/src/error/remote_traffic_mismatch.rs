use rem6_kernel::Tick;

use crate::WorkloadParallelRemoteFlowScope;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelRemoteTrafficConsistencyMismatch {
    pub scope: WorkloadParallelRemoteFlowScope,
    pub source: u32,
    pub target: u32,
    pub flow_send_count: usize,
    pub send_record_count: usize,
    pub flow_first_tick: Tick,
    pub send_first_tick: Option<Tick>,
    pub flow_last_tick: Tick,
    pub send_last_tick: Option<Tick>,
    pub flow_minimum_delay: Option<Tick>,
    pub send_minimum_delay: Option<Tick>,
    pub flow_maximum_delay: Option<Tick>,
    pub send_maximum_delay: Option<Tick>,
}
