use super::*;

const STORE_SEQUENCE: u64 = 11;
const PRODUCER_SEQUENCE: u64 = 10;
const STORE_PC: u64 = 0x8000;
const ROOT_ADDRESS: u64 = 0x9000;

#[test]
fn o3_live_checkpoint_v2_round_trips_pending_store_and_decodes_v1() {
    let expected = pending_store_payload();
    let encoded = expected.encode().unwrap();
    assert_eq!(encoded[4], 2);
    assert_eq!(encoded[5], 2);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(&encoded),
        Ok((2, expected.clone())),
    );
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected));

    let legacy = include_bytes!("fixtures/compute-v1.bin");
    assert_eq!(legacy[4], 1);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(legacy),
        Ok((1, compute_payload())),
    );
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(legacy),
        Ok(compute_payload()),
    );
}

#[test]
fn o3_live_checkpoint_v1_rejects_pending_profile_tag() {
    let mut legacy = include_bytes!("fixtures/compute-v1.bin").to_vec();
    assert_eq!(legacy[4], 1);
    legacy[5] = 2;
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&legacy),
        Err(RiscvO3LiveCheckpointError::UnsupportedProfile { profile: 2 }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_missing_pending_row() {
    let mut value = pending_store_payload();
    value.pending_address = None;
    assert_bad_pending_value(value);
}

#[test]
fn o3_live_checkpoint_compute_and_fp_profiles_reject_pending_row() {
    let pending = pending_store_payload().pending_address;
    for mut value in [
        compute_payload(),
        completed_fp_payload(MemoryWidth::Doubleword),
    ] {
        value.pending_address = pending.clone();
        assert_bad_pending_value(value);
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_destinationful_instruction() {
    assert_bad_pending(|value| {
        replace_pending_instruction(value, i_type(0, 5, 3, 7, 0x03));
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_non_store_lsq_kind() {
    assert_bad_pending(|value| {
        pending_mut(value).lsq_kind = O3LoadStoreQueueKind::Load;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_wrong_width_store() {
    assert_bad_pending(|value| {
        replace_pending_instruction(value, store(6, 5, MemoryWidth::Word));
        pending_mut(value).expected_lsq_bytes = 4;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_duplicate_consumed_requests() {
    assert_bad_pending(|value| {
        pending_mut(value).consumed_requests = vec![request(STORE_SEQUENCE); 2];
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_invalid_root_range() {
    let (mut encoded, root_address_offset) = pending_field_offset(|pending| {
        pending.root_range =
            AddressRange::new(Address::new(ROOT_ADDRESS + 1), AccessSize::new(8).unwrap()).unwrap();
    });
    encoded[root_address_offset..root_address_offset + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::InvalidField {
            field: "pending root range",
            value: u64::MAX,
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_sequence_lineage_mismatches() {
    let mutations: &[fn(&mut RiscvO3LiveCheckpointPayload)] = &[
        |value| pending_mut(value).root_sequence = PRODUCER_SEQUENCE - 1,
        |value| pending_mut(value).producer_sequence = STORE_SEQUENCE,
        |value| pending_mut(value).sequence = STORE_SEQUENCE + 1,
    ];
    for mutation in mutations {
        assert_bad_pending(*mutation);
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_publication_after_capture() {
    assert_bad_pending(|value| {
        pending_mut(value).published_producer_ready_tick = value.captured_tick + 1;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_wake_before_publication() {
    assert_bad_pending(|value| {
        let pending = pending_mut(value);
        pending.requested_wake_tick = pending.published_producer_ready_tick - 1;
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_trailing_bytes() {
    let mut encoded = pending_store_payload()
        .encode_without_validation_for_test()
        .unwrap();
    encoded.push(0xaa);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::TrailingBytes { remaining: 1 }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_truncated_consumed_request_list() {
    let (mut encoded, count_offset) = pending_field_offset(|pending| {
        pending.consumed_requests.clear();
    });
    encoded.truncate(count_offset + 4 + 11);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::Truncated {
            field: "pending consumed request",
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_excessive_consumed_request_list() {
    let (mut encoded, count_offset) = pending_field_offset(|pending| {
        pending.consumed_requests.clear();
    });
    encoded[count_offset..count_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::ExcessiveCount {
            field: "pending consumed requests",
            count: u64::from(u32::MAX),
            maximum: 65_536,
        }),
    );
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_invalid_boolean_tag_and_register() {
    let cases = [
        pending_wire_corruption(
            |pending| pending.root_atomic = true,
            2,
            RiscvO3LiveCheckpointError::InvalidBoolean {
                field: "pending root atomic",
                value: 2,
            },
        ),
        pending_wire_corruption(
            |pending| pending.lsq_kind = O3LoadStoreQueueKind::Load,
            9,
            RiscvO3LiveCheckpointError::InvalidTag {
                field: "pending LSQ kind",
                value: 9,
            },
        ),
        pending_wire_corruption(
            |pending| pending.producer_register = reg(6),
            32,
            RiscvO3LiveCheckpointError::InvalidRegister {
                field: "pending producer register",
                index: 32,
            },
        ),
    ];
    for (encoded, expected) in cases {
        assert_eq!(
            RiscvO3LiveCheckpointPayload::decode(&encoded),
            Err(expected)
        );
    }
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_generic_and_writeback_ownership() {
    let mutations: &[fn(&mut RiscvO3LiveCheckpointPayload)] = &[
        |value| value.events = compute_payload().events,
        |value| value.rename_rows = compute_payload().rename_rows,
        |value| value.executed_fetch_requests = vec![request(STORE_SEQUENCE)],
        |value| value.issued_fetch_requests = vec![request(STORE_SEQUENCE)],
        |value| value.writeback_counted_sequences = vec![PRODUCER_SEQUENCE],
        |value| value.writeback_published_sequences = vec![PRODUCER_SEQUENCE],
        |value| value.issue_rows[0].sequence += 1,
        |value| value.resident_sequences[0] += 1,
    ];
    for mutation in mutations {
        assert_bad_pending(*mutation);
    }
}

fn pending_store_payload() -> RiscvO3LiveCheckpointPayload {
    let mut value = compute_payload();
    value.profile = RiscvO3LiveCheckpointProfile::PendingDataAddress;
    value.next_fetch_pc = Address::new(STORE_PC + 4);
    value.events.clear();
    value.issue_rows = vec![RiscvO3LiveCheckpointIssueRow {
        sequence: STORE_SEQUENCE,
        fetch_request: request(STORE_SEQUENCE),
    }];
    value.rename_rows.clear();
    value.resident_sequences = vec![STORE_SEQUENCE];
    value.executed_fetch_requests.clear();
    value.issued_fetch_requests.clear();
    value.service.telemetry.current_occupancy = 1;
    value.pending_address = Some(RiscvO3LiveCheckpointPendingDataAddress {
        sequence: STORE_SEQUENCE,
        fetch: pending_fetch(store(6, 5, MemoryWidth::Doubleword)),
        consumed_requests: vec![request(STORE_SEQUENCE)],
        fetch_predecessor_request: request(PRODUCER_SEQUENCE),
        producer_register: reg(5),
        producer_sequence: PRODUCER_SEQUENCE,
        root_sequence: PRODUCER_SEQUENCE,
        root_fetch_request: request(PRODUCER_SEQUENCE),
        root_range: AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap())
            .unwrap(),
        root_atomic: false,
        lsq_kind: O3LoadStoreQueueKind::Store,
        expected_lsq_bytes: 8,
        published_producer_ready_tick: 99,
        requested_wake_tick: value.wake.tick,
    });
    value
}

fn pending_fetch(raw: u32) -> CpuFetchEvent {
    let bytes = raw.to_le_bytes().to_vec();
    let record = CpuFetchRecord::new(
        31,
        PartitionId::new(2),
        MemoryRouteId::new(9),
        TransportEndpointId::new("cpu0.ifetch").unwrap(),
        request(STORE_SEQUENCE),
        Address::new(STORE_PC),
        AccessSize::new(bytes.len() as u64).unwrap(),
    );
    CpuFetchEvent::completed(record, bytes)
}

fn replace_pending_instruction(value: &mut RiscvO3LiveCheckpointPayload, raw: u32) {
    pending_mut(value).fetch = pending_fetch(raw);
}

fn pending_mut(
    value: &mut RiscvO3LiveCheckpointPayload,
) -> &mut RiscvO3LiveCheckpointPendingDataAddress {
    value.pending_address.as_mut().unwrap()
}

fn assert_bad_pending(change: impl FnOnce(&mut RiscvO3LiveCheckpointPayload)) {
    let mut value = pending_store_payload();
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

fn pending_wire_corruption(
    change: impl FnOnce(&mut RiscvO3LiveCheckpointPendingDataAddress),
    corrupt: u8,
    expected: RiscvO3LiveCheckpointError,
) -> (Vec<u8>, RiscvO3LiveCheckpointError) {
    let (mut encoded, offset) = pending_field_offset(change);
    encoded[offset] = corrupt;
    (encoded, expected)
}

fn pending_field_offset(
    change: impl FnOnce(&mut RiscvO3LiveCheckpointPendingDataAddress),
) -> (Vec<u8>, usize) {
    let value = pending_store_payload();
    let encoded = value.encode_without_validation_for_test().unwrap();
    let mut changed = value;
    change(pending_mut(&mut changed));
    let changed = changed.encode_without_validation_for_test().unwrap();
    let offset = encoded
        .iter()
        .zip(&changed)
        .position(|(left, right)| left != right)
        .expect("changed pending field must alter its wire encoding");
    (encoded, offset)
}

fn store(rs2: u8, rs1: u8, width: MemoryWidth) -> u32 {
    let funct3 = match width {
        MemoryWidth::Word => 2,
        MemoryWidth::Doubleword => 3,
        _ => unreachable!(),
    };
    (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (funct3 << 12) | 0x23
}
