use std::collections::BTreeMap;

use rem6_cache::{
    MshrEntry, MshrQueueConfig, MshrQueueSnapshot, MshrTarget, MshrTargetSource, MsiCacheBank,
    MsiCacheBankSnapshot,
};
use rem6_coherence::MsiBankDirectoryHarnessSnapshot;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId,
};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn request_id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn read(agent_id: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn store_conditional_fail(
    agent_id: u32,
    sequence: u64,
    address: u64,
    data: Vec<u8>,
) -> MemoryRequest {
    let size = size(data.len() as u64);
    MemoryRequest::store_conditional_fail(
        request_id(agent_id, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn msi_bank_harness_byte_snapshot_round_trips_pending_store_conditional_fail_target() {
    let config = MshrQueueConfig::new(2, 4, 0).unwrap();
    let mut bank = MsiCacheBank::new_with_mshr(agent(1), layout(), config.clone());
    bank.accept_cpu_request(read(1, 66, 0x2114)).unwrap();

    let snapshot = bank.snapshot();
    let current_mshr = snapshot.mshr().unwrap();
    let current_entry = &current_mshr.entries()[0];
    let fail = store_conditional_fail(1, 67, 0x2118, vec![0x7d; 8]);
    let mut targets = current_entry.targets().to_vec();
    targets.push(MshrTarget::from_parts(
        fail.clone(),
        1,
        1,
        MshrTargetSource::Demand,
        true,
        None,
    ));
    let mshr_snapshot = MshrQueueSnapshot::new(
        config,
        vec![MshrEntry::from_parts(
            current_entry.handle(),
            current_entry.line(),
            current_entry.ready_tick(),
            current_entry.order(),
            current_entry.in_service(),
            current_entry.pending_modified(),
            targets,
        )],
        current_mshr.next_handle(),
        current_mshr.next_order() + 1,
    );
    let snapshot = MsiBankDirectoryHarnessSnapshot::new(
        layout(),
        BTreeMap::from([(
            agent(1),
            MsiCacheBankSnapshot::new_with_mshr(
                snapshot.agent(),
                snapshot.layout(),
                snapshot.next_sequence(),
                snapshot.lines().to_vec(),
                mshr_snapshot,
            ),
        )]),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let rebuilt = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    let targets = rebuilt
        .cache_snapshot(agent(1))
        .unwrap()
        .mshr()
        .unwrap()
        .entries()[0]
        .targets();

    assert_eq!(rebuilt, snapshot);
    assert_eq!(targets[1].request(), &fail);
    assert_eq!(
        targets[1].request().operation(),
        MemoryOperation::StoreConditionalFail
    );
}
