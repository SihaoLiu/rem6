use super::*;

const EVENT_COUNT_OFFSET: usize = 4 + 1 + 1 + 8 + 8 + 8;
const FIRST_EVENT_OFFSET: usize = EVENT_COUNT_OFFSET + 4;

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_v2_round_trips_compute_and_rejects_future_watermark() {
    let mut expected = compute_payload();
    expected.finalized_writeback.partial_cycle_ticks.clear();
    expected.finalized_writeback.partial_ready_rows_by_tick.clear();
    expected.finalized_writeback.partial_deferred_rows_by_tick.clear();
    expected.finalized_writeback.closed_before_tick = 100;
    let encoded = expected.encode().unwrap();
    assert_eq!(encoded[4], 2);
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected.clone()));
    expected.finalized_writeback.closed_before_tick = 101;
    assert!(matches!(expected.encode(), Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })));
    let encoded = expected.encode_without_validation_for_test().unwrap();
    assert!(matches!(RiscvO3LiveCheckpointPayload::decode(&encoded), Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })));
}

#[test]
fn o3_live_checkpoint_v2_distinguishes_absent_and_zero_service_identity() {
    let mut absent = compute_payload();
    absent.service.last_service_generation = None;
    let mut zero = absent.clone();
    zero.service.last_service_generation = Some((0, 0));

    let absent_bytes = absent.encode().unwrap();
    let zero_bytes = zero.encode().unwrap();
    assert_ne!(absent_bytes, zero_bytes);
    assert_eq!(
        RiscvO3LiveCheckpointPayload::decode(&absent_bytes),
        Ok(absent)
    );
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&zero_bytes), Ok(zero));
}

#[test]
fn o3_live_checkpoint_rejects_self_suppressing_service_identity_on_encode() {
    assert!(matches!(
        self_suppressing_compute_payload().encode(),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
}

#[test]
fn o3_live_checkpoint_rejects_self_suppressing_service_identity_on_decode() {
    let value = self_suppressing_compute_payload();
    let encoded = value.encode_without_validation_for_test().unwrap();
    assert!(matches!(
        RiscvO3LiveCheckpointPayload::decode(&encoded),
        Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })
    ));
}

fn self_suppressing_compute_payload() -> RiscvO3LiveCheckpointPayload {
    let mut value = compute_payload();
    value.captured_tick = 100;
    value.service.requested_tick = 100;
    value.service.mutation_generation = 7;
    value.service.last_service_generation = Some((100, 7));
    value.wake.tick = 100;
    value
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_v2_round_trips_completed_fp_projection() {
    for width in [MemoryWidth::Word, MemoryWidth::Doubleword] {
        let expected = completed_fp_payload(width);
        let encoded = expected.encode().unwrap();
        assert_eq!(encoded[4], 2);
        assert_eq!(RiscvO3LiveCheckpointPayload::decode(&encoded), Ok(expected), "{width:?}");
    }
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_integer_compute_companion_for_completed_fp_load() {
    let mut value = completed_fp_payload(MemoryWidth::Word);
    value.events.push(compute_payload().events.remove(0));
    assert!(matches!(value.encode(), Err(RiscvO3LiveCheckpointError::InvalidProfileShape { reason: "completed FP load companion is not scalar FP compute" })));
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_encode_preflights_counts_before_semantics() {
    let mut value = compute_payload();
    let mut invalid = value.events.remove(0); invalid.instruction_bytes = 2;
    value.events = vec![invalid; 4_097];
    assert_excessive(&value, "events", 4_097, 4_096);
    let mut value = compute_payload();
    value.events[0].register_writes = vec![RegisterWrite::new(reg(3), 13); 33];
    assert_excessive(&value, "integer writes", 33, 32);
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_overflowing_completed_fp_load_span() {
    assert_bad_completed(|value| {
        let Some(MemoryAccessKind::FloatLoad { address, .. }) = value.events[0].memory_access.as_mut() else { unreachable!() };
        *address = u64::MAX;
        value.completed_result.as_mut().unwrap().physical_address = Address::new(u64::MAX);
    });
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rebuilds_single_fetch_execution_from_raw_bytes() {
    let decoded = RiscvO3LiveCheckpointPayload::decode(&compute_payload().encode().unwrap()).unwrap();
    let add = decoded.events[0].rebuild().unwrap();
    assert!(matches!(add.instruction(), RiscvInstruction::Add { .. }));
    assert_eq!(add.fetch().request_id(), request(1));
    assert_eq!(add.execution().register_writes()[0].value(), 13);
    let mul = decoded.events[1].rebuild().unwrap();
    assert!(matches!(mul.instruction(), RiscvInstruction::Mul { .. }));
    assert_eq!(mul.fetch().request_id(), request(2));
    assert_eq!(mul.execution().register_writes()[0].value(), 65);
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_unknown_magic_version_and_profile() {
    let encoded = compute_payload().encode().unwrap();
    for (offset, value, expected) in [
        (0, b'X', RiscvO3LiveCheckpointError::InvalidMagic),
        (4, 3, RiscvO3LiveCheckpointError::UnsupportedVersion { version: 3 }),
        (5, 9, RiscvO3LiveCheckpointError::UnsupportedProfile { profile: 9 }),
    ] {
        assert_byte_error(&encoded, offset, value, expected);
    }
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_truncation_trailing_bytes_and_excessive_counts() {
    let encoded = compute_payload().encode().unwrap();
    assert!(matches!(RiscvO3LiveCheckpointPayload::decode(&encoded[..encoded.len() - 1]), Err(RiscvO3LiveCheckpointError::Truncated { .. })));
    let mut trailing = encoded.clone();
    trailing.push(0xaa);
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&trailing), Err(RiscvO3LiveCheckpointError::TrailingBytes { remaining: 1 }));
    let mut excessive = encoded;
    excessive[EVENT_COUNT_OFFSET..EVENT_COUNT_OFFSET + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(RiscvO3LiveCheckpointPayload::decode(&excessive), Err(RiscvO3LiveCheckpointError::ExcessiveCount { field: "events", count, .. }) if count == u64::from(u32::MAX)));
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_invalid_bool_event_kind_and_register() {
    let encoded = compute_payload().encode().unwrap();
    let endpoint_len = "cpu0.ifetch".len();
    let event_kind_offset = FIRST_EVENT_OFFSET + 8 + 4 + 8 + 4 + endpoint_len + 4 + 8 + 8 + 8;
    let register_count_offset = event_kind_offset + 1 + 4 + 4 + 8 + 8 + 1;
    let register_offset = register_count_offset + 4;
    let retired_offset = register_count_offset + 4 + 9 + 4 + 1 + 1;

    for (offset, value, expected) in [
        (retired_offset, 2, RiscvO3LiveCheckpointError::InvalidBoolean { field: "event counts as retired instruction", value: 2 }),
        (event_kind_offset, 9, RiscvO3LiveCheckpointError::InvalidTag { field: "fetch event kind", value: 9 }),
        (register_offset, 32, RiscvO3LiveCheckpointError::InvalidRegister { field: "integer write", index: 32 }),
    ] {
        assert_byte_error(&encoded, offset, value, expected);
    }
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_non_exact_fetched_instruction_width() {
    for (bytes, width) in [
        (add(3, 1, 2).to_le_bytes()[..2].to_vec(), 4),
        (vec![0x85, 0x01, 0xaa, 0xbb], 2),
    ] {
        let mut event = compute_payload().events.remove(0);
        replace_fetch_bytes(&mut event, bytes, width);
        assert!(matches!(event.rebuild(), Err(RiscvO3LiveCheckpointError::InstructionWidth { .. })));
    }
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_completed_fp_load_structural_mismatches() {
    let mutations: &[fn(&mut RiscvO3LiveCheckpointPayload)] = &[
        |v| v.events = compute_payload().events,
        |v| { let duplicate = v.events[0].clone(); v.events.push(duplicate); },
        |v| v.completed_result.as_mut().unwrap().fetch_request = request(99),
        |v| v.completed_result.as_mut().unwrap().destination = freg(4),
        |v| { let r = v.completed_result.as_mut().unwrap(); r.width = MemoryWidth::Doubleword; r.response_bytes.resize(8, 0); r.access_size = AccessSize::new(8).unwrap(); },
        |v| { v.completed_result.as_mut().unwrap().response_bytes.pop(); },
        |v| v.completed_result.as_mut().unwrap().physical_address = Address::new(0x9004),
        |v| v.reservation.as_mut().unwrap().source = RiscvO3LiveCheckpointWritebackSource::FixedFunction,
        |v| v.reservation.as_mut().unwrap().sequence += 1,
        |v| v.reservation.as_mut().unwrap().raw_ready_tick += 1,
        |v| v.completed_result.as_mut().unwrap().issue_tick += 1,
        |v| v.completed_result.as_mut().unwrap().issue_tick = u64::MAX,
        |v| { v.completed_result.as_mut().unwrap().raw_ready_tick = 102; v.reservation.as_mut().unwrap().raw_ready_tick = 102; },
        |v| { v.completed_result.as_mut().unwrap().admitted_tick = 103; v.reservation.as_mut().unwrap().admitted_tick = 103; },
    ];
    for mutation in mutations {
        assert_bad_completed(*mutation);
    }
    let mut incomplete = completed_fp_payload(MemoryWidth::Word);
    incomplete.events[0].data_access_event_kind = Some(RiscvDataAccessEventKind::Issued);
    assert!(matches!(incomplete.encode(), Err(RiscvO3LiveCheckpointError::UnsupportedEvent { .. })));
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_instruction_write_mismatches() {
    let cases: &[(RiscvO3LiveCheckpointEvent, fn(&mut RiscvO3LiveCheckpointEvent))] = &[
        (compute_payload().events.remove(0), |e| { e.register_writes.clear(); e.float_register_writes.push(FloatRegisterWrite::new(freg(3), 13)); }),
        (compute_payload().events.remove(0), |e| e.register_writes = vec![RegisterWrite::new(reg(4), 13)]),
        (completed_fp_payload(MemoryWidth::Word).events.remove(1), |e| { e.float_register_writes.clear(); e.register_writes = vec![RegisterWrite::new(reg(5), 1)]; }),
        (completed_fp_payload(MemoryWidth::Word).events.remove(0), |e| e.float_register_writes.push(FloatRegisterWrite::new(freg(3), 1))),
    ];
    for (event, mutation) in cases {
        let mut event = event.clone();
        mutation(&mut event);
        assert!(matches!(event.rebuild(), Err(RiscvO3LiveCheckpointError::InstructionMismatch)));
    }
}

#[test]
#[rustfmt::skip]
fn o3_live_checkpoint_rejects_invalid_rename_projection() {
    let unsupported_classes = [O3RegisterClass::Vector, O3RegisterClass::ConditionCode, O3RegisterClass::Misc];
    for row in unsupported_classes.map(|class| {
        O3RenameMapEntry::new(class, 3, O3PhysicalRegisterId::new(43))
    }).into_iter().chain([
        O3RenameMapEntry::new(O3RegisterClass::Integer, 3, O3PhysicalRegisterId::invalid()),
        O3RenameMapEntry::new(O3RegisterClass::Integer, 32, O3PhysicalRegisterId::new(43)),
    ]) {
        let mut value = compute_payload();
        value.rename_rows[0] = row;
        assert!(value.encode().is_err());
    }

    let encoded = compute_payload().encode().unwrap();
    let rename = encoded.windows(9).position(|bytes| bytes == [0, 3, 0, 0, 0, 43, 0, 0, 0]).unwrap();
    for (offset, bytes) in [2, 3, 4].map(|tag| (rename, vec![tag])).into_iter().chain([
        (rename + 1, 32_u32.to_le_bytes().to_vec()),
        (rename + 5, u32::MAX.to_le_bytes().to_vec()),
    ]) {
        let mut corrupt = encoded.clone();
        corrupt[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(RiscvO3LiveCheckpointPayload::decode(&corrupt).is_err());
    }
}

#[rustfmt::skip]
fn replace_fetch_bytes(event: &mut RiscvO3LiveCheckpointEvent, bytes: Vec<u8>, width: u8) {
    let fetch = &event.fetch;
    let record = CpuFetchRecord::new(
        fetch.tick(), fetch.partition(), fetch.route(), fetch.endpoint().clone(),
        fetch.request_id(), fetch.pc(), AccessSize::new(bytes.len() as u64).unwrap(),
    );
    event.fetch = CpuFetchEvent::completed(record, bytes);
    event.instruction_bytes = width;
    event.next_pc = event.execution_pc + u64::from(width);
}

#[rustfmt::skip]
fn assert_bad_completed(change: impl FnOnce(&mut RiscvO3LiveCheckpointPayload)) {
    let mut value = completed_fp_payload(MemoryWidth::Word);
    change(&mut value);
    assert!(matches!(value.encode(), Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })));
    let encoded = value.encode_without_validation_for_test().unwrap();
    assert!(matches!(RiscvO3LiveCheckpointPayload::decode(&encoded), Err(RiscvO3LiveCheckpointError::InvalidProfileShape { .. })));
}

#[rustfmt::skip]
fn assert_excessive(value: &RiscvO3LiveCheckpointPayload, field: &'static str, count: u64, maximum: usize) {
    assert_eq!(value.encode(), Err(RiscvO3LiveCheckpointError::ExcessiveCount { field, count, maximum }));
}

#[rustfmt::skip]
fn assert_byte_error(encoded: &[u8], offset: usize, value: u8, expected: RiscvO3LiveCheckpointError) {
    let mut corrupt = encoded.to_vec();
    corrupt[offset] = value;
    assert_eq!(RiscvO3LiveCheckpointPayload::decode(&corrupt), Err(expected));
}
