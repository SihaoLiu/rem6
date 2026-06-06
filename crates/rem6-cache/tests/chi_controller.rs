use rem6_cache::{ChiCacheController, ChiCacheControllerError, ChiCacheControllerResultKind};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse,
};
use rem6_protocol_chi::{ChiAction, ChiEvent, ChiLineId, ChiState};
use rem6_transport::TargetOutcome;

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(4), sequence)
}

fn line() -> ChiLineId {
    ChiLineId::new(Address::new(0x6000))
}

fn controller() -> ChiCacheController {
    ChiCacheController::new(AgentId::new(40), layout(), Address::new(0x6000))
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
fn chi_controller_read_miss_fills_shared_then_store_uses_make_read_unique() {
    let mut controller = controller();
    let read = MemoryRequest::read_shared(
        request_id(1),
        Address::new(0x6004),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = controller.accept_cpu_request(read.clone()).unwrap();

    assert_eq!(miss.kind(), ChiCacheControllerResultKind::Miss);
    assert_eq!(controller.state(), ChiState::InvalidToSharedClean);
    assert_eq!(
        miss.transition().unwrap().actions(),
        &[ChiAction::SendReadShared { line: line() }]
    );
    let downstream = miss.downstream_request().unwrap();
    assert_eq!(downstream.operation(), MemoryOperation::ReadShared);
    assert_eq!(downstream.range().start(), Address::new(0x6000));
    assert_eq!(downstream.range().size(), AccessSize::new(64).unwrap());

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(downstream, Some(fill_data)).unwrap();
    let completed = controller
        .accept_fill(fill, ChiEvent::CompDataSharedClean)
        .unwrap();

    assert_eq!(completed.kind(), ChiCacheControllerResultKind::Fill);
    assert_eq!(controller.state(), ChiState::SharedClean);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![4, 5, 6, 7])).unwrap()
        ))
    );

    let write = MemoryRequest::write(
        request_id(2),
        Address::new(0x6006),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        layout(),
    )
    .unwrap();
    let upgrade = controller.accept_cpu_request(write.clone()).unwrap();

    assert_eq!(upgrade.kind(), ChiCacheControllerResultKind::Miss);
    assert_eq!(controller.state(), ChiState::SharedCleanToUniqueClean);
    assert_eq!(
        upgrade.transition().unwrap().actions(),
        &[ChiAction::SendMakeReadUnique { line: line() }]
    );
    assert_eq!(
        upgrade.downstream_request().unwrap().operation(),
        MemoryOperation::Upgrade
    );

    let fill = MemoryResponse::completed(upgrade.downstream_request().unwrap(), None).unwrap();
    let completed = controller
        .accept_fill(fill, ChiEvent::CompDataUniqueDirty)
        .unwrap();

    assert_eq!(completed.kind(), ChiCacheControllerResultKind::Fill);
    assert_eq!(controller.state(), ChiState::UniqueDirty);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&write, None).unwrap()
        ))
    );
    assert_eq!(controller.cached_data().unwrap()[0x06..0x08], [0xaa, 0xbb]);
}

#[test]
fn chi_controller_unique_dirty_snoop_paths_preserve_or_drop_data() {
    let mut controller = controller();
    controller.install_unique_dirty(vec![0x22; 64]).unwrap();

    let shared_snoop = controller.accept_snoop(ChiEvent::SnoopShared).unwrap();

    assert_eq!(shared_snoop.kind(), ChiCacheControllerResultKind::Snoop);
    assert_eq!(controller.state(), ChiState::SharedClean);
    assert_eq!(controller.cached_data().unwrap()[0], 0x22);
    assert_eq!(
        shared_snoop.transition().unwrap().actions(),
        &[
            ChiAction::SnoopData { line: line() },
            ChiAction::DowngradeToSharedClean { line: line() },
        ]
    );

    controller.install_unique_dirty(vec![0x33; 64]).unwrap();
    let unique_snoop = controller.accept_snoop(ChiEvent::SnoopUnique).unwrap();

    assert_eq!(unique_snoop.kind(), ChiCacheControllerResultKind::Snoop);
    assert_eq!(controller.state(), ChiState::Invalid);
    assert!(controller.cached_data().is_none());
    assert_eq!(
        unique_snoop.transition().unwrap().actions(),
        &[
            ChiAction::SnoopData { line: line() },
            ChiAction::Invalidate { line: line() },
        ]
    );
}

#[test]
fn chi_controller_store_conditional_respects_cache_resident_reservations() {
    let mut controller = controller();
    controller.install_unique_dirty((0..64).collect()).unwrap();

    let failed_store = store_conditional(5, 0x6010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
    let failed = controller.accept_cpu_request(failed_store.clone()).unwrap();

    assert_eq!(failed.kind(), ChiCacheControllerResultKind::Hit);
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

    let load = load_locked(6, 0x6018, 8);
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

    let completed_store = store_conditional(7, 0x6010, vec![0xaa, 0xbb, 0xcc, 0xdd]);
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
fn chi_controller_store_conditional_miss_fails_without_reservation_after_fill() {
    let mut controller = controller();
    let store = store_conditional(8, 0x6010, vec![0xaa, 0xbb, 0xcc, 0xdd]);

    let miss = controller.accept_cpu_request(store.clone()).unwrap();

    assert_eq!(miss.kind(), ChiCacheControllerResultKind::Miss);
    assert_eq!(
        miss.downstream_request().unwrap().operation(),
        MemoryOperation::ReadUnique
    );

    let downstream = miss.downstream_request().unwrap().clone();
    let fill = MemoryResponse::completed(&downstream, Some((0..64).collect())).unwrap();
    let completed = controller
        .accept_fill(fill, ChiEvent::CompDataUniqueDirty)
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
fn chi_controller_public_line_install_clears_load_locked_reservations() {
    let mut controller = controller();
    controller.install_unique_dirty((0..64).collect()).unwrap();

    let load = load_locked(9, 0x6038, 8);
    controller.accept_cpu_request(load).unwrap();
    controller.install_unique_dirty(vec![0xee; 64]).unwrap();
    let store = store_conditional(10, 0x6038, vec![0xaa, 0xbb, 0xcc, 0xdd]);
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
fn chi_controller_rejects_busy_bad_fill_and_restores_pending_snapshot() {
    let mut source = controller();
    let read = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x6008),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();
    let second = MemoryRequest::read_shared(
        request_id(4),
        Address::new(0x600c),
        AccessSize::new(4).unwrap(),
        layout(),
    )
    .unwrap();

    let miss = source.accept_cpu_request(read.clone()).unwrap();
    let downstream = miss.downstream_request().unwrap().clone();
    assert_eq!(
        source.accept_cpu_request(second).unwrap_err(),
        ChiCacheControllerError::LineBusy {
            state: ChiState::InvalidToSharedClean,
        }
    );
    assert_eq!(
        source
            .accept_fill(
                MemoryResponse::completed(&downstream, Some(vec![0x11; 64])).unwrap(),
                ChiEvent::CpuRead,
            )
            .unwrap_err(),
        ChiCacheControllerError::UnexpectedFillEvent {
            event: ChiEvent::CpuRead,
        }
    );

    let snapshot = source.snapshot();
    let pending = snapshot.pending().unwrap();
    assert_eq!(snapshot.agent(), AgentId::new(40));
    assert_eq!(snapshot.line(), line());
    assert_eq!(snapshot.state(), ChiState::InvalidToSharedClean);
    assert_eq!(snapshot.next_sequence(), 1);
    assert_eq!(pending.original(), &read);
    assert_eq!(pending.downstream(), downstream.id());

    let mut restored = controller();
    restored.install_unique_clean(vec![0xff; 64]).unwrap();
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let fill_data: Vec<u8> = (0..64).collect();
    let fill = MemoryResponse::completed(&downstream, Some(fill_data)).unwrap();
    let completed = restored
        .accept_fill(fill, ChiEvent::CompDataSharedClean)
        .unwrap();

    assert_eq!(completed.kind(), ChiCacheControllerResultKind::Fill);
    assert_eq!(restored.state(), ChiState::SharedClean);
    assert_eq!(
        completed.target_outcome(),
        Some(&TargetOutcome::Respond(
            MemoryResponse::completed(&read, Some(vec![8, 9, 10, 11])).unwrap()
        ))
    );
}
