use super::*;

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_lsq_forwarding_snapshot_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_lsq_forwarding_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-lsq-forwarding-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
            "--dump-memory",
            "0x80000400:16",
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
        Some("5a0000006b0000000000000000000000"),
        "restored LSQ forwarding run should replay the checkpointed store/load and then the post-restore mutation"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    for dump in [first_dump, restored_dump] {
        for (path, value) in [
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_candidates",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_matches",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_store_to_load_forwarding_suppressed",
                0,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_candidates",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_matches",
                1,
            ),
            ("system.cpu1.lsq0.storeLoadForwardingCandidates", 1),
            ("system.cpu1.lsq0.storeLoadForwardingMatches", 1),
            ("system.cpu1.lsq0.storeLoadForwardingSuppressed", 0),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingCandidates",
                1,
            ),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingMatches",
                1,
            ),
            (
                "system.cpu1.lsq0.operation.load.storeLoadForwardingSuppressed",
                0,
            ),
            (
                "system.cpu1.lsq0.operation.store.storeLoadForwardingCandidates",
                0,
            ),
            (
                "system.cpu1.lsq0.operation.store.storeLoadForwardingMatches",
                0,
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
            "system.cpu.lsq0.storeLoadForwardingCandidates",
            "system.cpu.lsq0.storeLoadForwardingMatches",
            "system.cpu0.lsq0.storeLoadForwardingMatches",
            "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
            "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
            "system.cpu0.lsq0.operation.load.storeLoadForwardingCandidates",
            "system.cpu0.lsq0.operation.load.storeLoadForwardingMatches",
        ] {
            assert_stats_dump_sample_absent(dump, path);
            assert_json_stat_absent(&json, path);
        }
    }

    let o3_trace = json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 trace records: {json}"));
    let cpu1_record = o3_trace
        .iter()
        .find(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(1))
        .unwrap_or_else(|| panic!("missing CPU1 O3 trace record: {o3_trace:?}"));
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore_label")
            .and_then(Value::as_str),
        Some("gem5-m5-checkpoint")
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore/latest_tick")
            .and_then(Value::as_u64),
        Some(restore_tick)
    );
    assert_eq!(
        cpu1_record
            .pointer("/checkpoint_restore/execution_mode_authority/target/cpu1/mode/detailed")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        cpu1_record
            .pointer("/events")
            .and_then(Value::as_array)
            .is_some_and(|events| {
                !events.is_empty()
                    && events.iter().all(|event| {
                        event.pointer("/tick").and_then(Value::as_u64) > Some(restore_tick)
                    })
            }),
        "CPU1 O3 trace should contain post-restore replay events: {cpu1_record}"
    );

    assert_json_stat_at_least(
        &json,
        "sim.debug.o3_trace.checkpoint_restore_records",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.operation.load.storeLoadForwardingMatches",
        "Count",
        2,
        "monotonic",
    );
    for path in [
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu0.lsq0.storeLoadForwardingMatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        "system.cpu0.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu0.lsq0.operation.load.storeLoadForwardingMatches",
    ] {
        assert_json_stat_absent(&json, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_restores_multicore_o3_fu_snapshot_by_active_hart() {
    let path = multicore_hart1_detailed_o3_restore_fu_dump_stats_binary(
        "m5-switch-cpu-hart1-o3-restore-fu-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let restored_modes = host_actions
        .pointer("/checkpoint_restores/0/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing restored execution modes: {host_actions}"));
    assert!(
        restored_modes.iter().any(|mode| {
            mode.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && mode.pointer("/mode").and_then(Value::as_str) == Some("detailed")
        }),
        "restored checkpoint should preserve cpu1 detailed-mode authority: {restored_modes:?}"
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );

    for dump in [first_dump, restored_dump] {
        for (path, unit, value) in [
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_latency_instructions",
                "Count",
                2,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_latency_cycles",
                "Cycle",
                21,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_integer_mul_instructions",
                "Count",
                1,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_integer_div_latency_cycles",
                "Cycle",
                19,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_float_misc_instructions",
                "Count",
                0,
            ),
            (
                "sim.host_actions.stats_dump.cpu1.o3.fu_vector_float_misc_instructions",
                "Count",
                0,
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
        }
        for (path, value) in [
            ("system.cpu1.iq.issuedInstType_0::IntMult", 1),
            ("system.cpu1.iq.issuedInstType_0::IntDiv", 1),
            ("system.cpu1.iq.issuedInstType_0::FloatMisc", 0),
            ("system.cpu1.iq.issuedInstType_0::SimdFloatMisc", 0),
            ("system.cpu1.commit.committedInstType_0::IntMult", 1),
            ("system.cpu1.commit.committedInstType_0::IntDiv", 1),
            ("system.cpu1.commit.committedInstType_0::FloatMisc", 0),
            ("system.cpu1.commit.committedInstType_0::SimdFloatMisc", 0),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
            "system.cpu0.iq.issuedInstType_0::IntMult",
            "system.cpu0.commit.committedInstType_0::IntMult",
            "system.cpu.iq.issuedInstType_0::IntMult",
            "system.cpu.commit.committedInstType_0::IntMult",
        ] {
            assert_stats_dump_sample_absent(dump, path);
        }
    }

    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_latency_instructions",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType_0::IntMult");
}
