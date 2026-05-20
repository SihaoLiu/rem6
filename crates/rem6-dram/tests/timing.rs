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
