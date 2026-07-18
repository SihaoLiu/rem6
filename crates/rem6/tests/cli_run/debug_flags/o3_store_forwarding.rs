use super::*;

fn detailed_o3_store_forwarding_debug_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, 0b010),
        i_type(0, 5, 0b010, 12, 0x03),
        b_type(8, 11, 12, 0x1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.push(0);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_forwarding_suppression_debug_binary(
    name: &str,
    store_funct3: u32,
    load_offset: i32,
    load_funct3: u32,
    expected_register: u8,
) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0, 5, 0x17),
        i_type(60, 5, 0x0, 5, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 5, store_funct3),
        i_type(load_offset, 5, load_funct3, 12, 0x03),
        b_type(8, expected_register, 12, 0x1),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 64 {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn detailed_o3_store_forwarding_mismatch_debug_binary(name: &str) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 0b010, 4, 0b010, 0)
}

fn detailed_o3_store_forwarding_partial_overlap_debug_binary(name: &str) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 0b000, 0, 0b010, 11)
}

fn detailed_o3_store_forwarding_address_mismatch_byte_load_debug_binary(
    name: &str,
) -> std::path::PathBuf {
    detailed_o3_store_forwarding_suppression_debug_binary(name, 0b010, 4, 0b100, 0)
}

#[allow(clippy::too_many_arguments)]
fn assert_o3_event_with_store_forwarding(
    event: &Value,
    sequence: u64,
    pc: &str,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    store_load_forwarding_candidate: bool,
    store_load_forwarding_match: bool,
    system_event: bool,
) {
    assert_eq!(json_record_u64(event, "sequence"), sequence);
    json_record_u64(event, "tick");
    assert_eq!(json_record_str(event, "pc"), pc);
    assert_eq!(json_record_bool(event, "rob_allocated"), true);
    assert_eq!(json_record_bool(event, "rob_committed"), true);
    assert_eq!(json_record_u64(event, "rename_writes"), rename_writes);
    assert_eq!(json_record_u64(event, "lsq_loads"), lsq_loads);
    assert_eq!(json_record_u64(event, "lsq_stores"), lsq_stores);
    assert_o3_event_lsq_address_shape(event, lsq_loads, lsq_stores);
    assert_eq!(event.get("fu_latency_class"), Some(&Value::Null));
    assert_eq!(json_record_u64(event, "fu_latency_cycles"), 0);
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_candidate"),
        store_load_forwarding_candidate
    );
    assert_eq!(
        json_record_bool(event, "store_load_forwarding_match"),
        store_load_forwarding_match
    );
    assert_eq!(json_record_bool(event, "system_event"), system_event);
}

#[test]
fn rem6_run_o3_debug_flag_emits_store_forwarding_events() {
    let path = detailed_o3_store_forwarding_debug_binary("debug-flags-o3-store-forwarding-runtime");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .expect("debug O3 trace array");
    assert_eq!(trace.len(), 1);
    let record = &trace[0];

    for (field, value) in [
        ("instructions", 7),
        ("rob_allocations", 7),
        ("rob_commits", 7),
        ("rename_writes", 4),
        ("lsq_loads", 1),
        ("lsq_stores", 1),
        ("store_load_forwarding_candidates", 1),
        ("store_load_forwarding_matches", 1),
        ("fu_latency_instructions", 0),
        ("fu_latency_cycles", 0),
        ("max_rob_occupancy", 2),
        ("max_lsq_occupancy", 2),
        ("rename_map_entries", 3),
    ] {
        assert_eq!(
            json_record_u64(record, field),
            value,
            "O3 trace field {field}"
        );
    }
    let event_summary = record
        .pointer("/event_summary")
        .expect("O3 trace event summary should be embedded with the trace record");
    for (field, value) in [
        ("store_load_forwarding_candidates", 1),
        ("store_load_forwarding_matches", 1),
        ("store_load_forwarding_suppressed", 0),
        ("store_load_forwarding_address_mismatches", 0),
        ("store_load_forwarding_byte_mismatches", 0),
    ] {
        assert_eq!(
            json_record_u64(event_summary, field),
            value,
            "O3 event summary forwarding field {field}"
        );
    }
    for (operation, expected) in [("load", 1), ("store", 0)] {
        let operation_summary = event_summary
            .pointer(&format!("/lsq_operation/{operation}"))
            .unwrap_or_else(|| {
                panic!("missing event summary LSQ operation {operation}: {event_summary:?}")
            });
        for (field, value) in [
            ("forwarding_candidates", expected),
            ("forwarding_matches", expected),
            ("forwarding_suppressed", 0),
            ("forwarding_address_mismatches", 0),
            ("forwarding_byte_mismatches", 0),
        ] {
            assert_eq!(
                json_record_u64(operation_summary, field),
                value,
                "O3 event summary {operation} forwarding field {field}"
            );
        }
    }

    let events = record
        .pointer("/events")
        .and_then(Value::as_array)
        .expect("O3 trace events array");
    assert_eq!(events.len(), 7);
    assert_o3_event_with_store_forwarding(
        &events[0],
        0,
        "0x80000004",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[1],
        1,
        "0x80000008",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[2],
        2,
        "0x8000000c",
        1,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(
        &events[3],
        3,
        "0x80000010",
        0,
        0,
        1,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(&events[4], 4, "0x80000014", 1, 1, 0, true, true, false);
    assert_eq!(
        json_record_str(&events[3], "lsq_store_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(&events[3], "lsq_store_bytes"), 4);
    assert_eq!(
        json_record_str(&events[4], "lsq_load_address"),
        "0x80000040"
    );
    assert_eq!(json_record_u64(&events[4], "lsq_load_bytes"), 4);
    assert_o3_event_with_store_forwarding(
        &events[5],
        5,
        "0x80000018",
        0,
        0,
        0,
        false,
        false,
        false,
    );
    assert_o3_event_with_store_forwarding(&events[6], 6, "0x8000001c", 0, 0, 0, false, false, true);

    for (path, unit, value) in [
        ("sim.debug.o3_trace.records", "Count", 1),
        ("sim.debug.o3_trace.instructions", "Count", 7),
        ("sim.debug.o3_trace.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.lsq_stores", "Count", 1),
        (
            "sim.debug.o3_trace.store_load_forwarding_candidates",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.store_load_forwarding_matches",
            "Count",
            1,
        ),
        ("sim.debug.o3_trace.event.records", "Count", 7),
        ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
        ("sim.debug.o3_trace.event.lsq_stores", "Count", 1),
        (
            "sim.debug.o3_trace.event.store_load_forwarding_candidates",
            "Count",
            1,
        ),
        (
            "sim.debug.o3_trace.event.store_load_forwarding_matches",
            "Count",
            1,
        ),
    ] {
        assert_stat(&stdout, path, unit, value, "monotonic");
    }
}

#[test]
fn rem6_run_o3_debug_flag_classifies_store_forwarding_relations() {
    for (
        path,
        store_bytes,
        load_address,
        load_bytes,
        candidates,
        matches,
        suppressed,
        address_mismatches,
        byte_mismatches,
        partial,
        forwarded_bytes,
    ) in [
        (
            detailed_o3_store_forwarding_mismatch_debug_binary(
                "debug-flags-o3-store-forwarding-address-mismatch",
            ),
            4,
            "0x80000044",
            4,
            0,
            0,
            1,
            1,
            0,
            false,
            0,
        ),
        (
            detailed_o3_store_forwarding_partial_overlap_debug_binary(
                "debug-flags-o3-store-forwarding-partial-overlap",
            ),
            1,
            "0x80000040",
            4,
            1,
            1,
            0,
            0,
            0,
            true,
            1,
        ),
        (
            detailed_o3_store_forwarding_address_mismatch_byte_load_debug_binary(
                "debug-flags-o3-store-forwarding-address-mismatch-byte-load",
            ),
            4,
            "0x80000044",
            1,
            0,
            0,
            1,
            1,
            0,
            false,
            0,
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "160",
                "--stats-format",
                "json",
                "--execute",
                "--memory-system",
                "direct",
                "--debug-flags",
                "O3",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = serde_json::from_str(&stdout).unwrap();
        let trace = json
            .pointer("/debug/o3_trace")
            .and_then(Value::as_array)
            .expect("debug O3 trace array");
        assert_eq!(trace.len(), 1);
        let record = &trace[0];
        for (field, value) in [
            ("store_load_forwarding_candidates", candidates),
            ("store_load_forwarding_matches", matches),
            ("store_load_forwarding_suppressed", suppressed),
            (
                "store_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            ("store_load_forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(record, field),
                value,
                "O3 trace field {field}"
            );
        }
        let event_summary = record
            .pointer("/event_summary")
            .expect("O3 trace event summary should be embedded with the trace record");
        for (field, value) in [
            ("store_load_forwarding_candidates", candidates),
            ("store_load_forwarding_matches", matches),
            ("store_load_forwarding_suppressed", suppressed),
            (
                "store_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            ("store_load_forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(event_summary, field),
                value,
                "O3 event summary forwarding field {field}"
            );
        }
        let load_summary = event_summary
            .pointer("/lsq_operation/load")
            .expect("event summary LSQ load lane");
        for (field, value) in [
            ("forwarding_candidates", candidates),
            ("forwarding_matches", matches),
            ("forwarding_suppressed", suppressed),
            ("forwarding_address_mismatches", address_mismatches),
            ("forwarding_byte_mismatches", byte_mismatches),
        ] {
            assert_eq!(
                json_record_u64(load_summary, field),
                value,
                "O3 event summary load forwarding field {field}"
            );
        }
        let store_summary = event_summary
            .pointer("/lsq_operation/store")
            .expect("event summary LSQ store lane");
        for field in [
            "forwarding_candidates",
            "forwarding_matches",
            "forwarding_suppressed",
            "forwarding_address_mismatches",
            "forwarding_byte_mismatches",
        ] {
            assert_eq!(
                json_record_u64(store_summary, field),
                0,
                "O3 event summary store forwarding field {field}"
            );
        }

        let events = record
            .pointer("/events")
            .and_then(Value::as_array)
            .expect("O3 trace events array");
        assert_eq!(events.len(), 7);
        assert_o3_event_with_store_forwarding(
            &events[3],
            3,
            "0x80000010",
            0,
            0,
            1,
            false,
            false,
            false,
        );
        assert_o3_event_with_store_forwarding(
            &events[4],
            4,
            "0x80000014",
            1,
            1,
            0,
            candidates == 1,
            matches == 1,
            false,
        );
        assert_eq!(
            json_record_str(&events[3], "lsq_store_address"),
            "0x80000040"
        );
        assert_eq!(json_record_u64(&events[3], "lsq_store_bytes"), store_bytes);
        assert_eq!(
            json_record_str(&events[4], "lsq_load_address"),
            load_address
        );
        assert_eq!(json_record_u64(&events[4], "lsq_load_bytes"), load_bytes);
        for field in [
            "store_load_forwarding_suppressed",
            "store_load_forwarding_address_mismatch",
            "store_load_forwarding_byte_mismatch",
        ] {
            assert_eq!(json_record_bool(&events[3], field), false);
        }
        assert_eq!(
            json_record_bool(&events[3], "store_load_forwarding_partial"),
            false
        );
        assert_eq!(
            json_record_u64(&events[3], "store_load_forwarding_bytes"),
            0
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_suppressed"),
            suppressed == 1
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_address_mismatch"),
            address_mismatches == 1
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_byte_mismatch"),
            byte_mismatches == 1
        );
        assert_eq!(
            json_record_bool(&events[4], "store_load_forwarding_partial"),
            partial
        );
        assert_eq!(
            json_record_u64(&events[4], "store_load_forwarding_bytes"),
            forwarded_bytes
        );

        for (path, unit, value) in [
            ("sim.debug.o3_trace.records", "Count", 1),
            ("sim.debug.o3_trace.instructions", "Count", 7),
            ("sim.debug.o3_trace.lsq_loads", "Count", 1),
            ("sim.debug.o3_trace.lsq_stores", "Count", 1),
            (
                "sim.debug.o3_trace.store_load_forwarding_candidates",
                "Count",
                candidates,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_matches",
                "Count",
                matches,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_suppressed",
                "Count",
                suppressed,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_address_mismatches",
                "Count",
                address_mismatches,
            ),
            (
                "sim.debug.o3_trace.store_load_forwarding_byte_mismatches",
                "Count",
                byte_mismatches,
            ),
            ("sim.debug.o3_trace.event.records", "Count", 7),
            ("sim.debug.o3_trace.event.lsq_loads", "Count", 1),
            ("sim.debug.o3_trace.event.lsq_stores", "Count", 1),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_candidates",
                "Count",
                candidates,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_matches",
                "Count",
                matches,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_suppressed",
                "Count",
                suppressed,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_address_mismatches",
                "Count",
                address_mismatches,
            ),
            (
                "sim.debug.o3_trace.event.store_load_forwarding_byte_mismatches",
                "Count",
                byte_mismatches,
            ),
        ] {
            assert_stat(&stdout, path, unit, value, "monotonic");
        }
    }
}
