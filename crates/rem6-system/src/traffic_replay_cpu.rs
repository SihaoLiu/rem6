use std::sync::{Arc, Mutex};

use rem6_cpu::RiscvCore;
use rem6_kernel::SchedulerContext;
use rem6_transport::{RequestDelivery, TargetOutcome};

use crate::{
    TrafficTraceReplayTargetError, TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetRuntime,
};

pub fn traffic_trace_replay_runtime_data_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayTargetRuntime>>,
    core: RiscvCore,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let event = runtime
        .lock()
        .expect("trace replay target runtime lock")
        .target_event(delivery)?;
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => Ok(outcome),
        TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
            let request_id = record.failure().request_id();
            context
                .schedule_local_after(delay, move |context| {
                    let tick = context.now();
                    runtime
                        .lock()
                        .expect("trace replay target runtime lock")
                        .record_memory_failure(tick, record);
                    core.record_data_failure(request_id, tick);
                })
                .expect("validated trace replay failure delay");
            Ok(TargetOutcome::NoResponse)
        }
    }
}
