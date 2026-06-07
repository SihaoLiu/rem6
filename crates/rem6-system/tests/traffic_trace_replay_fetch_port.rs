use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuCore, CpuFetchConfig, CpuFetchEventKind, CpuId, CpuResetState, RiscvCore};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_system::{
    traffic_trace_replay_runtime_fetch_target_outcome, TrafficTraceReplayTargetRuntime,
};
use rem6_traffic::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceErrorKind,
    TrafficTraceMemoryFailure, TrafficTraceReplayAction,
};
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport};

mod support;

use support::traffic_trace::endpoint;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn fetch_core() -> RiscvCore {
    RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(0),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                rem6_transport::MemoryRouteId::new(0),
                layout(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    )
}

#[test]
fn traffic_trace_replay_runtime_fetch_target_outcome_records_cpu_fetch_failure() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = fetch_core();

    let runtime = Arc::new(Mutex::new(TrafficTraceReplayTargetRuntime::default()));
    let replay = Arc::clone(&runtime);
    let fetch_core = core.clone();
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, context| {
            let failure = TrafficTraceMemoryFailure::new(
                delivery.request().id(),
                TrafficTraceErrorKind::Read,
            );
            replay
                .lock()
                .unwrap()
                .record_batch(&TrafficControllerEventBatch::new(vec![
                    TrafficControllerEvent::TraceReplayAction(
                        TrafficTraceReplayAction::MemoryFailure { tick: 9, failure },
                    ),
                ]))
                .unwrap();
            traffic_trace_replay_runtime_fetch_target_outcome(
                Arc::clone(&replay),
                fetch_core,
                &delivery,
                context,
            )
            .unwrap()
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(core.inner().pc(), Address::new(0x8000));
    let fetch_events = core.inner().fetch_events();
    assert_eq!(
        fetch_events
            .iter()
            .map(|event| event.kind())
            .collect::<Vec<_>>(),
        vec![CpuFetchEventKind::Issued, CpuFetchEventKind::Failed],
    );
    assert_eq!(fetch_events[1].tick(), 9);
    assert_eq!(fetch_events[1].pc(), Address::new(0x8000));
    assert_eq!(fetch_events[1].route(), fetch_route);
    assert_eq!(fetch_events[1].endpoint(), &endpoint("l1i0"));

    let runtime = runtime.lock().unwrap();
    assert!(runtime.is_empty());
    assert_eq!(runtime.memory_failures().len(), 1);
    assert_eq!(
        runtime.memory_failures()[0].record().failure().error(),
        TrafficTraceErrorKind::Read,
    );
}
