use super::*;
use crate::{
    AccessSize, AgentId, CacheLineLayout, CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord,
    CpuResetState, MemoryRequestId, MemoryRouteId, TransportEndpointId,
};
use rem6_kernel::PartitionId;
use rem6_memory::{Address, MemoryRequest};
use rem6_transport::{MemoryTrace, ParallelMemoryTransaction, TargetOutcome, TransportError};

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn core_with_completed_fetch() -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(0x8000),
            ),
            CpuFetchConfig::new(
                endpoint("cpu0.ifetch"),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .push(CpuFetchEvent::completed(
            CpuFetchRecord::new(
                0,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                MemoryRequestId::new(AgentId::new(7), 0),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0013u32.to_le_bytes().to_vec(),
        ));
    core
}

fn unknown_route_transaction() -> ParallelMemoryTransaction {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(9), 0),
        Address::new(0x9000),
        AccessSize::new(4).unwrap(),
        CacheLineLayout::new(16).unwrap(),
    )
    .unwrap();
    ParallelMemoryTransaction::new(
        MemoryRouteId::new(99),
        request,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
        |_delivery| {},
    )
}

#[test]
fn failed_parallel_batch_cancels_prepared_pipeline_wake() {
    let cpu = CpuId::new(0);
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    let transport = MemoryTransport::new();
    let mut prepared_actions = PreparedParallelActions::new();

    assert!(push_prepared_pipeline_cycle_drive_event(
        cpu,
        &core,
        &mut scheduler,
        &mut prepared_actions,
    )
    .unwrap());
    let wake = core
        .checkpoint_owned_in_order_pipeline_wakes()
        .into_iter()
        .next()
        .expect("prepared pipeline cycle should own its scheduler wake");

    let result = finish_prepared_parallel_actions(
        &mut scheduler,
        &transport,
        prepared_actions,
        vec![cpu],
        vec![unknown_route_transaction()],
    );

    assert!(matches!(
        result,
        Err(RiscvClusterError::Core {
            cpu: failed_cpu,
            error: RiscvCpuError::Transport(TransportError::UnknownRoute { route }),
        }) if failed_cpu == cpu && route == MemoryRouteId::new(99)
    ));
    assert_eq!(scheduler.pending_event_snapshot(wake.1.id()), None);
    assert!(core.checkpoint_owned_in_order_pipeline_wakes().is_empty());
}
