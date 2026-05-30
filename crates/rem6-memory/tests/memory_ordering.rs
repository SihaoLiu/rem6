use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryBarrierSet,
    MemoryOperation, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn request(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent, sequence),
        Address::new(0x1000 + sequence * 0x40),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn memory_barrier_sets_match_ordering_read_and_write_effects() {
    let reads = MemoryBarrierSet::new(true, false);
    let writes = MemoryBarrierSet::new(false, true);
    let memory = MemoryBarrierSet::memory();

    assert!(reads.matches_operation(MemoryOperation::ReadShared));
    assert!(reads.matches_operation(MemoryOperation::Atomic));
    assert!(!reads.matches_operation(MemoryOperation::Write));

    assert!(writes.matches_operation(MemoryOperation::Write));
    assert!(writes.matches_operation(MemoryOperation::Atomic));
    assert!(!writes.matches_operation(MemoryOperation::InstructionFetch));

    assert!(memory.matches_operation(MemoryOperation::InstructionFetch));
    assert!(memory.matches_operation(MemoryOperation::WritebackDirty));
}

#[test]
fn requests_report_same_agent_ordering_edges() {
    let prior = request(7, 1);
    let release = request(7, 2).with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        None,
    ));
    let acquire = request(7, 3).with_ordering(MemoryAccessOrdering::new(
        None,
        Some(MemoryBarrierSet::memory()),
    ));
    let later = request(7, 4);
    let foreign = request(9, 5);

    assert!(prior.orders_before(&release));
    assert!(acquire.orders_before(&later));
    assert!(!foreign.orders_before(&release));
    assert!(!release.orders_before(&foreign));
}

#[test]
fn strict_order_requests_serialize_same_agent_neighbors() {
    let prior = request(7, 10);
    let strict = request(7, 11).with_uncacheable_strict_order();
    let later = request(7, 12);
    let foreign = request(9, 13);

    assert!(prior.orders_before(&strict));
    assert!(strict.orders_before(&later));
    assert!(!foreign.orders_before(&strict));
    assert!(!strict.orders_before(&foreign));
}
