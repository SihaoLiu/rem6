use rem6_cache::CacheControllerResultKind;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};
use rem6_transport::TargetOutcome;

use crate::{push_response_records_from_outcomes, CpuResponseRecord};

fn request(agent: u32, sequence: u64, address: u64) -> MemoryRequest {
    let layout = CacheLineLayout::new(16).unwrap();
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout,
    )
    .unwrap()
}

#[test]
fn target_outcome_records_preserve_all_responding_targets() {
    let first = request(1, 10, 0x1004);
    let second = request(1, 11, 0x1008);
    let first_response =
        rem6_memory::MemoryResponse::completed(&first, Some(vec![0xaa; 8])).unwrap();
    let second_response =
        rem6_memory::MemoryResponse::completed(&second, Some(vec![0xbb; 8])).unwrap();
    let outcomes = vec![
        TargetOutcome::Respond(first_response),
        TargetOutcome::NoResponse,
        TargetOutcome::Respond(second_response),
    ];
    let mut records: Vec<CpuResponseRecord> = Vec::new();

    let count = push_response_records_from_outcomes(
        &mut records,
        42,
        CacheControllerResultKind::Fill,
        &outcomes,
    );

    assert_eq!(count, 2);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].tick(), 42);
    assert_eq!(records[0].request(), first.id());
    assert_eq!(records[0].data().unwrap(), &[0xaa; 8]);
    assert_eq!(records[1].tick(), 42);
    assert_eq!(records[1].request(), second.id());
    assert_eq!(records[1].data().unwrap(), &[0xbb; 8]);
}

#[test]
fn target_outcome_records_delayed_response_tick() {
    let request = request(1, 12, 0x1004);
    let response = rem6_memory::MemoryResponse::completed(&request, Some(vec![0xcc; 8])).unwrap();
    let outcomes = vec![TargetOutcome::RespondAfter { delay: 7, response }];
    let mut records: Vec<CpuResponseRecord> = Vec::new();

    let count = push_response_records_from_outcomes(
        &mut records,
        42,
        CacheControllerResultKind::Fill,
        &outcomes,
    );

    assert_eq!(count, 1);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tick(), 49);
    assert_eq!(records[0].request(), request.id());
    assert_eq!(records[0].data().unwrap(), &[0xcc; 8]);
}
