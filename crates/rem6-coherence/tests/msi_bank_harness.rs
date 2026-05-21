use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{MsiBankDirectoryHarness, SubmitKind};
use rem6_directory::{DirectoryDataSource, DirectoryLineState};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::{MsiLineId, MsiState};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn line(address: u64) -> MsiLineId {
    MsiLineId::new(Address::new(address))
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

fn write(agent_id: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = size(data.len() as u64);
    MemoryRequest::write(
        request_id(agent_id, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn line_data(byte: u8) -> Vec<u8> {
    vec![byte; layout().bytes() as usize]
}

#[test]
fn msi_bank_harness_keeps_independent_lines_in_one_cache_bank() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    let first = harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004))
        .unwrap();
    assert_eq!(first.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(first.cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(
        first
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::BackingMemory,
    );

    let second = harness
        .submit_cpu_request(agent(1), read(1, 11, 0x1018))
        .unwrap();
    assert_eq!(second.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(second.cache_result(), CacheControllerResultKind::Miss);

    assert_eq!(
        harness.cache_line_addresses(agent(1)).unwrap(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1010)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 10));
    assert_eq!(responses[0].status(), ResponseStatus::Completed);
    assert_eq!(responses[0].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[1].request(), request_id(1, 11));
    assert_eq!(responses[1].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_transfers_modified_owner_data_without_touching_other_lines() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    harness
        .submit_cpu_request(agent(1), write(1, 20, 0x1004, vec![0xaa; 8]))
        .unwrap();
    let shared = harness
        .submit_cpu_request(agent(2), read(2, 30, 0x1004))
        .unwrap();

    assert_eq!(shared.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1)),
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000))
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(2), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 20));
    assert_eq!(responses[0].data(), None);
    assert_eq!(responses[1].request(), request_id(2, 30));
    assert_eq!(responses[1].data().unwrap(), &[0xaa; 8]);
    assert_eq!(
        harness.backing_line(Address::new(0x1010)).unwrap(),
        line_data(0x22).as_slice()
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010))
    );
}
