use rem6_cache::{
    ChiCacheController, MesiCacheController, MoesiCacheController, MsiCacheController,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn read(agent: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent, sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(agent: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(agent, sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn cache_downstream_read_misses_preserve_source_ordering() {
    let ordering = MemoryAccessOrdering::new(None, Some(MemoryBarrierSet::memory()));

    let mut msi = MsiCacheController::new(AgentId::new(10), layout(), Address::new(0x1000));
    let request = read(1, 1, 0x1008).with_ordering(ordering);
    let miss = msi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut mesi = MesiCacheController::new(AgentId::new(20), layout(), Address::new(0x2000));
    let request = read(2, 2, 0x2008).with_ordering(ordering);
    let miss = mesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut moesi = MoesiCacheController::new(AgentId::new(30), layout(), Address::new(0x3000));
    let request = read(3, 3, 0x3008).with_ordering(ordering);
    let miss = moesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut chi = ChiCacheController::new(AgentId::new(40), layout(), Address::new(0x4000));
    let request = read(4, 4, 0x4008).with_ordering(ordering);
    let miss = chi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);
}

#[test]
fn cache_downstream_write_misses_preserve_source_ordering() {
    let ordering = MemoryAccessOrdering::new(Some(MemoryBarrierSet::memory()), None);

    let mut msi = MsiCacheController::new(AgentId::new(10), layout(), Address::new(0x5000));
    let request = write(1, 5, 0x5008).with_ordering(ordering);
    let miss = msi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut mesi = MesiCacheController::new(AgentId::new(20), layout(), Address::new(0x6000));
    let request = write(2, 6, 0x6008).with_ordering(ordering);
    let miss = mesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut moesi = MoesiCacheController::new(AgentId::new(30), layout(), Address::new(0x7000));
    let request = write(3, 7, 0x7008).with_ordering(ordering);
    let miss = moesi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);

    let mut chi = ChiCacheController::new(AgentId::new(40), layout(), Address::new(0x8000));
    let request = write(4, 8, 0x8008).with_ordering(ordering);
    let miss = chi.accept_cpu_request(request.clone()).unwrap();
    assert_eq!(miss.downstream_request().unwrap().ordering(), ordering);
}

#[test]
fn cache_downstream_misses_preserve_uncacheable_strict_order_flags() {
    let mut msi = MsiCacheController::new(AgentId::new(10), layout(), Address::new(0x9000));
    let request = read(1, 9, 0x9008).with_uncacheable_strict_order();
    let miss = msi.accept_cpu_request(request.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap();
    assert!(downstream.is_uncacheable());
    assert!(downstream.is_strict_ordered());

    let mut mesi = MesiCacheController::new(AgentId::new(20), layout(), Address::new(0xa000));
    let request = read(2, 10, 0xa008).with_uncacheable_strict_order();
    let miss = mesi.accept_cpu_request(request.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap();
    assert!(downstream.is_uncacheable());
    assert!(downstream.is_strict_ordered());

    let mut moesi = MoesiCacheController::new(AgentId::new(30), layout(), Address::new(0xb000));
    let request = read(3, 11, 0xb008).with_uncacheable_strict_order();
    let miss = moesi.accept_cpu_request(request.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap();
    assert!(downstream.is_uncacheable());
    assert!(downstream.is_strict_ordered());

    let mut chi = ChiCacheController::new(AgentId::new(40), layout(), Address::new(0xc000));
    let request = read(4, 12, 0xc008).with_uncacheable_strict_order();
    let miss = chi.accept_cpu_request(request.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap();
    assert!(downstream.is_uncacheable());
    assert!(downstream.is_strict_ordered());
}
