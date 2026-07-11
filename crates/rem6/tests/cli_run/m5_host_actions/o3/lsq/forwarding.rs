use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_lsq_forwarding_snapshot() {
    let path = detailed_o3_lsq_forwarding_dump_reset_stats_binary(
        "m5-switch-cpu-o3-lsq-forwarding-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000080:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000033000000")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0)
    );
    for (path, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_candidates",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_matches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_byte_mismatches",
            0,
        ),
        ("system.cpu.lsq0.storeLoadForwardingSuppressed", 0),
        ("system.cpu.lsq0.storeLoadForwardingAddressMismatches", 0),
        ("system.cpu.lsq0.storeLoadForwardingByteMismatches", 0),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
            1,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
            1,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
            0,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingCandidates",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingMatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingSuppressed",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingAddressMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingByteMismatches",
            0,
        ),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1)
    );
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_suppressed",
        "system.cpu.lsq0.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
        "system.cpu.lsq0.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
        "system.cpu.lsq0.storeLoadForwardingByteMismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_suppressed",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_byte_mismatches",
        "system.cpu.lsq0.operation.store.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.operation.store.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.operation.store.storeLoadForwardingByteMismatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_multicore_o3_lsq_forwarding_by_active_hart() {
    let path = multicore_hart1_detailed_o3_lsq_forwarding_dump_reset_stats_binary(
        "m5-switch-cpu-hart1-o3-lsq-forwarding-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
            "--dump-memory",
            "0x80000080:16",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a000000330000000000000044000000"),
        "hart 1 reset-scoped LSQ forwarding run should preserve match and mismatch witnesses"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2),
        "multicore LSQ forwarding reset fixture should deliver pre/post dumps: {host_actions}"
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_execution_mode_switch(
        host_actions,
        0,
        "cpu1",
        None,
        "detailed",
        "execution-mode-switch-cpu1",
    );

    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing CPU1 pre-reset LSQ forwarding dump: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0)
    );
    for (suffix, value) in [
        ("candidates", 1),
        ("matches", 1),
        ("suppressed", 0),
        ("address_mismatches", 0),
        ("byte_mismatches", 0),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
        assert_stats_dump_sample(
            pre_reset_dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load.forwarding_{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (suffix, value) in [("Candidates", 1), ("Matches", 1), ("Suppressed", 0)] {
        assert_stats_dump_sample(
            pre_reset_dump,
            &format!("system.cpu1.lsq0.operation.load.storeLoadForwarding{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing CPU1 post-reset LSQ forwarding dump: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1)
    );
    for (suffix, value) in [
        ("candidates", 1),
        ("matches", 1),
        ("suppressed", 1),
        ("address_mismatches", 1),
        ("byte_mismatches", 0),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
        assert_stats_dump_sample(
            post_reset_dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
        assert_stats_dump_sample(
            post_reset_dump,
            &format!("sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load.forwarding_{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (suffix, value) in [
        ("Candidates", 1),
        ("Matches", 1),
        ("Suppressed", 1),
        ("AddressMismatches", 1),
        ("ByteMismatches", 0),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            &format!("system.cpu1.lsq0.operation.load.storeLoadForwarding{suffix}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for dump in [pre_reset_dump, post_reset_dump] {
        for suffix in [
            "candidates",
            "matches",
            "suppressed",
            "address_mismatches",
            "byte_mismatches",
        ] {
            for prefix in [
                "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding",
                "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding",
                "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.forwarding",
            ] {
                assert_stats_dump_sample_absent(dump, &format!("{prefix}_{suffix}"));
            }
        }
        for suffix in [
            "Candidates",
            "Matches",
            "Suppressed",
            "AddressMismatches",
            "ByteMismatches",
        ] {
            for prefix in ["system.cpu", "system.cpu0"] {
                assert_stats_dump_sample_absent(
                    dump,
                    &format!("{prefix}.lsq0.storeLoadForwarding{suffix}"),
                );
                assert_stats_dump_sample_absent(
                    dump,
                    &format!("{prefix}.lsq0.operation.load.storeLoadForwarding{suffix}"),
                );
            }
        }
    }
    for (suffix, value) in [
        ("candidates", 1),
        ("matches", 1),
        ("suppressed", 1),
        ("address_mismatches", 1),
        ("byte_mismatches", 0),
    ] {
        assert_json_stat(
            &json,
            &format!("sim.cpu1.o3.lsq_store_to_load_forwarding_{suffix}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for (suffix, value) in [
        ("Candidates", 1),
        ("Matches", 1),
        ("Suppressed", 1),
        ("AddressMismatches", 1),
        ("ByteMismatches", 0),
    ] {
        assert_json_stat(
            &json,
            &format!("system.cpu1.lsq0.operation.load.storeLoadForwarding{suffix}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for path in [
        "candidates",
        "matches",
        "suppressed",
        "address_mismatches",
        "byte_mismatches",
    ] {
        for prefix in [
            "sim.cpu0.o3.lsq_store_to_load_forwarding",
            "sim.cpu0.o3.lsq_operation.load_forwarding",
        ] {
            assert_json_stat_absent(&json, &format!("{prefix}_{path}"));
        }
    }
    for suffix in [
        "Candidates",
        "Matches",
        "Suppressed",
        "AddressMismatches",
        "ByteMismatches",
    ] {
        for prefix in ["system.cpu", "system.cpu0"] {
            assert_json_stat_absent(&json, &format!("{prefix}.lsq0.storeLoadForwarding{suffix}"));
            assert_json_stat_absent(
                &json,
                &format!("{prefix}.lsq0.operation.load.storeLoadForwarding{suffix}"),
            );
        }
    }
}

#[test]
fn rem6_run_o3_runtime_json_classifies_store_load_forwarding_relations() {
    for (path, candidates, matches, suppressed, address_mismatches, byte_mismatches) in [
        (
            detailed_o3_lsq_store_load_mismatch_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-address-suppression-json",
            ),
            0,
            0,
            1,
            1,
            0,
        ),
        (
            detailed_o3_lsq_store_load_partial_overlap_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-partial-overlap-json",
            ),
            1,
            1,
            0,
            0,
            0,
        ),
        (
            detailed_o3_lsq_store_load_address_mismatch_byte_load_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-address-byte-load-suppression-json",
            ),
            0,
            0,
            1,
            1,
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
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let json: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("stopped_by_host")
        );
        let o3_runtime = json
            .pointer("/cores/0/o3_runtime")
            .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

        for (pointer, stat_path, value) in [
            (
                "/store_load_forwarding_candidates",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
                candidates,
            ),
            (
                "/store_load_forwarding_matches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
                matches,
            ),
            (
                "/store_load_forwarding_suppressed",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
                suppressed,
            ),
            (
                "/store_load_forwarding_address_mismatches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            (
                "/store_load_forwarding_byte_mismatches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
                byte_mismatches,
            ),
            (
                "/lsq/store_load_forwarding_suppressed",
                "system.cpu.lsq0.storeLoadForwardingSuppressed",
                suppressed,
            ),
            (
                "/lsq/store_load_forwarding_address_mismatches",
                "system.cpu.lsq0.storeLoadForwardingAddressMismatches",
                address_mismatches,
            ),
            (
                "/lsq/store_load_forwarding_byte_mismatches",
                "system.cpu.lsq0.storeLoadForwardingByteMismatches",
                byte_mismatches,
            ),
            (
                "/lsq/operation/load/forwarding_suppressed",
                "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
                suppressed,
            ),
            (
                "/lsq/operation/load/forwarding_address_mismatches",
                "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
                address_mismatches,
            ),
            (
                "/lsq/operation/load/forwarding_byte_mismatches",
                "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
                byte_mismatches,
            ),
        ] {
            assert_eq!(
                o3_runtime.pointer(pointer).and_then(Value::as_u64),
                Some(value),
                "structured O3 runtime JSON should expose {pointer}: {o3_runtime}"
            );
            assert_json_stat(&json, stat_path, "Count", value, "monotonic");
        }
        for pointer in [
            "/lsq/operation/store/forwarding_suppressed",
            "/lsq/operation/store/forwarding_address_mismatches",
            "/lsq/operation/store/forwarding_byte_mismatches",
            "/lsq/operation/atomic/forwarding_suppressed",
            "/lsq/operation/atomic/forwarding_address_mismatches",
            "/lsq/operation/atomic/forwarding_byte_mismatches",
        ] {
            assert_eq!(
                o3_runtime.pointer(pointer).and_then(Value::as_u64),
                Some(0),
                "inactive O3 forwarding-suppression lane should stay zero at {pointer}: {o3_runtime}"
            );
        }
    }
}
