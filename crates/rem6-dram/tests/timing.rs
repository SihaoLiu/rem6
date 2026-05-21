use rem6_dram::{
    DramAccessKind, DramCommandKind, DramController, DramError, DramGeometry, DramTiming,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(2), sequence)
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

fn write(address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn dram_controller_schedules_closed_row_read_with_activate_latency() {
    let mut controller = DramController::new(geometry(), timing());

    let access = controller.schedule(10, &read(0x0000, 8, 1)).unwrap();

    assert_eq!(access.kind(), DramAccessKind::Read);
    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 0);
    assert!(!access.row_hit());
    assert_eq!(access.arrival_cycle(), 10);
    assert_eq!(access.command_cycle(), 13);
    assert_eq!(access.ready_cycle(), 18);
    assert_eq!(access.commands().len(), 2);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[0].cycle(), 10);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Read);
    assert_eq!(access.commands()[1].cycle(), 13);
}

#[test]
fn dram_controller_keeps_open_row_for_row_hits() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(1, &read(0x0100, 8, 2)).unwrap();

    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 0);
    assert!(access.row_hit());
    assert_eq!(access.command_cycle(), 8);
    assert_eq!(access.ready_cycle(), 13);
    assert_eq!(access.commands().len(), 1);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Read);
}

#[test]
fn dram_controller_precharges_on_row_conflict() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(8, &read(0x0400, 8, 2)).unwrap();

    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 1);
    assert!(!access.row_hit());
    assert_eq!(access.command_cycle(), 13);
    assert_eq!(access.ready_cycle(), 18);
    assert_eq!(access.commands().len(), 3);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Precharge);
    assert_eq!(access.commands()[0].cycle(), 8);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[1].cycle(), 10);
    assert_eq!(access.commands()[2].kind(), DramCommandKind::Read);
    assert_eq!(access.commands()[2].cycle(), 13);
}

#[test]
fn dram_controller_enforces_read_write_turnaround_across_banks() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(0, &write(0x0040, 2)).unwrap();

    assert_eq!(access.kind(), DramAccessKind::Write);
    assert_eq!(access.bank(), 1);
    assert_eq!(access.row(), 0);
    assert_eq!(access.command_cycle(), 7);
    assert_eq!(access.ready_cycle(), 14);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[0].cycle(), 0);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Write);
    assert_eq!(access.commands()[1].cycle(), 7);
}

#[test]
fn dram_controller_reports_bank_port_and_window_activity() {
    let mut controller = DramController::new(geometry(), timing());
    let activity_start = controller.mark_activity();

    controller.schedule(0, &read(0x0000, 8, 10)).unwrap();
    controller.schedule(1, &read(0x0100, 8, 11)).unwrap();
    controller.schedule(0, &write(0x0040, 12)).unwrap();

    let profile = controller.activity_profile();
    assert_eq!(profile.active_port_count(), 1);
    assert_eq!(profile.active_bank_count(), 2);
    assert_eq!(profile.access_count(), 3);
    assert_eq!(profile.read_count(), 2);
    assert_eq!(profile.write_count(), 1);
    assert_eq!(profile.row_hit_count(), 1);
    assert_eq!(profile.row_miss_count(), 2);
    assert_eq!(profile.command_count(), 5);
    assert_eq!(profile.turnaround_count(), 1);
    assert_eq!(profile.total_ready_latency_cycles(), 39);
    assert_eq!(profile.max_ready_latency_cycles(), 19);
    assert!(profile.has_row_misses());
    assert_eq!(controller.activity_profile_since(activity_start), profile);

    let bank0 = controller.bank_activity(0, 0).unwrap();
    assert_eq!(bank0.access_count(), 2);
    assert_eq!(bank0.row_hit_count(), 1);
    assert_eq!(bank0.row_miss_count(), 1);
    assert_eq!(bank0.command_count(), 3);
    assert_eq!(bank0.first_arrival_cycle(), 0);
    assert_eq!(bank0.last_ready_cycle(), 13);
    assert_eq!(bank0.total_ready_latency_cycles(), 20);
    assert_eq!(bank0.max_ready_latency_cycles(), 12);

    let bank1 = controller.bank_activity(0, 1).unwrap();
    assert_eq!(bank1.access_count(), 1);
    assert_eq!(bank1.row_miss_count(), 1);
    assert_eq!(bank1.command_count(), 2);
    assert_eq!(bank1.total_ready_latency_cycles(), 19);

    let port = controller.port_activity(0).unwrap();
    assert_eq!(port.access_count(), 3);
    assert_eq!(port.read_count(), 2);
    assert_eq!(port.write_count(), 1);
    assert_eq!(port.turnaround_count(), 1);
    assert_eq!(port.command_count(), 5);

    controller.clear_activity();
    assert!(controller.activity_profile().is_empty());
}

#[test]
fn dram_controller_rejects_invalid_geometry_and_line_mismatch() {
    assert_eq!(
        DramGeometry::new(0, 256, 64).unwrap_err(),
        DramError::ZeroBankCount
    );
    assert_eq!(
        DramGeometry::new(4, 0, 64).unwrap_err(),
        DramError::ZeroRowSize
    );
    assert_eq!(
        DramGeometry::new(4, 256, 0).unwrap_err(),
        DramError::ZeroLineSize
    );
    assert_eq!(
        DramGeometry::new(4, 96, 64).unwrap_err(),
        DramError::RowSizeNotLineMultiple {
            row_size: 96,
            line_size: 64,
        }
    );

    let mut controller = DramController::new(geometry(), timing());
    let actual = CacheLineLayout::new(128).unwrap();
    let request = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x0000),
        AccessSize::new(8).unwrap(),
        actual,
    )
    .unwrap();
    assert_eq!(
        controller.schedule(0, &request).unwrap_err(),
        DramError::LineSizeMismatch {
            request: request.id(),
            expected: 64,
            actual: 128,
        }
    );
}

#[test]
fn dram_controller_rejects_requests_crossing_decoded_rows() {
    let mut controller = DramController::new(DramGeometry::new(1, 64, 64).unwrap(), timing());
    let request = MemoryRequest::read_shared(
        request_id(4),
        Address::new(0x0030),
        AccessSize::new(64).unwrap(),
        layout(),
    )
    .unwrap();

    assert_eq!(
        controller.schedule(0, &request).unwrap_err(),
        DramError::RequestCrossesRow {
            request: request.id(),
            start_bank: 0,
            start_row: 0,
            end_bank: 0,
            end_row: 1,
        }
    );
}
