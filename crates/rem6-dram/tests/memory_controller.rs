use rem6_dram::{
    DramAccessKind, DramControllerConfig, DramError, DramGeometry, DramMemoryController,
    DramMemoryError, DramTiming,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryError, MemoryRequest,
    MemoryRequestId, MemoryTargetId, ResponseStatus,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
}

fn fast_timing() -> DramTiming {
    DramTiming::new(2, 4, 6, 2, 3).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(6), sequence)
}

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
}

fn read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(address: u64, bytes: &[u8], sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes.len() as u64).unwrap(),
        bytes.to_vec(),
        ByteMask::full(AccessSize::new(bytes.len() as u64).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

fn controller_with_targets() -> (DramMemoryController, MemoryTargetId, MemoryTargetId) {
    let low = MemoryTargetId::new(1);
    let high = MemoryTargetId::new(2);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            low,
            layout(),
            geometry(),
            timing(),
        ))
        .unwrap();
    controller
        .add_target(DramControllerConfig::new(
            high,
            layout(),
            geometry(),
            fast_timing(),
        ))
        .unwrap();
    controller
        .map_region(low, Address::new(0x0000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    controller
        .map_region(high, Address::new(0x8000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    controller
        .insert_line(low, Address::new(0x1000), line_data(0x10))
        .unwrap();
    controller
        .insert_line(high, Address::new(0x8000), line_data(0x80))
        .unwrap();
    (controller, low, high)
}

#[test]
fn dram_memory_controller_routes_reads_and_returns_data_at_ready_cycle() {
    let (mut controller, low, high) = controller_with_targets();

    let low_outcome = controller.accept(10, &read(0x1004, 4, 1)).unwrap();
    assert_eq!(low_outcome.target(), low);
    assert_eq!(low_outcome.arrival_cycle(), 10);
    assert_eq!(low_outcome.ready_cycle(), 18);
    assert_eq!(low_outcome.dram_access().kind(), DramAccessKind::Read);
    assert_eq!(low_outcome.dram_access().bank(), 0);
    assert_eq!(low_outcome.dram_access().row(), 4);
    assert_eq!(
        low_outcome.response().unwrap().status(),
        ResponseStatus::Completed
    );
    assert_eq!(
        low_outcome.response().unwrap().data().unwrap(),
        &[0x14, 0x15, 0x16, 0x17]
    );

    let high_outcome = controller.accept(0, &read(0x8008, 4, 2)).unwrap();
    assert_eq!(high_outcome.target(), high);
    assert_eq!(high_outcome.ready_cycle(), 6);
    assert_eq!(
        high_outcome.response().unwrap().data().unwrap(),
        &[0x88, 0x89, 0x8a, 0x8b]
    );
}

#[test]
fn dram_memory_controller_applies_writes_and_preserves_target_data() {
    let (mut controller, low, high) = controller_with_targets();

    let write_outcome = controller
        .accept(0, &write(0x1002, &[0xaa, 0xbb, 0xcc, 0xdd], 3))
        .unwrap();
    assert_eq!(write_outcome.target(), low);
    assert_eq!(write_outcome.ready_cycle(), 10);
    assert_eq!(write_outcome.response().unwrap().data(), None);
    assert_eq!(
        &controller.line_data(low, Address::new(0x1000)).unwrap()[..8],
        &[0x10, 0x11, 0xaa, 0xbb, 0xcc, 0xdd, 0x16, 0x17]
    );
    assert_eq!(
        &controller.line_data(high, Address::new(0x8000)).unwrap()[..8],
        &[0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87]
    );

    let read_outcome = controller.accept(10, &read(0x1002, 4, 4)).unwrap();
    assert!(read_outcome.dram_access().row_hit());
    assert_eq!(read_outcome.ready_cycle(), 15);
    assert_eq!(
        read_outcome.response().unwrap().data().unwrap(),
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );
}

#[test]
fn dram_memory_controller_keeps_independent_timing_per_target() {
    let (mut controller, low, high) = controller_with_targets();

    let first = controller.accept(0, &read(0x1000, 8, 5)).unwrap();
    let second = controller.accept(0, &read(0x8000, 8, 6)).unwrap();

    assert_eq!(first.target(), low);
    assert_eq!(first.ready_cycle(), 8);
    assert_eq!(second.target(), high);
    assert_eq!(second.ready_cycle(), 6);
    assert_eq!(controller.target_count(), 2);
    assert_eq!(controller.line_count(low).unwrap(), 1);
    assert_eq!(controller.line_count(high).unwrap(), 1);
}

#[test]
fn dram_memory_controller_reports_target_activity_profiles() {
    let (mut controller, low, high) = controller_with_targets();
    let activity_start = controller.mark_activity();

    controller.accept(0, &read(0x1000, 8, 80)).unwrap();
    controller.accept(1, &read(0x1008, 8, 81)).unwrap();
    controller
        .accept(0, &write(0x8000, &[0xaa, 0xbb, 0xcc, 0xdd], 82))
        .unwrap();

    let low_activity = controller.target_activity(low).unwrap();
    assert_eq!(low_activity.target(), low);
    assert_eq!(low_activity.profile().access_count(), 2);
    assert_eq!(low_activity.profile().read_count(), 2);
    assert_eq!(low_activity.profile().row_hit_count(), 1);
    assert_eq!(low_activity.profile().row_miss_count(), 1);
    assert_eq!(low_activity.profile().command_count(), 3);
    assert_eq!(low_activity.profile().total_ready_latency_cycles(), 20);

    let high_activity = controller.target_activity(high).unwrap();
    assert_eq!(high_activity.target(), high);
    assert_eq!(high_activity.profile().access_count(), 1);
    assert_eq!(high_activity.profile().write_count(), 1);
    assert_eq!(high_activity.profile().row_miss_count(), 1);

    let memory_profile = controller.activity_profile();
    assert_eq!(memory_profile.active_target_count(), 2);
    assert_eq!(memory_profile.access_count(), 3);
    assert_eq!(memory_profile.read_count(), 2);
    assert_eq!(memory_profile.write_count(), 1);
    assert_eq!(memory_profile.row_hit_count(), 1);
    assert_eq!(memory_profile.row_miss_count(), 2);
    assert_eq!(memory_profile.command_count(), 5);
    assert_eq!(memory_profile.active_port_count(), 2);
    assert_eq!(memory_profile.active_bank_count(), 2);
    assert_eq!(memory_profile.total_ready_latency_cycles(), 28);
    assert_eq!(memory_profile.max_ready_latency_cycles(), 12);
    assert_eq!(
        controller.activity_profile_since(&activity_start),
        memory_profile
    );

    let target_windows = controller.target_activities_since(&activity_start);
    assert_eq!(target_windows.len(), 2);
    assert_eq!(target_windows[0].target(), low);
    assert_eq!(target_windows[1].target(), high);
}

#[test]
fn dram_memory_controller_snapshots_and_restores_storage_and_timing_state() {
    let (mut controller, low, high) = controller_with_targets();

    let first = controller.accept(0, &read(0x1000, 8, 70)).unwrap();
    assert_eq!(first.target(), low);
    assert!(!first.dram_access().row_hit());
    assert_eq!(first.ready_cycle(), 8);
    let snapshot = controller.snapshot();

    controller
        .accept(8, &write(0x1000, &[0xaa, 0xbb, 0xcc, 0xdd], 71))
        .unwrap();
    controller.accept(0, &read(0x8000, 8, 72)).unwrap();
    assert_eq!(
        &controller.line_data(low, Address::new(0x1000)).unwrap()[..4],
        &[0xaa, 0xbb, 0xcc, 0xdd]
    );

    controller.restore(&snapshot).unwrap();

    assert_eq!(controller.snapshot(), snapshot);
    assert_eq!(
        &controller.line_data(low, Address::new(0x1000)).unwrap()[..4],
        &[0x10, 0x11, 0x12, 0x13]
    );
    assert_eq!(
        &controller.line_data(high, Address::new(0x8000)).unwrap()[..4],
        &[0x80, 0x81, 0x82, 0x83]
    );
    let bank = controller
        .dram_controller(low)
        .unwrap()
        .bank_state(0)
        .unwrap();
    assert_eq!(bank.open_row(), Some(4));
    assert_eq!(bank.available_cycle(), 8);

    let row_hit = controller.accept(8, &read(0x1008, 4, 73)).unwrap();
    assert!(row_hit.dram_access().row_hit());
    assert_eq!(row_hit.ready_cycle(), 13);
}

#[test]
fn dram_memory_controller_reports_memory_errors_before_timing_mutation() {
    let target = MemoryTargetId::new(3);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            geometry(),
            timing(),
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x4000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();

    assert_eq!(
        controller.accept(0, &read(0x3000, 4, 7)).unwrap_err(),
        DramMemoryError::Memory(MemoryError::UnmappedAddress {
            address: Address::new(0x3000)
        })
    );
    assert_eq!(
        controller.accept(0, &read(0x4040, 4, 8)).unwrap_err(),
        DramMemoryError::Memory(MemoryError::UnmappedLine {
            line: Address::new(0x4040)
        })
    );

    let bank_before = controller
        .dram_controller(target)
        .unwrap()
        .bank_state(1)
        .unwrap();
    assert_eq!(bank_before.open_row(), None);
    assert_eq!(bank_before.available_cycle(), 0);
}

#[test]
fn dram_memory_controller_reports_dram_errors_before_storage_mutation() {
    let target = MemoryTargetId::new(4);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            geometry(),
            timing(),
        ))
        .unwrap();
    controller
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(target, Address::new(0x0000), line_data(0x20))
        .unwrap();

    let actual = CacheLineLayout::new(128).unwrap();
    let request = MemoryRequest::read_shared(
        request_id(9),
        Address::new(0x0000),
        AccessSize::new(8).unwrap(),
        actual,
    )
    .unwrap();
    assert_eq!(
        controller.accept(0, &request).unwrap_err(),
        DramMemoryError::Dram {
            target,
            source: DramError::LineSizeMismatch {
                request: request.id(),
                expected: 64,
                actual: 128,
            },
        }
    );

    assert_eq!(
        &controller.line_data(target, Address::new(0x0000)).unwrap()[..8],
        &[0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27]
    );
}

#[test]
fn dram_memory_controller_rejects_duplicate_and_mismatched_targets() {
    let target = MemoryTargetId::new(5);
    let mut controller = DramMemoryController::new();
    controller
        .add_target(DramControllerConfig::new(
            target,
            layout(),
            geometry(),
            timing(),
        ))
        .unwrap();

    assert_eq!(
        controller
            .add_target(DramControllerConfig::new(
                target,
                layout(),
                geometry(),
                timing()
            ))
            .unwrap_err(),
        DramMemoryError::Memory(MemoryError::DuplicateMemoryTarget { target })
    );

    let bad = MemoryTargetId::new(6);
    assert_eq!(
        controller
            .add_target(DramControllerConfig::new(
                bad,
                layout(),
                DramGeometry::new(4, 256, 128).unwrap(),
                timing(),
            ))
            .unwrap_err(),
        DramMemoryError::TargetLineSizeMismatch {
            target: bad,
            layout: 64,
            geometry: 128,
        }
    );
}
