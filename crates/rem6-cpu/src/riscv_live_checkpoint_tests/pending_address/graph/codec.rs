use super::super::runtime::pending_store_payload;
use super::*;

const ROOT_SEQUENCE: u64 = 10;
const FIRST_LOAD_SEQUENCE: u64 = 11;
const FIRST_LOAD_PC: u64 = 0x8000;
const ROOT_ADDRESS: u64 = 0x9000;
const ROOT_REGISTER: u8 = 5;
const FIRST_DESTINATION_REGISTER: u8 = 6;
const FIRST_DESTINATION_PHYSICAL: u32 = 80;
const PUBLISHED_TICK: u64 = 99;
const WAKE_TICK: u64 = 108;

#[test]
fn pending_store_v2_fixture_decodes_as_one_logical_row() {
    let fixture = include_bytes!("../../fixtures/pending-store-v2.bin");
    assert_eq!(&fixture[..6], b"O3LC\x02\x02");

    let (version, decoded) = RiscvO3LiveCheckpointPayload::decode_versioned(fixture).unwrap();
    assert_eq!(version, 2);
    assert_eq!(decoded.pending_addresses.len(), 1);
    assert_eq!(decoded.pending_addresses[0].destination, None);
    assert_eq!(decoded, pending_store_payload());

    let encoded = decoded.encode().unwrap();
    assert_eq!(&encoded[..6], b"O3LC\x03\x02");
}

#[test]
fn pending_load_graph_v3_round_trips_sibling_chain_and_mixed() {
    for (name, producers) in [
        ("siblings", [5, 5, 5]),
        ("chain", [5, 6, 7]),
        ("mixed", [5, 5, 7]),
    ] {
        let expected = pending_load_graph(producers);
        assert_eq!(
            expected
                .pending_addresses
                .iter()
                .map(|pending| pending.producer_register.index())
                .collect::<Vec<_>>(),
            producers,
            "{name}",
        );
        assert_graph_round_trip(expected);
    }
}

#[test]
fn pending_load_graph_v3_rejects_malformed_graph_cases() {
    let cases = [
        MalformedCase::new("zero rows", zero_rows, ExpectedMalformed::InvalidShape),
        MalformedCase::new("four rows", four_rows, ExpectedMalformed::ExcessiveCount),
        MalformedCase::new(
            "duplicate sequence",
            duplicate_sequence,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "duplicate fetch",
            duplicate_fetch,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "unordered PC",
            unordered_pc,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "broken predecessor",
            broken_predecessor,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "multi-row store",
            multi_row_store,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "missing load destination",
            missing_load_destination,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "duplicate destination identity",
            duplicate_destination_identity,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "duplicate architectural destination",
            duplicate_architectural_destination,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "duplicate physical destination",
            duplicate_physical_destination,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "destination overwrites root source",
            destination_overwrites_root_source,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "destination overwrites older destination",
            destination_overwrites_older_destination,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "nonadjacent producer",
            nonadjacent_producer,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "wrong source register",
            wrong_source_register,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "self root sequence",
            self_root_sequence,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "future root sequence",
            future_root_sequence,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "publication after capture",
            publication_after_capture,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "wake before capture",
            wake_before_capture,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "wake before publication",
            wake_before_publication,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "missing root wake",
            missing_root_wake,
            ExpectedMalformed::InvalidShape,
        ),
        MalformedCase::new(
            "internal wake present",
            internal_wake_present,
            ExpectedMalformed::InvalidShape,
        ),
    ];

    for case in cases {
        assert_malformed(case);
    }
}

struct MalformedCase {
    name: &'static str,
    build: fn() -> RiscvO3LiveCheckpointPayload,
    expected: ExpectedMalformed,
}

impl MalformedCase {
    fn new(
        name: &'static str,
        build: fn() -> RiscvO3LiveCheckpointPayload,
        expected: ExpectedMalformed,
    ) -> Self {
        Self {
            name,
            build,
            expected,
        }
    }
}

#[derive(Clone, Copy)]
enum ExpectedMalformed {
    InvalidShape,
    ExcessiveCount,
}

fn assert_malformed(case: MalformedCase) {
    let value = (case.build)();
    match case.expected {
        ExpectedMalformed::InvalidShape => {
            assert!(
                matches!(
                    value.encode(),
                    Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
                ),
                "{} encoded successfully",
                case.name,
            );
            let encoded = value.encode_without_validation_for_test().unwrap();
            assert!(
                matches!(
                    RiscvO3LiveCheckpointPayload::decode(&encoded),
                    Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
                ),
                "{} decoded successfully",
                case.name,
            );
        }
        ExpectedMalformed::ExcessiveCount => {
            let expected = RiscvO3LiveCheckpointError::ExcessiveCount {
                field: "pending address rows",
                count: 4,
                maximum: 3,
            };
            assert_eq!(value.encode(), Err(expected.clone()), "{}", case.name);
            assert_eq!(
                value.encode_without_validation_for_test(),
                Err(expected),
                "{}",
                case.name,
            );
        }
    }
}

fn zero_rows() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses.clear();
    sync_rows(&mut value);
    value.service.telemetry.peak_occupancy = 0;
    value
}

fn four_rows() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 6, 7]);
    let sequence = FIRST_LOAD_SEQUENCE + 3;
    let fetch = load_fetch(
        FIRST_LOAD_PC + 12,
        sequence,
        FIRST_DESTINATION_REGISTER + 2,
        FIRST_DESTINATION_REGISTER + 3,
    );
    value
        .pending_addresses
        .push(RiscvO3LiveCheckpointPendingDataAddress {
            sequence,
            fetch: fetch.clone(),
            consumed_requests: vec![fetch.request_id()],
            fetch_predecessor_request: value.pending_addresses[2].fetch.request_id(),
            producer_register: reg(FIRST_DESTINATION_REGISTER + 2),
            destination: Some(destination(
                FIRST_DESTINATION_REGISTER + 3,
                FIRST_DESTINATION_PHYSICAL + 3,
            )),
            producer_sequence: FIRST_LOAD_SEQUENCE + 2,
            root_sequence: ROOT_SEQUENCE,
            root_fetch_request: request(ROOT_SEQUENCE),
            root_range: root_range(),
            root_atomic: false,
            lsq_kind: O3LoadStoreQueueKind::Load,
            expected_lsq_bytes: 8,
            published_producer_ready_tick: None,
            requested_wake_tick: None,
        });
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 16);
    value.next_fetch_request_sequence = FIRST_LOAD_SEQUENCE + 4;
    sync_rows(&mut value);
    value
}

fn duplicate_sequence() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].sequence = value.pending_addresses[0].sequence;
    sync_rows(&mut value);
    value
}

fn duplicate_fetch() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    set_fetch(&mut value, 1, FIRST_LOAD_PC + 4, FIRST_LOAD_SEQUENCE, 5, 7);
    value
}

fn unordered_pc() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    set_fetch(
        &mut value,
        1,
        FIRST_LOAD_PC + 12,
        FIRST_LOAD_SEQUENCE + 1,
        5,
        7,
    );
    value
}

fn broken_predecessor() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[2].fetch_predecessor_request =
        value.pending_addresses[0].fetch.request_id();
    value
}

fn multi_row_store() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_store_payload();
    let sequence = FIRST_LOAD_SEQUENCE + 1;
    let fetch = store_fetch(FIRST_LOAD_PC + 4, sequence);
    value
        .pending_addresses
        .push(RiscvO3LiveCheckpointPendingDataAddress {
            sequence,
            fetch: fetch.clone(),
            consumed_requests: vec![fetch.request_id()],
            fetch_predecessor_request: value.pending_addresses[0].fetch.request_id(),
            producer_register: reg(ROOT_REGISTER),
            destination: None,
            producer_sequence: ROOT_SEQUENCE,
            root_sequence: ROOT_SEQUENCE,
            root_fetch_request: request(ROOT_SEQUENCE),
            root_range: root_range(),
            root_atomic: false,
            lsq_kind: O3LoadStoreQueueKind::Store,
            expected_lsq_bytes: 8,
            published_producer_ready_tick: Some(PUBLISHED_TICK),
            requested_wake_tick: Some(WAKE_TICK),
        });
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 8);
    value.next_fetch_request_sequence = FIRST_LOAD_SEQUENCE + 2;
    sync_rows(&mut value);
    value
}

fn missing_load_destination() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].destination = None;
    value
}

fn duplicate_destination_identity() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    let older = value.pending_addresses[0].destination.unwrap();
    set_destination_and_rd(
        &mut value,
        1,
        older.architectural() as u8,
        older.physical().get(),
    );
    value
}

fn duplicate_architectural_destination() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    let older = value.pending_addresses[0].destination.unwrap();
    set_destination_and_rd(
        &mut value,
        1,
        older.architectural() as u8,
        FIRST_DESTINATION_PHYSICAL + 1,
    );
    value
}

fn duplicate_physical_destination() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    let older = value.pending_addresses[0].destination.unwrap();
    set_destination_and_rd(
        &mut value,
        1,
        FIRST_DESTINATION_REGISTER + 1,
        older.physical().get(),
    );
    value
}

fn destination_overwrites_root_source() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    set_destination_and_rd(&mut value, 0, ROOT_REGISTER, FIRST_DESTINATION_PHYSICAL);
    value
}

fn destination_overwrites_older_destination() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    let older = value.pending_addresses[0].destination.unwrap();
    set_destination_and_rd(
        &mut value,
        2,
        older.architectural() as u8,
        FIRST_DESTINATION_PHYSICAL + 2,
    );
    value
}

fn nonadjacent_producer() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 6, 7]);
    value.pending_addresses[2].producer_sequence = value.pending_addresses[0].sequence;
    set_fetch(
        &mut value,
        2,
        FIRST_LOAD_PC + 8,
        FIRST_LOAD_SEQUENCE + 2,
        6,
        8,
    );
    value.pending_addresses[2].producer_register = reg(6);
    value
}

fn wrong_source_register() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].producer_register = reg(6);
    set_fetch(
        &mut value,
        1,
        FIRST_LOAD_PC + 4,
        FIRST_LOAD_SEQUENCE + 1,
        6,
        7,
    );
    value
}

fn self_root_sequence() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    set_common_root_sequence(&mut value, FIRST_LOAD_SEQUENCE);
    value
}

fn future_root_sequence() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    set_common_root_sequence(&mut value, FIRST_LOAD_SEQUENCE + 3);
    value
}

fn publication_after_capture() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].published_producer_ready_tick = Some(value.captured_tick + 1);
    value
}

fn wake_before_capture() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].requested_wake_tick = Some(value.captured_tick - 1);
    value
}

fn wake_before_publication() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].requested_wake_tick = Some(
        value.pending_addresses[1]
            .published_producer_ready_tick
            .unwrap()
            - 1,
    );
    value
}

fn missing_root_wake() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 5, 5]);
    value.pending_addresses[1].published_producer_ready_tick = None;
    value.pending_addresses[1].requested_wake_tick = None;
    value
}

fn internal_wake_present() -> RiscvO3LiveCheckpointPayload {
    let mut value = pending_load_graph([5, 6, 7]);
    value.pending_addresses[1].published_producer_ready_tick = Some(PUBLISHED_TICK);
    value.pending_addresses[1].requested_wake_tick = Some(WAKE_TICK);
    value
}

fn pending_load_graph(producers: [u8; 3]) -> RiscvO3LiveCheckpointPayload {
    let mut value = compute_payload();
    value.profile = RiscvO3LiveCheckpointProfile::PendingDataAddress;
    value.captured_tick = 100;
    value.next_fetch_pc = Address::new(FIRST_LOAD_PC + 12);
    value.next_fetch_request_sequence = FIRST_LOAD_SEQUENCE + 3;
    value.events.clear();
    value.rename_rows.clear();
    value.executed_fetch_requests.clear();
    value.issued_fetch_requests.clear();
    value.writeback_counted_sequences.clear();
    value.writeback_published_sequences.clear();
    value.reservation = None;
    value.completed_result = None;
    value.service.requested_tick = WAKE_TICK;
    value.service.telemetry.current_occupancy = 3;
    value.service.telemetry.peak_occupancy = 3;
    value.wake.tick = WAKE_TICK;
    value.wake.partition = PartitionId::new(2);
    value.finalized_writeback.partial_cycle_ticks.clear();
    value.finalized_writeback.partial_ready_rows_by_tick.clear();
    value
        .finalized_writeback
        .partial_deferred_rows_by_tick
        .clear();
    value.finalized_writeback.closed_before_tick = value.captured_tick;

    value.pending_addresses = producers
        .into_iter()
        .enumerate()
        .map(|(index, producer)| load_row(index, producer))
        .collect();
    sync_rows(&mut value);
    value
}

fn load_row(index: usize, producer: u8) -> RiscvO3LiveCheckpointPendingDataAddress {
    let sequence = FIRST_LOAD_SEQUENCE + index as u64;
    let destination_register = FIRST_DESTINATION_REGISTER + index as u8;
    let producer_sequence = if producer == ROOT_REGISTER {
        ROOT_SEQUENCE
    } else {
        assert_eq!(producer, destination_register - 1);
        sequence - 1
    };
    let fetch = load_fetch(
        FIRST_LOAD_PC + 4 * index as u64,
        sequence,
        producer,
        destination_register,
    );
    let root_dependent = producer_sequence == ROOT_SEQUENCE;
    RiscvO3LiveCheckpointPendingDataAddress {
        sequence,
        fetch: fetch.clone(),
        consumed_requests: vec![fetch.request_id()],
        fetch_predecessor_request: if index == 0 {
            request(ROOT_SEQUENCE)
        } else {
            request(FIRST_LOAD_SEQUENCE + index as u64 - 1)
        },
        producer_register: reg(producer),
        destination: Some(destination(
            destination_register,
            FIRST_DESTINATION_PHYSICAL + index as u32,
        )),
        producer_sequence,
        root_sequence: ROOT_SEQUENCE,
        root_fetch_request: request(ROOT_SEQUENCE),
        root_range: root_range(),
        root_atomic: false,
        lsq_kind: O3LoadStoreQueueKind::Load,
        expected_lsq_bytes: 8,
        published_producer_ready_tick: root_dependent.then_some(PUBLISHED_TICK),
        requested_wake_tick: root_dependent.then_some(WAKE_TICK),
    }
}

fn set_destination_and_rd(
    value: &mut RiscvO3LiveCheckpointPayload,
    index: usize,
    architectural: u8,
    physical: u32,
) {
    value.pending_addresses[index].destination = Some(destination(architectural, physical));
    let pending = &value.pending_addresses[index];
    set_fetch(
        value,
        index,
        pending.fetch.pc().get(),
        pending.fetch.request_id().sequence(),
        pending.producer_register.index(),
        architectural,
    );
}

fn set_fetch(
    value: &mut RiscvO3LiveCheckpointPayload,
    index: usize,
    pc: u64,
    sequence: u64,
    producer: u8,
    destination: u8,
) {
    let fetch = load_fetch(pc, sequence, producer, destination);
    value.pending_addresses[index].fetch = fetch.clone();
    value.pending_addresses[index].consumed_requests = vec![fetch.request_id()];
    sync_rows(value);
}

fn set_common_root_sequence(value: &mut RiscvO3LiveCheckpointPayload, root_sequence: u64) {
    for pending in &mut value.pending_addresses {
        if pending.producer_sequence == pending.root_sequence {
            pending.producer_sequence = root_sequence;
        }
        pending.root_sequence = root_sequence;
    }
}

fn sync_rows(value: &mut RiscvO3LiveCheckpointPayload) {
    value.issue_rows = value
        .pending_addresses
        .iter()
        .map(|pending| RiscvO3LiveCheckpointIssueRow {
            sequence: pending.sequence,
            fetch_request: pending.fetch.request_id(),
        })
        .collect();
    value.resident_sequences = value
        .pending_addresses
        .iter()
        .map(|pending| pending.sequence)
        .collect();
    value.service.telemetry.current_occupancy = value.pending_addresses.len() as u64;
    value.service.telemetry.peak_occupancy = value
        .service
        .telemetry
        .peak_occupancy
        .max(value.pending_addresses.len() as u64);
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

fn destination(architectural: u8, physical: u32) -> O3RenameMapEntry {
    O3RenameMapEntry::new(
        O3RegisterClass::Integer,
        u32::from(architectural),
        O3PhysicalRegisterId::new(physical),
    )
}

fn root_range() -> AddressRange {
    AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap()).unwrap()
}

fn load_fetch(pc: u64, sequence: u64, producer: u8, destination: u8) -> CpuFetchEvent {
    completed_fetch(pc, sequence, i_type(0, producer, 3, destination, 0x03))
}

fn store_fetch(pc: u64, sequence: u64) -> CpuFetchEvent {
    completed_fetch(
        pc,
        sequence,
        store(6, ROOT_REGISTER, MemoryWidth::Doubleword),
    )
}

fn completed_fetch(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    let bytes = raw.to_le_bytes().to_vec();
    let record = CpuFetchRecord::new(
        31,
        PartitionId::new(2),
        MemoryRouteId::new(9),
        TransportEndpointId::new("cpu0.ifetch").unwrap(),
        request(sequence),
        Address::new(pc),
        AccessSize::new(bytes.len() as u64).unwrap(),
    );
    CpuFetchEvent::completed(record, bytes)
}

fn store(rs2: u8, rs1: u8, width: MemoryWidth) -> u32 {
    let funct3 = match width {
        MemoryWidth::Word => 2,
        MemoryWidth::Doubleword => 3,
        _ => unreachable!(),
    };
    (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | 0x23
}
