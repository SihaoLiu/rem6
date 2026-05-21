use rem6_cache::{MoesiCacheController, MoesiCacheControllerError, MoesiCacheControllerResultKind};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_moesi::{MoesiAction, MoesiEvent, MoesiLineId, MoesiState};
use rem6_transport::TargetOutcome;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(3), sequence)
}

fn line() -> MoesiLineId {
    MoesiLineId::new(Address::new(0x4000))
}

fn controller() -> MoesiCacheController {
    MoesiCacheController::new(AgentId::new(30), layout(), Address::new(0x4000))
}

#[test]
fn moesi_controller_read_miss_can_fill_exclusive_and_silently_upgrade_store() {
    let mut controller = controller();
    let read = MemoryRequest::read_shared(
        request_id(1),
        Address::new(0x4004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(read.clone()).unwrap();

    assert_eq!(miss.kind(), MoesiCacheControllerResultKind::Miss);
    assert_eq!(controller.state(), MoesiState::InvalidToExclusive);
    assert_eq!(
        miss.transition().unwrap().actions(),
        &[MoesiAction::SendGetShared { line: line() }]
    );
    let downstream = miss.downstream_request().unwrap();
    assert_eq!(downstream.operation(), MemoryOperation::ReadShared);
    assert_eq!(downstream.range().start(), Address::new(0x4000));
    assert_eq!(downstream.range().size(), AccessSize::new(64).unwrap());

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(downstream, Some(fill_data)).unwrap();
    let completed = controller
        .accept_fill(fill, MoesiEvent::DataExclusive)
        .unwrap();

    assert_eq!(completed.kind(), MoesiCacheControllerResultKind::Fill);
    assert_eq!(controller.state(), MoesiState::Exclusive);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );

    let write = MemoryRequest::write(
        request_id(2),
        Address::new(0x4006),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let store_hit = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(store_hit.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(controller.state(), MoesiState::Modified);
    assert!(store_hit.downstream_request().is_none());
    assert_eq!(
        store_hit.transition().unwrap().actions(),
        &[
            MoesiAction::SilentUpgrade { line: line() },
            MoesiAction::WriteHit { line: line() },
        ]
    );
    assert_eq!(
        store_hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );

    let read_back = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x4004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read_back, Some(vec![4, 5, 0xaa, 0xbb])).unwrap()
        ))
    );
}

#[test]
fn moesi_controller_owned_store_uses_upgrade_transaction() {
    let mut controller = controller();
    controller.install_owned(vec![0x11; 64]).unwrap();

    let write = MemoryRequest::write(
        request_id(4),
        Address::new(0x4010),
        AccessSize::new(1).unwrap(),
        vec![0x44],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let upgrade = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(controller.state(), MoesiState::OwnedToModified);
    assert_eq!(upgrade.kind(), MoesiCacheControllerResultKind::Miss);
    assert_eq!(
        upgrade.downstream_request().unwrap().operation(),
        MemoryOperation::Upgrade
    );

    let fill = MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap();
    let completed = controller
        .accept_fill(fill, MoesiEvent::DataModified)
        .unwrap();

    assert_eq!(controller.state(), MoesiState::Modified);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );
    assert_eq!(controller.cached_data().unwrap()[0x10], 0x44);
}

#[test]
fn moesi_controller_snoop_read_keeps_owned_data_resident() {
    let mut controller = controller();
    controller.install_modified(vec![0x22; 64]).unwrap();

    let snoop = controller.accept_snoop(MoesiEvent::SnoopRead).unwrap();

    assert_eq!(snoop.kind(), MoesiCacheControllerResultKind::Snoop);
    assert_eq!(controller.state(), MoesiState::Owned);
    assert_eq!(controller.cached_data().unwrap()[0], 0x22);
    assert_eq!(
        snoop.transition().unwrap().actions(),
        &[
            MoesiAction::SupplyData { line: line() },
            MoesiAction::DowngradeToOwned { line: line() },
        ]
    );

    let read = MemoryRequest::read_shared(
        request_id(5),
        Address::new(0x4000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read.clone()).unwrap();
    assert_eq!(hit.kind(), MoesiCacheControllerResultKind::Hit);
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![0x22])).unwrap()
        ))
    );
}

#[test]
fn moesi_controller_snoop_write_invalidates_owned_data_after_transfer() {
    let mut controller = controller();
    controller.install_owned(vec![0x33; 64]).unwrap();

    let snoop = controller.accept_snoop(MoesiEvent::SnoopWrite).unwrap();

    assert_eq!(snoop.kind(), MoesiCacheControllerResultKind::Snoop);
    assert_eq!(controller.state(), MoesiState::Invalid);
    assert!(controller.cached_data().is_none());
    assert_eq!(
        snoop.transition().unwrap().actions(),
        &[
            MoesiAction::SupplyData { line: line() },
            MoesiAction::Invalidate { line: line() },
        ]
    );
}

#[test]
fn moesi_controller_rejects_requests_while_line_is_transient() {
    let mut controller = controller();
    let first = MemoryRequest::read_shared(
        request_id(6),
        Address::new(0x4000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let second = MemoryRequest::read_shared(
        request_id(7),
        Address::new(0x4001),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();

    controller.accept_cpu_request(first).unwrap();
    assert_eq!(
        controller.accept_cpu_request(second).unwrap_err(),
        MoesiCacheControllerError::LineBusy {
            state: MoesiState::InvalidToExclusive,
        }
    );
}

#[test]
fn moesi_controller_rejects_wrong_line_and_unexpected_fill_event() {
    let mut controller = controller();
    let wrong = MemoryRequest::read_shared(
        request_id(8),
        Address::new(0x5000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    assert_eq!(
        controller.accept_cpu_request(wrong).unwrap_err(),
        MoesiCacheControllerError::WrongLine {
            expected: line(),
            actual: MoesiLineId::new(Address::new(0x5000)),
        }
    );

    let request = MemoryRequest::read_shared(
        request_id(9),
        Address::new(0x4000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let miss = controller.accept_cpu_request(request).unwrap();
    let fill =
        MemoryResponse::completed(miss.downstream_request().unwrap(), Some(vec![1; 64])).unwrap();
    assert_eq!(
        controller
            .accept_fill(fill, MoesiEvent::SnoopRead)
            .unwrap_err(),
        MoesiCacheControllerError::UnexpectedFillEvent {
            event: MoesiEvent::SnoopRead,
        }
    );
}
