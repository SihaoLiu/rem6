use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_snapshots_nested_o3_fu_latency_classes() {
    let path = detailed_o3_dump_reset_fu_stats_binary(
        "m5-switch-cpu-o3-dump-reset-nested-fu-latency-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
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

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the old epoch before resetting: {dump}"
    );

    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.cycles",
            "Cycle",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.min_cycles",
            "Cycle",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.float_misc.avg_cycles",
            "Cycle",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.vector_float_misc.instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.vector_float_misc.cycles",
            "Cycle",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.instructions",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.fu_latency_class.integer_div.cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_class.integer_mul.instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_class.integer_div.cycles",
        "Cycle",
        19,
        "monotonic",
    );
}

#[test]
fn rem6_run_restore_exposes_nested_o3_fu_latency_class_runtime_summary() {
    let path =
        detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-nested-fu-summary");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1500",
            "--stats-format",
            "json",
            "--execute",
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
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    for (pointer, value) in [
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/instructions",
            1,
        ),
        ("/cores/0/o3_runtime/fu_latency_class/integer_mul/cycles", 2),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/instructions",
            1,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/float_misc/instructions",
            2,
        ),
        ("/cores/0/o3_runtime/fu_latency_class/float_misc/cycles", 3),
        (
            "/cores/0/o3_runtime/fu_latency_class/vector_float_misc/instructions",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/vector_float_misc/cycles",
            3,
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "missing restored nested FU latency summary {pointer}: {json}"
        );
    }

    assert_eq!(
        json.pointer("/cores/0/o3_runtime/fu_integer_mul_instructions")
            .and_then(Value::as_u64),
        Some(1),
        "legacy flat runtime summary should remain available: {json}"
    );
}

#[test]
fn rem6_run_restore_exposes_o3_fu_latency_class_min_max_avg_runtime_summary() {
    let path =
        detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-fu-latency-extrema");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1500",
            "--stats-format",
            "json",
            "--execute",
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
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    for (path, unit, value) in [
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.min_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_mul.avg_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.max_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.min_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.cpu0.o3.fu_latency_class.integer_div.avg_cycles",
            "Cycle",
            19,
        ),
    ] {
        assert_json_stat(&json, path, unit, value, "monotonic");
    }

    for (pointer, value) in [
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/max_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/min_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_mul/avg_cycles",
            2,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/max_cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/min_cycles",
            19,
        ),
        (
            "/cores/0/o3_runtime/fu_latency_class/integer_div/avg_cycles",
            19,
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "missing restored FU latency class runtime summary {pointer}: {json}"
        );
    }
}
