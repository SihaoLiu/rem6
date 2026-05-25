use rem6_dram::{
    DramController, DramGeometry, DramQosRequest, DramQosTurnaroundPolicy, DramTiming,
};
use rem6_fabric::{QosPriority, QosQueueArbiter, QosQueuePolicyKind};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn request(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn dram_qos_batch_preserves_same_agent_release_ordering() {
    let mut controller = DramController::new(geometry(), timing());
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let prior = request(1, 0x1000);
    let release = request(2, 0x2000).with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        None,
    ));

    let accesses = controller
        .schedule_qos_batch(
            0,
            [
                DramQosRequest::new(&prior, QosPriority::new(1), 0),
                DramQosRequest::new(&release, QosPriority::new(0), 1),
            ],
            &mut arbiter,
        )
        .unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].request(), prior.id());
    assert_eq!(accesses[1].request(), release.id());
}

#[test]
fn dram_qos_batch_preserves_same_agent_acquire_ordering() {
    let mut controller = DramController::new(geometry(), timing());
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let acquire = request(3, 0x3000).with_ordering(MemoryAccessOrdering::new(
        None,
        Some(MemoryBarrierSet::memory()),
    ));
    let later = request(4, 0x4000);

    let accesses = controller
        .schedule_qos_batch(
            0,
            [
                DramQosRequest::new(&acquire, QosPriority::new(1), 0),
                DramQosRequest::new(&later, QosPriority::new(0), 1),
            ],
            &mut arbiter,
        )
        .unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].request(), acquire.id());
    assert_eq!(accesses[1].request(), later.id());
}

#[test]
fn dram_qos_turnaround_preserves_same_agent_release_ordering() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &request(10, 0x0000)).unwrap();
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let prior = write(11, 0x0040);
    let release = request(12, 0x0100).with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        None,
    ));

    let accesses = controller
        .schedule_qos_batch_with_turnaround_policy(
            8,
            [
                DramQosRequest::new(&prior, QosPriority::new(0), 0),
                DramQosRequest::new(&release, QosPriority::new(0), 1),
            ],
            &mut arbiter,
            DramQosTurnaroundPolicy::PreferCurrentDirection,
        )
        .unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].request(), prior.id());
    assert_eq!(accesses[1].request(), release.id());
}
