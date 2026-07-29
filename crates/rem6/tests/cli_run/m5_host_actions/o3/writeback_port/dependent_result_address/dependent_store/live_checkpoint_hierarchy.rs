use super::*;

pub(super) fn assert_pending_store_live_hierarchy(
    restored: &Value,
    captured: &Value,
    baseline: &Value,
    row: DependentStoreRow,
    schedule: PendingStoreLiveSchedule,
) {
    assert_eq!(row.head, DependentAddressHead::AtomicSwap);
    assert_eq!(row.memory_system, "cache-fabric-dram");
    assert_eq!(row.issue_width, 2);
    assert_pending_store_live_capture(captured, schedule);

    let baseline_head = memory_result_event_at_pc(baseline, HEAD_PC);
    let restored_store = memory_result_event_at_pc(restored, STORE_PC);
    assert_eq!(event_str(baseline_head, "lsq_operation"), "atomic");
    assert_eq!(
        event_u64(baseline_head, "commit_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_store_request_transport(restored, baseline_head, restored_store, row.memory_system);
    for action in [
        restored.pointer("/host_actions/checkpoints/0").unwrap(),
        restored
            .pointer("/host_actions/checkpoint_restores/0")
            .unwrap(),
    ] {
        let fabric = action
            .pointer("/components")
            .and_then(Value::as_array)
            .and_then(|components| {
                components.iter().find(|component| {
                    component.pointer("/component").and_then(Value::as_str) == Some("fabric0")
                })
            })
            .expect("fabric0 checkpoint component");
        assert_eq!(event_u64(fabric, "chunk_count"), 2);
        let chunks = fabric
            .pointer("/chunks")
            .and_then(Value::as_array)
            .expect("fabric checkpoint chunks");
        assert!(chunks.iter().any(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) == Some("fabric-runtime-state")
        }));
    }
    let root_start = DATA_START;
    let root_end = root_start + 8;
    let store_start = row.pointer.wrapping_add_signed(i64::from(row.offset));
    let store_end = store_start + 8;
    assert!(root_end <= store_start || store_end <= root_start);
    assert_eq!(
        memory_dump_hex(restored, DATA_START),
        Some(hex_u64_pair(SWAP_VALUE, HEAD_GUARD).as_str())
    );
    for pointer in [
        "/cores/0/committed_instructions",
        "/cores/0/registers",
        "/memory",
        "/simulation/instruction_probes",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            captured.pointer(pointer),
            "{pointer}"
        );
        assert_eq!(
            captured.pointer(pointer),
            baseline.pointer(pointer),
            "{pointer}"
        );
    }
    assert_eq!(
        restored.pointer("/memory_resources/cache/data"),
        baseline.pointer("/memory_resources/cache/data"),
        "restored hierarchy cache parity"
    );
    for pointer in [
        "/memory_resources/fabric/activity",
        "/memory_resources/fabric/active_hops",
        "/memory_resources/fabric/bytes",
        "/memory_resources/fabric/flits",
        "/memory_resources/dram/writes",
        "/memory_resources/dram/write_bytes",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "restored hierarchy work parity {pointer}"
        );
    }
    for pointer in [
        "/memory_resources/dram/accesses",
        "/memory_resources/dram/reads",
        "/memory_resources/dram/read_bytes",
        "/memory_resources/dram/commands",
    ] {
        let restored_value = json_u64(restored, pointer);
        let baseline_value = json_u64(baseline, pointer);
        assert!(restored_value > 0, "restored hierarchy work {pointer}");
        assert_eq!(
            restored_value, baseline_value,
            "restored hierarchy work {pointer}"
        );
    }
}

pub(super) fn assert_hierarchy_retained_checkpoints_restore_out_of_order(
    fixture: &DependentStoreFixture,
    baseline: &Value,
) {
    let restored = fixture.run(
        fixture.row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            "100:early",
            "--host-checkpoint",
            "300:late",
            "--host-restore-checkpoint",
            "500:early",
            "--host-restore-checkpoint",
            "501:late",
        ],
    );

    assert_eq!(json_u64(&restored, "/host_actions/checkpoint_count"), 2);
    assert_eq!(
        json_u64(&restored, "/host_actions/checkpoint_restored_count"),
        2
    );
    for pointer in [
        "/cores/0/registers",
        "/cores/0/committed_instructions",
        "/memory",
        "/memory_resources/cache/data",
        "/memory_resources/dram/accesses",
        "/memory_resources/dram/reads",
        "/memory_resources/dram/writes",
        "/memory_resources/dram/read_bytes",
        "/memory_resources/dram/write_bytes",
        "/memory_resources/dram/commands",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "{pointer}"
        );
    }
}
