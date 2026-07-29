use super::*;

#[test]
fn o3_live_checkpoint_v2_round_trips_pending_store_and_decodes_v1() {
    let expected = pending_store_payload();
    let encoded = expected.encode().unwrap();
    assert_eq!(encoded[4], 3);
    assert_eq!(encoded[5], 2);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode_versioned(&encoded),
        Ok((3, expected.clone())),
    );
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected));

    let legacy = include_bytes!("../../fixtures/compute-v1.bin");
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
    let mut legacy = include_bytes!("../../fixtures/compute-v1.bin").to_vec();
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
    value.pending_addresses.clear();
    assert_bad_pending_value(value);
}

#[test]
fn o3_live_checkpoint_compute_and_fp_profiles_reject_pending_row() {
    let pending = pending_store_payload().pending_addresses;
    for mut value in [
        compute_payload(),
        completed_fp_payload(MemoryWidth::Doubleword),
    ] {
        value.pending_addresses = pending.clone();
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
        pending_mut(value).published_producer_ready_tick = Some(value.captured_tick + 1);
    });
}

#[test]
fn o3_live_checkpoint_pending_profile_rejects_wake_before_publication() {
    assert_bad_pending(|value| {
        let pending = pending_mut(value);
        pending.requested_wake_tick = Some(pending.published_producer_ready_tick.unwrap() - 1);
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
