use super::*;

fn request(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn store_entry(
    sequence: u64,
    address: u64,
    bytes: u32,
    ownership: RiscvO3LiveDataHandoffOwnership,
) -> RiscvO3LiveDataHandoffEntry {
    RiscvO3LiveDataHandoffEntry {
        fetch_request: request(3, sequence),
        data_request: request(4, sequence + 10),
        issue_tick: 20 + sequence,
        partition: PartitionId::new(2),
        operation: RiscvO3LiveDataHandoffOperation::Store,
        ownership,
        target: RiscvO3LiveDataHandoffTarget::Memory {
            route: MemoryRouteId::new(7),
        },
        address: Address::new(address),
        bytes,
        o3_sequence: sequence,
        trace_sequence: Some(sequence + 100),
    }
}

fn load_entry(sequence: u64, address: u64, bytes: u32) -> RiscvO3LiveDataHandoffEntry {
    RiscvO3LiveDataHandoffEntry {
        operation: RiscvO3LiveDataHandoffOperation::Load,
        ownership: RiscvO3LiveDataHandoffOwnership::Transport,
        address: Address::new(address),
        bytes,
        ..store_entry(
            sequence,
            address,
            bytes,
            RiscvO3LiveDataHandoffOwnership::Transport,
        )
    }
}

fn source(
    entry: RiscvO3LiveDataHandoffEntry,
    ownership_mask: u8,
    source_data: [u8; 8],
) -> RiscvO3LiveDataHandoffPartialOverlaySource {
    RiscvO3LiveDataHandoffPartialOverlaySource {
        source_data_request: entry.data_request,
        source_address: entry.address,
        source_bytes: entry.bytes,
        ownership_mask,
        source_data,
    }
}

fn completed_overlay(
    middle: RiscvO3LiveDataHandoffEntry,
    youngest: RiscvO3LiveDataHandoffEntry,
) -> RiscvO3LiveDataHandoffCompletedPartialOverlay {
    RiscvO3LiveDataHandoffCompletedPartialOverlay {
        fetch_request: request(3, 8),
        load_data_request: request(4, 18),
        issue_tick: 28,
        response_tick: 48,
        address: Address::new(0x8000_0100),
        bytes: 8,
        original_forwarded_mask: 0x0f,
        live_forwarded_mask: 0x0c,
        data: [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88],
        o3_sequence: 8,
        trace_sequence: Some(108),
        sources: vec![
            source(middle, 0x08, [0x00, 0x06, 0, 0, 0, 0, 0, 0]),
            source(youngest, 0x04, [0xdd, 0, 0, 0, 0, 0, 0, 0]),
        ],
    }
}

fn issued_store(entry: RiscvO3LiveDataHandoffEntry, value: u64) -> RiscvIssuedScalarMemoryHandoff {
    RiscvIssuedScalarMemoryHandoff {
        fetch_request: entry.fetch_request,
        data_request: entry.data_request,
        issue_tick: entry.issue_tick,
        partition: entry.partition,
        operation: entry.operation,
        target: entry.target,
        address: entry.address,
        bytes: entry.bytes,
        store_data: Some(value.to_le_bytes()),
        partial_overlay: None,
    }
}

fn completed_partial_row() -> RiscvCompletedPartialScalarLoadHandoff {
    RiscvCompletedPartialScalarLoadHandoff {
        fetch_request: request(3, 8),
        data_request: request(4, 18),
        issue_tick: 28,
        response_tick: 48,
        address: Address::new(0x8000_0100),
        bytes: 8,
        original_forwarded_mask: 0x0f,
        data: [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88],
        o3_sequence: 8,
        trace_sequence: Some(108),
    }
}

fn representative_completed_partial_parts() -> (
    Vec<RiscvO3LiveDataHandoffEntry>,
    RiscvO3LiveDataHandoffCompletedPartialOverlay,
) {
    let middle = store_entry(
        6,
        0x8000_0102,
        2,
        RiscvO3LiveDataHandoffOwnership::Transport,
    );
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request(),
        },
    );
    (vec![middle, youngest], completed_overlay(middle, youngest))
}

fn representative_completed_partial_handoff() -> RiscvO3LiveDataHandoff {
    let (entries, overlay) = representative_completed_partial_parts();
    RiscvO3LiveDataHandoff::with_completed_partial_overlay(entries, overlay, 0)
        .expect("representative completed partial handoff")
}

fn v6_multi_source_pending_handoff() -> RiscvO3LiveDataHandoff {
    let middle = store_entry(
        6,
        0x8000_0102,
        2,
        RiscvO3LiveDataHandoffOwnership::Transport,
    );
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request(),
        },
    );
    let load = load_entry(8, 0x8000_0100, 8);
    let overlay = RiscvO3LiveDataHandoffPartialOverlay {
        load_data_request: load.data_request,
        address: load.address,
        bytes: load.bytes,
        forwarded_mask: 0x0c,
        data: [0, 0, 0xdd, 0x06, 0, 0, 0, 0],
        sources: vec![
            source(middle, 0x08, [0x00, 0x06, 0, 0, 0, 0, 0, 0]),
            source(youngest, 0x04, [0xdd, 0, 0, 0, 0, 0, 0, 0]),
        ],
    };
    RiscvO3LiveDataHandoff::with_partial_overlay(vec![middle, youngest, load], overlay, 0)
        .expect("version-6 multi-source pending handoff")
}

const V7_HEADER_BYTES: usize = HEADER_BYTES + 12;
const V7_MEMORY_ENTRY_BYTES: usize = V1_ENTRY_BYTES + 15;
const V7_COMPLETED_RECORD_OFFSET: usize = V7_HEADER_BYTES + 2 * V7_MEMORY_ENTRY_BYTES;
const COMPLETED_ISSUE_TICK_OFFSET: usize = 24;
const COMPLETED_RESPONSE_TICK_OFFSET: usize = 32;
const COMPLETED_BYTES_OFFSET: usize = 48;
const COMPLETED_ORIGINAL_MASK_OFFSET: usize = 52;
const COMPLETED_LIVE_MASK_OFFSET: usize = 53;
const COMPLETED_DATA_OFFSET: usize = 54;
const COMPLETED_O3_SEQUENCE_OFFSET: usize = 62;
const COMPLETED_SOURCE_RECORDS_OFFSET: usize = 83;
const COMPLETED_SOURCE_RECORD_BYTES: usize = 21;
const COMPLETED_SOURCE_OWNERSHIP_OFFSET: usize = 12;

fn completed_v7_payload() -> Vec<u8> {
    representative_completed_partial_handoff().encode()
}

fn write_u64(payload: &mut [u8], offset: usize, value: u64) {
    payload[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn completed_partial_handoff_is_rejected(
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    overlay: RiscvO3LiveDataHandoffCompletedPartialOverlay,
) -> bool {
    RiscvO3LiveDataHandoff::with_completed_partial_overlay(entries, overlay, 0).is_none()
}

fn forged_completed_partial_handoff(
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    overlay: RiscvO3LiveDataHandoffCompletedPartialOverlay,
) -> RiscvO3LiveDataHandoff {
    RiscvO3LiveDataHandoff {
        entries,
        forwarded_rows: Vec::new(),
        partial_overlays: Vec::new(),
        completed_partial_overlays: vec![overlay],
        younger_rows: 0,
    }
}

#[test]
fn completed_partial_overlay_tracks_original_live_and_retired_masks() {
    let (entries, completed) = representative_completed_partial_parts();

    let handoff = RiscvO3LiveDataHandoff::with_completed_partial_overlay(entries, completed, 0)
        .expect("representative completed overlay");

    assert_eq!(handoff.resident_rows(), 3);
    assert_eq!(handoff.completed_partial_overlays().len(), 1);
    let row = &handoff.completed_partial_overlays()[0];
    assert_eq!(row.original_forwarded_mask(), 0x0f);
    assert_eq!(row.original_response_mask(), 0xf0);
    assert_eq!(row.live_forwarded_mask(), 0x0c);
    assert_eq!(row.retired_forwarded_mask(), 0x03);
    assert_eq!(row.data(), [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88]);
}

#[test]
fn completed_partial_overlay_recomposes_live_sources_in_o3_order() {
    let middle = store_entry(
        6,
        0x8000_0102,
        2,
        RiscvO3LiveDataHandoffOwnership::Transport,
    );
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request(),
        },
    );
    let (live_mask, sources) = compose_completed_partial_overlay_sources(
        &[issued_store(middle, 0x06bb), issued_store(youngest, 0xdd)],
        Address::new(0x8000_0100),
        8,
        0x0f,
        &[0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88],
    )
    .expect("live sources should preserve youngest-store byte ownership");

    assert_eq!(live_mask, 0x0c);
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].source_data_request(), middle.data_request());
    assert_eq!(sources[0].ownership_mask(), 0x08);
    assert_eq!(sources[0].source_data(), &[0x00, 0x06]);
    assert_eq!(sources[1].source_data_request(), youngest.data_request());
    assert_eq!(sources[1].ownership_mask(), 0x04);
    assert_eq!(sources[1].source_data(), &[0xdd]);
}

#[test]
fn completed_partial_overlay_capture_uses_o3_entry_order() {
    let middle = store_entry(
        6,
        0x8000_0102,
        2,
        RiscvO3LiveDataHandoffOwnership::Transport,
    );
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request(),
        },
    );
    let handoff = build_completed_partial_overlay_handoff(
        vec![middle, youngest],
        &[issued_store(youngest, 0xdd), issued_store(middle, 0x06bb)],
        &[],
        &[completed_partial_row()],
        0,
    )
    .expect("completed overlay capture should use O3-sorted entries");

    let row = &handoff.completed_partial_overlays()[0];
    assert_eq!(row.live_forwarded_mask(), 0x0c);
    assert_eq!(
        row.sources()[0].source_data_request(),
        middle.data_request()
    );
    assert_eq!(row.sources()[0].ownership_mask(), 0x08);
    assert_eq!(
        row.sources()[1].source_data_request(),
        youngest.data_request()
    );
    assert_eq!(row.sources()[1].ownership_mask(), 0x04);
}

#[test]
fn completed_partial_overlay_rejects_invalid_original_and_live_masks() {
    for (original_forwarded_mask, live_forwarded_mask) in [
        (0, 0x0c),
        (0xff, 0x0c),
        (0x0f, 0),
        (0x0f, 0xff),
        (0x0c, 0x0e),
    ] {
        let (entries, mut overlay) = representative_completed_partial_parts();
        overlay.original_forwarded_mask = original_forwarded_mask;
        overlay.live_forwarded_mask = live_forwarded_mask;
        assert!(
            completed_partial_handoff_is_rejected(entries, overlay),
            "accepted original mask {original_forwarded_mask:#04x} and live mask {live_forwarded_mask:#04x}"
        );
    }
}

#[test]
fn completed_partial_overlay_rejects_invalid_source_ownership() {
    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.sources[0].ownership_mask = 0;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));

    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.sources[0].ownership_mask = 0x02;
    overlay.live_forwarded_mask = 0x06;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));

    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.live_forwarded_mask = 0x0e;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));

    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.sources[0].ownership_mask = 0x0c;
    overlay.sources[0].source_data = [0xdd, 0x06, 0, 0, 0, 0, 0, 0];
    assert!(completed_partial_handoff_is_rejected(entries, overlay));
}

#[test]
fn completed_partial_overlay_rejects_noncanonical_live_source_provenance() {
    let (mut outside_entries, mut outside_overlay) = representative_completed_partial_parts();
    outside_entries[0].address = Address::new(0x8000_0100);
    outside_entries[0].bytes = 4;
    outside_overlay.sources[0].source_address = outside_entries[0].address;
    outside_overlay.sources[0].source_bytes = outside_entries[0].bytes;
    outside_overlay.sources[0].source_data = [0, 0, 0, 0x06, 0, 0, 0, 0];

    let (mut precedence_entries, mut precedence_overlay) = representative_completed_partial_parts();
    precedence_entries[1].address = Address::new(0x8000_0101);
    precedence_entries[1].bytes = 2;
    precedence_overlay.live_forwarded_mask = 0x0e;
    precedence_overlay.sources[0].ownership_mask = 0x0c;
    precedence_overlay.sources[0].source_data = [0xdd, 0x06, 0, 0, 0, 0, 0, 0];
    precedence_overlay.sources[1].source_address = precedence_entries[1].address;
    precedence_overlay.sources[1].source_bytes = precedence_entries[1].bytes;
    precedence_overlay.sources[1].ownership_mask = 0x02;
    precedence_overlay.sources[1].source_data = [0; 8];

    for (entries, overlay) in [
        (outside_entries, outside_overlay),
        (precedence_entries, precedence_overlay),
    ] {
        assert!(completed_partial_handoff_is_rejected(
            entries.clone(),
            overlay.clone()
        ));
        let payload = forged_completed_partial_handoff(entries, overlay).encode();
        let decoded = RiscvO3LiveDataHandoff::decode(&payload);
        assert!(
            matches!(
                decoded,
                Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape { .. })
            ),
            "unexpected forged completed-overlay decode result: {decoded:?}"
        );
    }
}

#[test]
fn completed_partial_overlay_rejects_invalid_sequence_and_timing() {
    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.o3_sequence = 7;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));

    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.response_tick = overlay.issue_tick - 1;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));
}

#[test]
fn completed_partial_overlay_rejects_nonzero_data_padding() {
    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.bytes = 4;
    overlay.original_forwarded_mask = 0x0e;
    overlay.data[4] = 1;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));

    let (entries, mut overlay) = representative_completed_partial_parts();
    overlay.sources[0].source_data[2] = 1;
    assert!(completed_partial_handoff_is_rejected(entries, overlay));
}

#[test]
fn live_data_handoff_round_trips_completed_partial_overlay_v7() {
    let handoff = representative_completed_partial_handoff();
    let payload = handoff.encode();
    assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
    let (decoded, version) = RiscvO3LiveDataHandoff::decode_with_version(&payload).unwrap();
    assert_eq!(version, 7);
    assert_eq!(decoded, handoff);
}

#[test]
fn live_data_handoff_decodes_v6_multi_source_partial_overlay() {
    let handoff = v6_multi_source_pending_handoff();
    let (decoded, version) =
        RiscvO3LiveDataHandoff::decode_with_version(LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD)
            .unwrap();
    assert_eq!(version, VERSION_MULTI_SOURCE_CURRENT);
    assert_eq!(decoded, handoff);
    assert_eq!(decoded.entries().len(), 3);
    assert_eq!(decoded.partial_overlays().len(), 1);
    assert_eq!(decoded.partial_overlays()[0].sources().len(), 2);
    assert_eq!(decoded.partial_overlays()[0].forwarded_mask(), 0x0c);
    assert!(decoded.completed_partial_overlays().is_empty());

    let current = decoded.encode();
    assert_eq!(current[MAGIC.len()], VERSION_CURRENT);
    assert_ne!(current.as_slice(), LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD);
    assert_eq!(
        RiscvO3LiveDataHandoff::decode_with_version(&current),
        Ok((handoff, VERSION_CURRENT))
    );
}

#[test]
fn completed_partial_overlay_v7_rejects_invalid_masks() {
    for (original, expected) in [
        (
            0,
            RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask { mask: 0, bytes: 8 },
        ),
        (
            0xff,
            RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask {
                mask: 0xff,
                bytes: 8,
            },
        ),
    ] {
        let mut payload = completed_v7_payload();
        payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_ORIGINAL_MASK_OFFSET] = original;
        assert_eq!(RiscvO3LiveDataHandoff::decode(&payload), Err(expected));
    }

    for (original, live) in [(0x0f, 0), (0x0f, 0xff), (0x0c, 0x0e)] {
        let mut payload = completed_v7_payload();
        payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_ORIGINAL_MASK_OFFSET] = original;
        payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_LIVE_MASK_OFFSET] = live;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Err(
                RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlayLiveMask {
                    original,
                    live,
                }
            )
        );
    }
}

#[test]
fn completed_partial_overlay_v7_rejects_invalid_timing_padding_and_identity() {
    let mut payload = completed_v7_payload();
    let issue_offset = V7_COMPLETED_RECORD_OFFSET + COMPLETED_ISSUE_TICK_OFFSET;
    let response_offset = V7_COMPLETED_RECORD_OFFSET + COMPLETED_RESPONSE_TICK_OFFSET;
    write_u64(&mut payload, response_offset, 27);
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&payload),
        Err(RiscvO3LiveDataHandoffError::ForwardedResponseBeforeIssue {
            issue_tick: 28,
            response_tick: 27,
        })
    );
    assert_eq!(
        u64::from_le_bytes(payload[issue_offset..issue_offset + 8].try_into().unwrap()),
        28
    );

    let mut payload = completed_v7_payload();
    payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_BYTES_OFFSET
        ..V7_COMPLETED_RECORD_OFFSET + COMPLETED_BYTES_OFFSET + 4]
        .copy_from_slice(&4_u32.to_le_bytes());
    payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_ORIGINAL_MASK_OFFSET] = 0x0e;
    payload[V7_COMPLETED_RECORD_OFFSET + COMPLETED_DATA_OFFSET + 4] = 1;
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&payload),
        Err(RiscvO3LiveDataHandoffError::NonZeroForwardedDataPadding { index: 4, value: 1 })
    );

    let mut payload = completed_v7_payload();
    let first_fetch: [u8; 12] = payload[V7_HEADER_BYTES..V7_HEADER_BYTES + 12]
        .try_into()
        .unwrap();
    payload[V7_COMPLETED_RECORD_OFFSET..V7_COMPLETED_RECORD_OFFSET + 12]
        .copy_from_slice(&first_fetch);
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&payload),
        Err(RiscvO3LiveDataHandoffError::DuplicateFetchRequest {
            request: request(3, 6),
        })
    );
}

#[test]
fn completed_partial_overlay_v7_rejects_non_younger_load_sequence() {
    let mut payload = completed_v7_payload();
    write_u64(
        &mut payload,
        V7_COMPLETED_RECORD_OFFSET + COMPLETED_O3_SEQUENCE_OFFSET,
        7,
    );
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&payload),
        Err(
            RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlaySequence {
                source: 7,
                load: 7,
            }
        )
    );
}

#[test]
fn completed_partial_overlay_v7_rejects_incomplete_and_overlapping_ownership() {
    let mut incomplete = completed_v7_payload();
    incomplete[V7_COMPLETED_RECORD_OFFSET + COMPLETED_LIVE_MASK_OFFSET] = 0x0e;
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&incomplete),
        Err(
            RiscvO3LiveDataHandoffError::IncompletePartialOverlayOwnership {
                expected: 0x0e,
                actual: 0x0c,
            }
        )
    );

    let mut overlapping = completed_v7_payload();
    let first_ownership = V7_COMPLETED_RECORD_OFFSET
        + COMPLETED_SOURCE_RECORDS_OFFSET
        + COMPLETED_SOURCE_OWNERSHIP_OFFSET;
    let second_ownership = first_ownership + COMPLETED_SOURCE_RECORD_BYTES;
    overlapping[first_ownership] = 0x0c;
    overlapping[first_ownership + 1] = 0xdd;
    assert_eq!(overlapping[second_ownership], 0x04);
    assert_eq!(
        RiscvO3LiveDataHandoff::decode(&overlapping),
        Err(RiscvO3LiveDataHandoffError::OverlappingPartialOverlayOwnership { mask: 0x04 })
    );
}
