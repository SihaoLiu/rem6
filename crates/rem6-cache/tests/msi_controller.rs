use rem6_cache::{
    CacheControllerError, CacheControllerResultKind, MsiCacheController, MsiCacheControllerSnapshot,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryOperation,
    MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::{MsiAction, MsiEvent, MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(1), sequence)
}

type AtomicBinary = fn(u64, u64) -> u64;
type AtomicLogicalCase = (MemoryAtomicOp, AtomicBinary);

fn controller() -> MsiCacheController {
    MsiCacheController::new(AgentId::new(10), layout(), Address::new(0x1000))
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

fn store_conditional_fail(sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::store_conditional_fail(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn atomic(sequence: u64, address: u64, data: Vec<u8>, mask: ByteMask) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic(
        request_id(sequence),
        Address::new(address),
        size,
        data,
        mask,
        layout(),
    )
    .unwrap()
}

fn atomic_with_op(
    sequence: u64,
    address: u64,
    op: MemoryAtomicOp,
    data: Vec<u8>,
    mask: ByteMask,
) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic_with_op(
        request_id(sequence),
        Address::new(address),
        size,
        op,
        data,
        mask,
        layout(),
    )
    .unwrap()
}

fn atomic_no_return(sequence: u64, address: u64, data: Vec<u8>, mask: ByteMask) -> MemoryRequest {
    let size = AccessSize::new(data.len() as u64).unwrap();
    MemoryRequest::atomic_no_return(
        request_id(sequence),
        Address::new(address),
        size,
        MemoryAtomicOp::Swap,
        data,
        mask,
        layout(),
    )
    .unwrap()
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
fn controller_atomic_hit_returns_old_bytes_and_updates_modified_line() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();
    let request = atomic(
        24,
        0x1008,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::from_bits(vec![true, false, true, false]).unwrap(),
    );

    let result = controller.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        result.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&request, Some(vec![8, 9, 10, 11])).unwrap()
        ))
    );
    let read_back = MemoryRequest::read_shared(
        request_id(25),
        Address::new(0x1008),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read_back, Some(vec![0xaa, 9, 0xcc, 11])).unwrap()
        ))
    );
}

#[test]
fn controller_store_conditional_fail_hit_does_not_write_even_with_reservation() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();

    let load = load_locked(27, 0x1010, 4);
    controller.accept_cpu_request(load).unwrap();
    let forced_fail = store_conditional_fail(28, 0x1010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let failed = controller.accept_cpu_request(forced_fail.clone()).unwrap();

    assert_eq!(failed.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        failed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::store_conditional_failed(&forced_fail).unwrap()
        ))
    );
    assert_eq!(
        &controller.cached_data().unwrap()[0x10..0x14],
        &[0x10, 0x11, 0x12, 0x13]
    );
}

#[test]
fn controller_store_conditional_respects_cache_resident_reservations() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();

    let failed_store = store_conditional(28, 0x1010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let failed = controller.accept_cpu_request(failed_store.clone()).unwrap();

    assert_eq!(failed.kind(), CacheControllerResultKind::Hit);
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

    let load = load_locked(29, 0x1018, 8);
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

    let completed_store = store_conditional(30, 0x1010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
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
fn controller_store_conditional_miss_fails_without_reservation_after_fill() {
    let mut controller = controller();
    let store = store_conditional(31, 0x1010, vec![0xaa, 0xbb, 0xcc, 0xdd]);

    let miss = controller.accept_cpu_request(store.clone()).unwrap();

    assert_eq!(miss.kind(), CacheControllerResultKind::Miss);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );

    let downstream = miss.downstream_request().unwrap().clone();
    let fill = MemoryResponse::completed(&downstream, Some((0..64).collect())).unwrap();
    let completed = controller.accept_fill(fill).unwrap();

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
fn controller_public_line_install_clears_load_locked_reservations() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();

    let load = load_locked(32, 0x1038, 8);
    controller.accept_cpu_request(load).unwrap();
    controller.install_modified(vec![0xee; 64]).unwrap();
    let store = store_conditional(33, 0x1038, vec![0xaa, 0xbb, 0xcc, 0xdd]);
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
fn controller_atomic_no_return_hit_updates_modified_line_without_old_bytes() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();
    let request = atomic_no_return(
        26,
        0x1008,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::from_bits(vec![true, false, true, false]).unwrap(),
    );

    let result = controller.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        result.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&request, None).unwrap()
        ))
    );
    let read_back = MemoryRequest::read_shared(
        request_id(27),
        Address::new(0x1008),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read_back, Some(vec![0xaa, 9, 0xcc, 11])).unwrap()
        ))
    );
}

#[test]
fn controller_atomic_add_hit_returns_old_bytes_and_updates_modified_line() {
    let mut controller = controller();
    controller.install_modified((0..64).collect()).unwrap();
    let request = atomic_with_op(
        26,
        0x1008,
        MemoryAtomicOp::Add,
        0x0102_0304_0506_0708u64.to_le_bytes().to_vec(),
        ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
    );

    let result = controller.accept_cpu_request(request.clone()).unwrap();

    assert_eq!(result.kind(), CacheControllerResultKind::Hit);
    assert_eq!(
        result.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&request, Some(vec![8, 9, 10, 11, 12, 13, 14, 15])).unwrap()
        ))
    );
    let read_back = MemoryRequest::read_shared(
        request_id(27),
        Address::new(0x1008),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read_back, Some(vec![0x10; 8])).unwrap()
        ))
    );
}

#[test]
fn controller_atomic_logical_ops_hit_return_old_bytes_and_update_modified_line() {
    let cases: [AtomicLogicalCase; 3] = [
        (MemoryAtomicOp::Xor, |old: u64, operand: u64| old ^ operand),
        (MemoryAtomicOp::Or, |old: u64, operand: u64| old | operand),
        (MemoryAtomicOp::And, |old: u64, operand: u64| old & operand),
    ];

    for (index, (op, expected)) in cases.into_iter().enumerate() {
        let old = 0xf0f0_0f0f_aaaa_5555u64;
        let operand = 0x0ff0_f00f_5555_3333u64;
        let mut line = vec![0; 64];
        line[8..16].copy_from_slice(&old.to_le_bytes());
        let mut controller = controller();
        controller.install_modified(line).unwrap();
        let request = atomic_with_op(
            28 + index as u64,
            0x1008,
            op,
            operand.to_le_bytes().to_vec(),
            ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
        );

        let result = controller.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), CacheControllerResultKind::Hit);
        assert_eq!(
            result.target_outcome(),
            Some(&TargetOutcome::Respond(
                MemoryResponse::completed(&request, Some(old.to_le_bytes().to_vec())).unwrap()
            ))
        );
        let read_back = MemoryRequest::read_shared(
            request_id(31 + index as u64),
            Address::new(0x1008),
            AccessSize::new(8).unwrap(),
            layout(),
        )
        .unwrap();
        let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
        assert_eq!(
            hit.target_outcome(),
            Some(&TargetOutcome::Respond(
                MemoryResponse::completed(
                    &read_back,
                    Some(expected(old, operand).to_le_bytes().to_vec())
                )
                .unwrap()
            ))
        );
    }
}

#[test]
fn controller_atomic_min_max_ops_hit_return_old_bytes_and_update_modified_line() {
    let negative = 0xffff_ffff_ffff_fff0u64;
    let positive = 7u64;
    let cases: [(MemoryAtomicOp, u64, u64, u64); 4] = [
        (MemoryAtomicOp::MinSigned, negative, positive, negative),
        (MemoryAtomicOp::MaxSigned, negative, positive, positive),
        (MemoryAtomicOp::MinUnsigned, negative, positive, positive),
        (MemoryAtomicOp::MaxUnsigned, negative, positive, negative),
    ];

    for (index, (op, old, operand, expected)) in cases.into_iter().enumerate() {
        let mut line = vec![0; 64];
        line[8..16].copy_from_slice(&old.to_le_bytes());
        let mut controller = controller();
        controller.install_modified(line).unwrap();
        let request = atomic_with_op(
            34 + index as u64,
            0x1008,
            op,
            operand.to_le_bytes().to_vec(),
            ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
        );

        let result = controller.accept_cpu_request(request.clone()).unwrap();

        assert_eq!(result.kind(), CacheControllerResultKind::Hit);
        assert_eq!(
            result.target_outcome(),
            Some(&TargetOutcome::Respond(
                MemoryResponse::completed(&request, Some(old.to_le_bytes().to_vec())).unwrap()
            ))
        );
        let read_back = MemoryRequest::read_shared(
            request_id(38 + index as u64),
            Address::new(0x1008),
            AccessSize::new(8).unwrap(),
            layout(),
        )
        .unwrap();
        let hit = controller.accept_cpu_request(read_back.clone()).unwrap();
        assert_eq!(
            hit.target_outcome(),
            Some(&TargetOutcome::Respond(
                MemoryResponse::completed(&read_back, Some(expected.to_le_bytes().to_vec()))
                    .unwrap()
            ))
        );
    }
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
fn controller_snapshot_restore_preserves_installed_data() {
    let mut controller = controller();
    let line_data = vec![0x5a; 64];
    controller.install_modified(line_data.clone()).unwrap();

    let snapshot = controller.snapshot();

    assert_eq!(snapshot.agent(), AgentId::new(10));
    assert_eq!(snapshot.layout(), layout());
    assert_eq!(snapshot.line(), MsiLineId::new(Address::new(0x1000)));
    assert_eq!(snapshot.state(), MsiState::Modified);
    assert_eq!(snapshot.cached_data(), Some(line_data.as_slice()));
    assert!(snapshot.pending().is_none());

    controller.accept_snoop(MsiEvent::SnoopWrite).unwrap();
    assert_ne!(controller.snapshot(), snapshot);

    controller.restore(&snapshot).unwrap();
    assert_eq!(controller.snapshot(), snapshot);

    let read = MemoryRequest::read_shared(
        request_id(10),
        Address::new(0x1002),
        AccessSize::new(2).unwrap(),
        layout(),
    )
    .unwrap();
    let hit = controller.accept_cpu_request(read.clone()).unwrap();
    assert_eq!(
        hit.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![0x5a, 0x5a])).unwrap()
        ))
    );
}

#[test]
fn controller_snapshot_restore_preserves_pending_miss_and_sequence() {
    let mut source = controller();
    let read = MemoryRequest::read_shared(
        request_id(11),
        Address::new(0x1004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = source.accept_cpu_request(read.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap().clone();
    let snapshot = source.snapshot();
    let pending = snapshot.pending().unwrap();

    assert_eq!(snapshot.state(), MsiState::InvalidToShared);
    assert_eq!(snapshot.next_sequence(), 1);
    assert_eq!(pending.original(), &read);
    assert_eq!(pending.downstream(), downstream.id());
    assert_eq!(pending.fill_event(), MsiEvent::DataShared);

    let mut restored = controller();
    restored.install_modified(vec![0xff; 64]).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(&downstream, Some(fill_data)).unwrap();
    let completed = restored.accept_fill(fill).unwrap();

    assert_eq!(completed.kind(), CacheControllerResultKind::Fill);
    assert_eq!(restored.state(), MsiState::Shared);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );

    let write = MemoryRequest::write(
        request_id(12),
        Address::new(0x1010),
        AccessSize::new(1).unwrap(),
        vec![0xee],
        ByteMask::full(AccessSize::new(1).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let upgrade = restored.accept_cpu_request(write).unwrap();
    assert_eq!(
        upgrade.downstream_request().unwrap().id(),
        MemoryRequestId::new(AgentId::new(10), 1)
    );
}

#[test]
fn controller_restore_rejects_foreign_snapshot() {
    let mut source = controller();
    source.install_shared(vec![0x33; 64]).unwrap();
    let snapshot = source.snapshot();
    let mut foreign = MsiCacheController::new(AgentId::new(11), layout(), Address::new(0x1000));

    assert_eq!(
        foreign.restore(&snapshot).unwrap_err(),
        CacheControllerError::SnapshotIdentityMismatch {
            expected_agent: AgentId::new(11),
            actual_agent: AgentId::new(10),
            expected_line: MsiLineId::new(Address::new(0x1000)),
            actual_line: MsiLineId::new(Address::new(0x1000)),
            expected_layout: layout(),
            actual_layout: layout(),
        }
    );
    assert_eq!(foreign.state(), MsiState::Invalid);
}

#[test]
fn controller_restore_rejects_snapshot_with_bad_line_data() {
    let mut source = controller();
    source.install_shared(vec![0x33; 64]).unwrap();
    let snapshot = source.snapshot();
    let corrupt = MsiCacheControllerSnapshot::new(
        snapshot.line_state().clone(),
        snapshot.layout(),
        snapshot.next_sequence(),
        Some(vec![0; 63]),
        snapshot.pending().cloned(),
    );
    let mut restored = controller();

    assert_eq!(
        restored.restore(&corrupt).unwrap_err(),
        CacheControllerError::LineDataSizeMismatch {
            expected: 64,
            actual: 63,
        }
    );
    assert_eq!(restored.state(), MsiState::Invalid);
    assert!(restored.cached_data().is_none());
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
