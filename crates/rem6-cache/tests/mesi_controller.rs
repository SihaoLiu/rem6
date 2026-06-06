use rem6_cache::{
    MesiCacheController, MesiCacheControllerError, MesiCacheControllerResultKind,
    MesiCacheControllerSnapshot,
};
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

fn load_locked(sequence: u64, address: u64, size: u64) -> MemoryRequest {
    MemoryRequest::load_locked(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn store_conditional(sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::store_conditional(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
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

#[test]
fn mesi_controller_store_conditional_respects_cache_resident_reservations() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();

    let failed_store = store_conditional(8, 0x2010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let failed = controller.accept_cpu_request(failed_store.clone()).unwrap();

    assert_eq!(failed.kind(), MesiCacheControllerResultKind::Hit);
    assert_eq!(
        failed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::store_conditional_failed(&failed_store).unwrap()
        ))
    );
    assert_eq!(
        &controller.cached_data().unwrap()[0x10..0x14],
        &[0x10, 0x11, 0x12, 0x13]
    );

    let load = load_locked(9, 0x2018, 8);
    let load_hit = controller.accept_cpu_request(load.clone()).unwrap();
    assert_eq!(
        load_hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(
                &load,
                Some(vec![0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f])
            )
            .unwrap()
        ))
    );

    let completed_store = store_conditional(10, 0x2010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let completed = controller
        .accept_cpu_request(completed_store.clone())
        .unwrap();

    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&completed_store, None).unwrap()
        ))
    );
    assert_eq!(
        &controller.cached_data().unwrap()[0x10..0x14],
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );
}

#[test]
fn mesi_controller_store_conditional_miss_fails_without_reservation_after_fill() {
    let mut controller = controller();
    let store = store_conditional(11, 0x2010, vec![0xaa, 0xbb, 0xcc, 0xdd]);

    let miss = controller.accept_cpu_request(store.clone()).unwrap();

    assert_eq!(miss.kind(), MesiCacheControllerResultKind::Miss);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );

    let downstream = miss.downstream_request().unwrap().clone();
    let fill = MemoryResponse::completed(&downstream, Some((0..64).collect())).unwrap();
    let completed = controller
        .accept_fill(fill, MesiEvent::DataModified)
        .unwrap();

    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::store_conditional_failed(&store).unwrap()
        ))
    );
    assert_eq!(
        &controller.cached_data().unwrap()[0x10..0x14],
        &[0x10, 0x11, 0x12, 0x13]
    );
}

#[test]
fn mesi_controller_public_line_install_clears_load_locked_reservations() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();

    let load = load_locked(12, 0x2038, 8);
    controller.accept_cpu_request(load).unwrap();
    controller.install_modified(vec![0xee; 64]).unwrap();
    let store = store_conditional(13, 0x2038, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let failed = controller.accept_cpu_request(store.clone()).unwrap();

    assert_eq!(
        failed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::store_conditional_failed(&store).unwrap()
        ))
    );
    assert_eq!(
        &controller.cached_data().unwrap()[0x38..0x3c],
        &[0xee, 0xee, 0xee, 0xee]
    );
}

#[test]
fn mesi_controller_snapshot_restore_preserves_pending_miss_and_sequence() {
    let mut source = controller();
    let read = MemoryRequest::read_shared(
        request_id(6),
        Address::new(0x2008),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = source.accept_cpu_request(read.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap().clone();
    let snapshot = source.snapshot();
    let pending = snapshot.pending().unwrap();

    assert_eq!(snapshot.agent(), AgentId::new(20));
    assert_eq!(snapshot.layout(), layout());
    assert_eq!(snapshot.line(), MesiLineId::new(Address::new(0x2000)));
    assert_eq!(snapshot.state(), MesiState::InvalidToExclusive);
    assert_eq!(snapshot.next_sequence(), 1);
    assert_eq!(pending.original(), &read);
    assert_eq!(pending.downstream(), downstream.id());

    let mut restored = controller();
    restored.install_modified(vec![0xff; 64]).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(&downstream, Some(fill_data)).unwrap();
    let completed = restored.accept_fill(fill, MesiEvent::DataShared).unwrap();

    assert_eq!(completed.kind(), MesiCacheControllerResultKind::Fill);
    assert_eq!(restored.state(), MesiState::Shared);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![8, 9, 10, 11])).unwrap()
        ))
    );

    let write = MemoryRequest::write(
        request_id(7),
        Address::new(0x2010),
        AccessSize::new(1).unwrap(),
        vec![0xee],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let upgrade = restored.accept_cpu_request(write).unwrap();
    assert_eq!(
        upgrade.downstream_request().unwrap().id(),
        MemoryRequestId::new(AgentId::new(20), 1)
    );
}

#[test]
fn mesi_controller_restore_rejects_foreign_snapshot() {
    let mut source = controller();
    source.install_exclusive(vec![0x33; 64]).unwrap();
    let snapshot = source.snapshot();
    let mut foreign = MesiCacheController::new(AgentId::new(21), layout(), Address::new(0x2000));

    assert_eq!(
        foreign.restore(&snapshot).unwrap_err(),
        MesiCacheControllerError::SnapshotIdentityMismatch {
            expected_agent: AgentId::new(21),
            actual_agent: AgentId::new(20),
            expected_line: MesiLineId::new(Address::new(0x2000)),
            actual_line: MesiLineId::new(Address::new(0x2000)),
            expected_layout: layout(),
            actual_layout: layout(),
        }
    );
    assert_eq!(foreign.state(), MesiState::Invalid);
}

#[test]
fn mesi_controller_restore_rejects_snapshot_with_bad_line_data() {
    let mut source = controller();
    source.install_exclusive(vec![0x33; 64]).unwrap();
    let snapshot = source.snapshot();
    let corrupt = MesiCacheControllerSnapshot::new(
        snapshot.line_state().clone(),
        snapshot.layout(),
        snapshot.next_sequence(),
        Some(vec![0; 63]),
        snapshot.pending().cloned(),
    );
    let mut restored = controller();

    assert_eq!(
        restored.restore(&corrupt).unwrap_err(),
        MesiCacheControllerError::LineDataSizeMismatch {
            expected: 64,
            actual: 63,
        }
    );
    assert_eq!(restored.state(), MesiState::Invalid);
    assert!(restored.cached_data().is_none());
}
