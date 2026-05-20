use rem6_cache::{CacheControllerError, CacheControllerResultKind, MsiCacheController};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiAction, MsiEvent, MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(1), sequence)
}

fn controller() -> MsiCacheController {
    MsiCacheController::new(AgentId::new(10), layout(), Address::new(0x1000))
}

#[test]
fn controller_read_miss_fetches_line_and_completes_original_request() {
    let mut controller = controller();
    let read = MemoryRequest::read_shared(
        request_id(1),
        Address::new(0x1004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(read.clone()).unwrap();

    assert_eq!(miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(controller.state(), MsiState::InvalidToShared);
    assert!(miss.target_outcome().is_none());
    assert_eq!(
        miss.transition().unwrap().actions(),
        &[MsiAction::SendGetShared {
            line: MsiLineId::new(Address::new(0x1000))
        }]
    );
    let downstream = miss.downstream_request().unwrap();
    assert_eq!(downstream.operation(), MemoryOperation::ReadShared);
    assert_eq!(downstream.range().start(), Address::new(0x1000));
    assert_eq!(downstream.range().size(), AccessSize::new(64).unwrap());

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(downstream, Some(fill_data)).unwrap();
    let completed = controller.accept_fill(fill).unwrap();

    assert_eq!(completed.kind(), CacheControllerResultKind::Fill);
    assert_eq!(controller.state(), MsiState::Shared);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );

    let hit = controller.accept_cpu_request(read.clone()).unwrap();
    assert_eq!(hit.kind(), CacheControllerResultKind::Hit);
    assert!(hit.downstream_request().is_none());
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );
}

#[test]
fn controller_write_miss_fetches_unique_line_then_acks_store() {
    let mut controller = controller();
    let write = MemoryRequest::write(
        request_id(2),
        Address::new(0x1008),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(controller.state(), MsiState::InvalidToModified);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );

    let fill_data = vec![0; 64];
    let fill =
        MemoryResponse::completed(miss.downstream_request().unwrap(), Some(fill_data)).unwrap();
    let completed = controller.accept_fill(fill).unwrap();

    assert_eq!(controller.state(), MsiState::Modified);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );

    let read_back = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x1008),
        AccessSize::new(2).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read_back, Some(vec![0xaa, 0xbb])).unwrap()
        ))
    );
}

#[test]
fn controller_shared_store_uses_upgrade_transaction() {
    let mut controller = controller();
    controller.install_shared(vec![0x11; 64]).unwrap();
    let write = MemoryRequest::write(
        request_id(4),
        Address::new(0x1010),
        AccessSize::new(1).unwrap(),
        vec![0x44],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(controller.state(), MsiState::SharedToModified);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::Upgrade
    );

    let fill = MemoryResponse::completed(miss.downstream_request().unwrap(), None).unwrap();
    let completed = controller.accept_fill(fill).unwrap();
    assert_eq!(controller.state(), MsiState::Modified);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );
}

#[test]
fn controller_rejects_requests_while_line_is_transient() {
    let mut controller = controller();
    let first = MemoryRequest::read_shared(
        request_id(5),
        Address::new(0x1000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let second = MemoryRequest::read_shared(
        request_id(6),
        Address::new(0x1001),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();

    controller.accept_cpu_request(first).unwrap();
    assert_eq!(
        controller.accept_cpu_request(second).unwrap_err(),
        CacheControllerError::LineBusy {
            state: MsiState::InvalidToShared
        }
    );
}

#[test]
fn controller_snoop_write_invalidates_cached_line() {
    let mut controller = controller();
    controller.install_modified(vec![0x22; 64]).unwrap();

    let result = controller.accept_snoop(MsiEvent::SnoopWrite).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Snoop);
    assert_eq!(controller.state(), MsiState::Invalid);
    assert!(controller.cached_data().is_none());
    assert_eq!(
        result.transition().unwrap().actions(),
        &[
            MsiAction::SupplyData {
                line: MsiLineId::new(Address::new(0x1000))
            },
            MsiAction::Invalidate {
                line: MsiLineId::new(Address::new(0x1000))
            },
        ]
    );
}

#[test]
fn controller_rejects_wrong_line_and_unexpected_fill() {
    let mut controller = controller();
    let wrong = MemoryRequest::read_shared(
        request_id(7),
        Address::new(0x2000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    assert_eq!(
        controller.accept_cpu_request(wrong).unwrap_err(),
        CacheControllerError::WrongLine {
            expected: MsiLineId::new(Address::new(0x1000)),
            actual: MsiLineId::new(Address::new(0x2000)),
        }
    );

    let request = MemoryRequest::read_shared(
        request_id(8),
        Address::new(0x1000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let response = MemoryResponse::completed(&request, Some(vec![1])).unwrap();
    assert_eq!(
        controller.accept_fill(response).unwrap_err(),
        CacheControllerError::NoPendingMiss
    );
}
