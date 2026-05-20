use rem6_cache::{MesiCacheController, MesiCacheControllerResultKind};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_mesi::{MesiAction, MesiEvent, MesiLineId, MesiState};
use rem6_transport::TargetOutcome;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(2), sequence)
}

fn controller() -> MesiCacheController {
    MesiCacheController::new(AgentId::new(20), layout(), Address::new(0x2000))
}

#[test]
fn mesi_controller_read_miss_can_fill_exclusive_and_silently_upgrade_store() {
    let mut controller = controller();
    let read = MemoryRequest::read_shared(
        request_id(1),
        Address::new(0x2004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(read.clone()).unwrap();

    assert_eq!(miss.kind(), MesiCacheControllerResultKind::Miss);
    assert_eq!(controller.state(), MesiState::InvalidToExclusive);
    assert_eq!(
        miss.transition().unwrap().actions(),
        &[MesiAction::SendGetShared {
            line: MesiLineId::new(Address::new(0x2000))
        }]
    );
    let downstream = miss.downstream_request().unwrap();
    assert_eq!(downstream.operation(), MemoryOperation::ReadShared);
    assert_eq!(downstream.range().start(), Address::new(0x2000));
    assert_eq!(downstream.range().size(), AccessSize::new(64).unwrap());

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(downstream, Some(fill_data)).unwrap();
    let completed = controller
        .accept_fill(fill, MesiEvent::DataExclusive)
        .unwrap();

    assert_eq!(completed.kind(), MesiCacheControllerResultKind::Fill);
    assert_eq!(controller.state(), MesiState::Exclusive);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );

    let write = MemoryRequest::write(
        request_id(2),
        Address::new(0x2006),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let store_hit = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(store_hit.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(controller.state(), MesiState::Modified);
    assert!(store_hit.downstream_request().is_none());
    assert_eq!(
        store_hit.transition().unwrap().actions(),
        &[
            MesiAction::SilentUpgrade {
                line: MesiLineId::new(Address::new(0x2000))
            },
            MesiAction::WriteHit {
                line: MesiLineId::new(Address::new(0x2000))
            },
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
        Address::new(0x2004),
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
fn mesi_controller_shared_store_uses_upgrade_transaction() {
    let mut controller = controller();
    let read = MemoryRequest::read_shared(
        request_id(4),
        Address::new(0x2000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap();
    let miss = controller.accept_cpu_request(read.clone()).unwrap();
    let fill = MemoryResponse::completed(miss.downstream_request().unwrap(), Some(vec![0x11; 64]))
        .unwrap();
    controller.accept_fill(fill, MesiEvent::DataShared).unwrap();
    assert_eq!(controller.state(), MesiState::Shared);

    let write = MemoryRequest::write(
        request_id(5),
        Address::new(0x2010),
        AccessSize::new(1).unwrap(),
        vec![0x44],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let upgrade = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(controller.state(), MesiState::SharedToModified);
    assert_eq!(upgrade.kind(), MesiCacheControllerResultKind::Miss);
    assert_eq!(
        upgrade.downstream_request().unwrap().operation(),
        MemoryOperation::Upgrade
    );

    let fill = MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap();
    let completed = controller
        .accept_fill(fill, MesiEvent::DataModified)
        .unwrap();

    assert_eq!(controller.state(), MesiState::Modified);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );
}
