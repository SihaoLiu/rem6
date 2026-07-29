use super::super::runtime::pending_store_payload;
use super::*;

const ROOT_SEQUENCE: u64 = 10;
const FIRST_LOAD_SEQUENCE: u64 = 11;
const FIRST_LOAD_PC: u64 = 0x8000;
const ROOT_ADDRESS: u64 = 0x9000;
const ROOT_REGISTER: u8 = 5;
const FIRST_DESTINATION_REGISTER: u8 = 8;
const WAKE_TICK: u64 = 108;
const PUBLISHED_TICK: u64 = 99;

#[derive(Clone, Copy)]
enum Producer {
    Root,
    Previous,
}

#[test]
fn pending_store_v2_fixture_decodes_old_singleton_contract() {
    let fixture = include_bytes!("../../fixtures/pending-store-v2.bin");
    assert_eq!(&fixture[..6], b"O3LC\x02\x02");

    let expected = pending_store_payload();
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(fixture),
        Ok((2, expected.clone())),
    );
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(fixture),
        Ok(expected.clone()),
    );

    let encoded = expected.encode().unwrap();
    assert_eq!(&encoded[..6], b"O3LC\x03\x02");
    assert_ne!(encoded, fixture);
}

#[test]
fn pending_load_graph_v3_round_trips_sibling_rows() {
    assert_graph_round_trip(pending_load_graph(&[Producer::Root, Producer::Root]));
}

#[test]
fn pending_load_graph_v3_round_trips_chain_rows() {
    assert_graph_round_trip(pending_load_graph(&[
        Producer::Root,
        Producer::Previous,
        Producer::Previous,
    ]));
}

#[test]
fn pending_load_graph_v3_round_trips_mixed_rows() {
    assert_graph_round_trip(pending_load_graph(&[
        Producer::Root,
        Producer::Root,
        Producer::Previous,
    ]));
}

#[test]
fn pending_load_graph_v3_rejects_duplicate_destination() {
    assert_bad_load_graph(|value| {
        value.pending_addresses[1].destination = value.pending_addresses[0].destination;
    });
}

#[test]
fn pending_load_graph_v3_rejects_broken_fetch_predecessor_lineage() {
    assert_bad_load_graph(|value| {
        value.pending_addresses[1].fetch_predecessor_request = request(ROOT_SEQUENCE);
    });
}

#[test]
fn pending_load_graph_v3_rejects_internal_dependency_with_wake_authority() {
    assert_bad_load_graph(|value| {
        let row = &mut value.pending_addresses[1];
        row.published_producer_ready_tick = Some(PUBLISHED_TICK);
        row.requested_wake_tick = Some(WAKE_TICK);
    });
}

#[test]
fn pending_load_graph_v3_rejects_root_dependency_without_wake_authority() {
    let mut value = pending_load_graph(&[Producer::Root, Producer::Root]);
    value.pending_addresses[1].published_producer_ready_tick = None;
    value.pending_addresses[1].requested_wake_tick = None;
    assert_bad_pending_value(value);
}

#[test]
fn pending_load_graph_v3_rejects_non_contiguous_pcs() {
    assert_bad_load_graph(|value| {
        let row = &mut value.pending_addresses[1];
        row.fetch = load_fetch(FIRST_LOAD_PC + 12, FIRST_LOAD_SEQUENCE + 1, 8, 9);
    });
}

#[test]
fn pending_load_graph_v3_rejects_mismatched_internal_producer_register() {
    assert_bad_load_graph(|value| {
        value.pending_addresses[1].producer_register = reg(31);
    });
}

#[test]
fn pending_load_graph_v3_rejects_four_rows() {
    let mut value = pending_load_graph(&[Producer::Root, Producer::Previous, Producer::Previous]);
    let mut row = value.pending_addresses[2].clone();
    row.sequence += 1;
    row.fetch = load_fetch(FIRST_LOAD_PC + 12, FIRST_LOAD_SEQUENCE + 3, 10, 11);
    row.consumed_requests = vec![request(FIRST_LOAD_SEQUENCE + 3)];
    row.fetch_predecessor_request = request(FIRST_LOAD_SEQUENCE + 2);
    row.producer_sequence = FIRST_LOAD_SEQUENCE + 2;
    row.producer_register = reg(10);
    row.destination = Some(destination(11, 83));
    value.pending_addresses.push(row);
    value.issue_rows.push(RiscvO3LiveCheckpointIssueRow {
        sequence: FIRST_LOAD_SEQUENCE + 3,
        fetch_request: request(FIRST_LOAD_SEQUENCE + 3),
    });
    value.resident_sequences.push(FIRST_LOAD_SEQUENCE + 3);
    value.service.telemetry.current_occupancy = 4;
    value.service.telemetry.peak_occupancy = 4;
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 16);
    assert_eq!(
        value.encode(),
        Err(RiscvO3LiveCheckpointError::ExcessiveCount {
            field: "pending address rows",
            count: 4,
            maximum: 3,
        }),
    );
}

#[test]
fn pending_load_graph_v3_rejects_multi_row_store() {
    let mut value = pending_store_payload();
    let mut second = value.pending_addresses[0].clone();
    second.sequence += 1;
    second.fetch = store_fetch(FIRST_LOAD_PC + 4, FIRST_LOAD_SEQUENCE + 1);
    second.consumed_requests = vec![request(FIRST_LOAD_SEQUENCE + 1)];
    second.fetch_predecessor_request = value.pending_addresses[0].fetch.request_id();
    value.issue_rows.push(RiscvO3LiveCheckpointIssueRow {
        sequence: second.sequence,
        fetch_request: second.fetch.request_id(),
    });
    value.resident_sequences.push(second.sequence);
    value.pending_addresses.push(second);
    value.service.telemetry.current_occupancy = 2;
    value.service.telemetry.peak_occupancy = 2;
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 8);
    assert_bad_pending_value(value);
}

fn assert_graph_round_trip(expected: RiscvO3LiveCheckpointPayload) {
    let encoded = expected.encode().unwrap();
    assert_eq!(&encoded[..6], b"O3LC\x03\x02");
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(&encoded),
        Ok((3, expected.clone())),
    );
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected));
}

fn assert_bad_load_graph(change: impl FnOnce(&mut RiscvO3LiveCheckpointPayload)) {
    let mut value = pending_load_graph(&[Producer::Root, Producer::Previous, Producer::Root]);
    change(&mut value);
    assert_bad_pending_value(value);
}

fn assert_bad_pending_value(value: RiscvO3LiveCheckpointPayload) {
    assert!(matches!(
        value.encode(),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
    let encoded = value.encode_without_validation_for_test().unwrap();
    assert!(matches!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
}

fn pending_load_graph(linkage: &[Producer]) -> RiscvO3LiveCheckpointPayload {
    assert!(!linkage.is_empty() && linkage.len() <= 3);
    let mut value = compute_payload();
    value.profile = RiscvO3LiveCheckpointProfile::PendingDataAddress;
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 4 * linkage.len() as u64);
    value.next_fetch_request_sequence = FIRST_LOAD_SEQUENCE + linkage.len() as u64;
    value.events.clear();
    value.rename_rows.clear();
    value.executed_fetch_requests.clear();
    value.issued_fetch_requests.clear();
    value.writeback_counted_sequences.clear();
    value.writeback_published_sequences.clear();
    value.reservation = None;
    value.completed_result = None;
    value.service.requested_tick = WAKE_TICK;
    value.service.telemetry.current_occupancy = linkage.len() as u64;
    value.service.telemetry.peak_occupancy = linkage.len() as u64;
    value.wake.tick = WAKE_TICK;
    value.wake.partition = PartitionId::new(2);

    let mut rows: Vec<RiscvO3LiveCheckpointPendingDataAddress> = Vec::with_capacity(linkage.len());
    for (index, producer) in linkage.iter().copied().enumerate() {
        let sequence = FIRST_LOAD_SEQUENCE + index as u64;
        let destination_register = FIRST_DESTINATION_REGISTER + index as u8;
        let (producer_sequence, producer_register, published, requested) = match (index, producer) {
            (0, _) | (_, Producer::Root) => (
                ROOT_SEQUENCE,
                reg(ROOT_REGISTER),
                Some(PUBLISHED_TICK),
                Some(WAKE_TICK),
            ),
            (_, Producer::Previous) => {
                let previous = &rows[index - 1];
                let previous_destination = previous.destination.unwrap();
                (
                    previous.sequence,
                    reg(previous_destination.architectural() as u8),
                    None,
                    None,
                )
            }
        };
        rows.push(RiscvO3LiveCheckpointPendingDataAddress {
            sequence,
            fetch: load_fetch(
                FIRST_LOAD_PC + 4 * index as u64,
                sequence,
                producer_register.index(),
                destination_register,
            ),
            consumed_requests: vec![request(sequence)],
            fetch_predecessor_request: if index == 0 {
                request(ROOT_SEQUENCE)
            } else {
                request(sequence - 1)
            },
            producer_register,
            destination: Some(destination(
                destination_register,
                80 + u32::try_from(index).unwrap(),
            )),
            producer_sequence,
            root_sequence: ROOT_SEQUENCE,
            root_fetch_request: request(ROOT_SEQUENCE),
            root_range: AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap())
                .unwrap(),
            root_atomic: false,
            lsq_kind: O3LoadStoreQueueKind::Load,
            expected_lsq_bytes: 8,
            published_producer_ready_tick: published,
            requested_wake_tick: requested,
        });
    }

    value.issue_rows = rows
        .iter()
        .map(|row| RiscvO3LiveCheckpointIssueRow {
            sequence: row.sequence,
            fetch_request: row.fetch.request_id(),
        })
        .collect();
    value.resident_sequences = rows.iter().map(|row| row.sequence).collect();
    value.pending_addresses = rows;
    value
}

fn destination(architectural: u8, physical: u32) -> O3RenameMapEntry {
    O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        u32::from(architectural),
        O3PhysicalRegisterId::new(physical),
    )
}

fn load_fetch(pc: u64, sequence: u64, rs1: u8, rd: u8) -> CpuFetchEvent {
    let raw = i_type(0, rs1, 3, rd, 0x03);
    completed_fetch(pc, sequence, raw)
}

fn store_fetch(pc: u64, sequence: u64) -> CpuFetchEvent {
    let raw = (6_u32 << 20) | (5_u32 << 15) | (3 << 12) | 0x23;
    completed_fetch(pc, sequence, raw)
}

fn completed_fetch(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    let bytes = raw.to_le_bytes().to_vec();
    let record = CpuFetchRecord::new(
        31 + sequence,
        PartitionId::new(2),
        MemoryRouteId::new(9),
        TransportEndpointId::new("cpu0.ifetch").unwrap(),
        request(sequence),
        Address::new(pc),
        AccessSize::new(bytes.len() as u64).unwrap(),
    );
    CpuFetchEvent::completed(record, bytes)
}
