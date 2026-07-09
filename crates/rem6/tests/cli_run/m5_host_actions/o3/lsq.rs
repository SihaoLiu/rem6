use super::*;

#[path = "lsq/event_summary_dump.rs"]
mod event_summary_dump;
#[path = "lsq/event_window_dump.rs"]
mod event_window_dump;
#[path = "lsq/forwarding.rs"]
mod forwarding;
#[path = "lsq/runtime.rs"]
mod runtime;

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_lsq_matrix_snapshot() {
    let path = detailed_o3_lsq_matrix_dump_reset_stats_binary(
        "m5-switch-cpu-o3-lsq-matrix-dump-reset-stats",
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
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
            "--dump-memory",
            "0x800000a0:16",
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
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
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
        Some(0),
        "dump-reset should snapshot the LSQ epoch before resetting: {pre_reset_dump}"
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load", 1),
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store", 3),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.release",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire_release",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_conditional_failures",
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
    for (path, value) in [
        ("system.cpu.lsq0.operation.load", 1),
        ("system.cpu.lsq0.operation.store", 3),
        ("system.cpu.lsq0.operation.loadReserved", 1),
        ("system.cpu.lsq0.operation.storeConditional", 1),
        ("system.cpu.lsq0.operation.atomic", 1),
        ("system.cpu.lsq0.operation.total", 7),
        ("system.cpu.lsq0.ordering.acquire", 1),
        ("system.cpu.lsq0.ordering.release", 1),
        ("system.cpu.lsq0.ordering.acquireRelease", 1),
        ("system.cpu.lsq0.ordering.total", 3),
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
    for (path, value) in [
        ("system.cpu.lsq0.operation.load.storeConditionalFailures", 0),
        (
            "system.cpu.lsq0.operation.store.storeConditionalFailures",
            0,
        ),
        (
            "system.cpu.lsq0.operation.storeConditional.storeConditionalFailures",
            0,
        ),
        (
            "system.cpu.lsq0.operation.atomic.storeConditionalFailures",
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
    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_samples",
            "Count",
            7,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_ticks",
            "Tick",
            14,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_max_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_min_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_avg_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_samples",
            "Count",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_ticks",
            "Tick",
            6,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic_latency_ticks",
            "Tick",
            2,
        ),
    ] {
        assert_stats_dump_sample(pre_reset_dump, path, "counter", unit, value, "resettable");
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {post_reset_dump}"
    );
    assert!(
        post_reset_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {post_reset_dump}"
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load", 0),
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store", 1),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.release",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire_release",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_conditional_failures",
            1,
        ),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, value) in [
        ("system.cpu.lsq0.operation.load", 0),
        ("system.cpu.lsq0.operation.store", 1),
        ("system.cpu.lsq0.operation.storeConditional", 1),
        ("system.cpu.lsq0.operation.total", 2),
        ("system.cpu.lsq0.ordering.acquireRelease", 0),
        ("system.cpu.lsq0.ordering.total", 0),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, value) in [
        ("system.cpu.lsq0.operation.load.storeConditionalFailures", 0),
        (
            "system.cpu.lsq0.operation.store.storeConditionalFailures",
            0,
        ),
        (
            "system.cpu.lsq0.operation.storeConditional.storeConditionalFailures",
            1,
        ),
        (
            "system.cpu.lsq0.operation.atomic.storeConditionalFailures",
            0,
        ),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_samples",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_max_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_avg_ticks",
            "Tick",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_samples",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
            "Tick",
            0,
        ),
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", unit, value, "resettable");
    }
    runtime::assert_o3_lsq_matrix_dump_nested_aliases(pre_reset_dump, post_reset_dump);

    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.store_conditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_conditional_failures",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_stats_exposes_multicore_o3_lsq_forwarding_by_active_hart() {
    let path = multicore_hart1_detailed_o3_lsq_forwarding_dump_stats_binary(
        "m5-switch-cpu-o3-multicore-lsq-forwarding-dump-stats",
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
            "--cores",
            "2",
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
        Some("5a00000000000000"),
        "hart 1 LSQ forwarding run should store and load the same word"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "multicore LSQ forwarding fixture should deliver one m5_dump_stats action: {host_actions}"
    );
    assert_execution_mode_switch(
        host_actions,
        0,
        "cpu1",
        None,
        "detailed",
        "execution-mode-switch-cpu1",
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing CPU1 LSQ forwarding stats dump: {host_actions}"));
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
        (
            "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.load_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.store_forwarding_candidates",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.store_forwarding_matches",
            0,
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
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu0.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu0.lsq0.operation.load.storeLoadForwardingCandidates",
    ] {
        assert_stats_dump_sample_absent(dump, path);
        assert_json_stat_absent(&json, path);
    }
    assert_json_stat(
        &json,
        "sim.cpu1.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.operation.load.storeLoadForwardingMatches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
}

#[test]
fn rem6_run_records_o3_lsq_store_load_matches_after_detailed_switch() {
    let path =
        detailed_o3_lsq_store_load_match_binary("m5-switch-cpu-detailed-o3-lsq-store-load-match");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
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
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 1, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    for path in [
        "sim.cpu0.o3.lsq_operation.store_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.store_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.store_conditional_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.store_conditional_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.atomic_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.atomic_forwarding_matches",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_load_forwarding_candidates")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_load_forwarding_matches")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_store_forwarding_candidates")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_store_forwarding_matches")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_o3_runtime_json_counts_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-store-conditional-failure-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000040:16",
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
        Some("88776655443322110100000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_store", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
        ("lsq_store_conditional_failures", 1),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
        assert_o3_lsq_count_alias(&json, field, value);
    }
    assert_o3_lsq_count_alias_totals(&json, 2, 0);
}

#[test]
fn rem6_run_does_not_record_o3_store_conditional_failure_after_functional_run() {
    let path = functional_store_conditional_failure_binary(
        "functional-store-conditional-failure-omits-o3-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000040:16",
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
        Some("88776655443322110100000000000000")
    );
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "functional failed SC run should not emit O3 runtime state: {json}"
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_conditional_failures");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_operation.store_conditional");
}

#[test]
fn rem6_run_o3_runtime_json_exposes_float_vector_lsq_matrix() {
    let path = detailed_o3_float_vector_lsq_binary(
        "m5-switch-cpu-detailed-o3-float-vector-lsq-runtime-json",
    );

    let direct_json = float_vector_lsq_runtime_json(&path, Some("direct"), "220");
    let cache_json = float_vector_lsq_runtime_json(&path, None, "320");

    assert_eq!(
        direct_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_eq!(
        cache_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    let direct_latency = assert_float_vector_lsq_runtime_json(&direct_json);
    let cache_latency = assert_float_vector_lsq_runtime_json(&cache_json);
    assert!(
        cache_latency >= direct_latency,
        "cache-backed float/vector LSQ latency should include at least the direct latency: direct={direct_latency}, cache={cache_latency}"
    );
}

fn float_vector_lsq_runtime_json(
    path: &Path,
    memory_system: Option<&str>,
    max_tick: &str,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick,
        "--stats-format",
        "json",
        "--execute",
        "--dump-memory",
        "0x80000080:16",
        "--dump-memory",
        "0x80000090:16",
    ]);
    if let Some(memory_system) = memory_system {
        command.args(["--memory-system", memory_system]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_float_vector_lsq_runtime_json(json: &Value) -> u64 {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("000000000000f03f000000000000f03f")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("44332211887766554433221188776655")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_load", 0),
        ("lsq_operation_store", 0),
        ("lsq_operation_load_reserved", 0),
        ("lsq_operation_store_conditional", 0),
        ("lsq_operation_atomic", 0),
        ("lsq_operation_float_load", 1),
        ("lsq_operation_float_store", 1),
        ("lsq_operation_vector_load", 1),
        ("lsq_operation_vector_store", 1),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
        assert_o3_lsq_count_alias(&json, field, value);
    }
    assert_o3_lsq_count_alias_totals(&json, 4, 0);

    let mut aggregate_latency_samples = 0;
    let mut aggregate_latency_ticks = 0;
    let mut aggregate_latency_max_ticks = 0;
    let mut aggregate_latency_min_ticks = u64::MAX;
    for (operation, alias_operation) in [
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
    ] {
        let samples_field = format!("lsq_operation_{operation}_latency_samples");
        let total_field = format!("lsq_operation_{operation}_latency_ticks");
        let max_field = format!("lsq_operation_{operation}_latency_max_ticks");
        let min_field = format!("lsq_operation_{operation}_latency_min_ticks");
        let avg_field = format!("lsq_operation_{operation}_latency_avg_ticks");
        let samples = o3_runtime
            .pointer(&format!("/{samples_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {samples_field}: {o3_runtime}")
            });
        let total = o3_runtime
            .pointer(&format!("/{total_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {total_field}: {o3_runtime}")
            });
        let max = o3_runtime
            .pointer(&format!("/{max_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {max_field}: {o3_runtime}")
            });
        let min = o3_runtime
            .pointer(&format!("/{min_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {min_field}: {o3_runtime}")
            });
        let avg = o3_runtime
            .pointer(&format!("/{avg_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {avg_field}: {o3_runtime}")
            });
        assert!(
            samples > 0,
            "{samples_field} should be positive: {o3_runtime}"
        );
        assert!(total > 0, "{total_field} should be positive: {o3_runtime}");
        assert!(
            max >= min && min > 0,
            "invalid latency bounds for {operation}: {o3_runtime}"
        );
        assert!(
            avg >= min && avg <= max,
            "average latency should stay within bounds for {operation}: {o3_runtime}"
        );
        assert_eq!(
            avg,
            total / samples,
            "average latency should use the structured sample count for {operation}: {o3_runtime}"
        );
        aggregate_latency_samples += samples;
        aggregate_latency_ticks += total;
        aggregate_latency_max_ticks = aggregate_latency_max_ticks.max(max);
        aggregate_latency_min_ticks = aggregate_latency_min_ticks.min(min);
        for (suffix, value) in [
            ("latency_samples", samples),
            ("latency_ticks", total),
            ("latency_max_ticks", max),
            ("latency_min_ticks", min),
            ("latency_avg_ticks", avg),
        ] {
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{suffix}")
                ),
                value,
                "stat registry should match structured runtime {operation}_{suffix}"
            );
        }
        for (suffix, unit, value) in [
            ("samples", "Count", samples),
            ("totalLatency", "Tick", total),
            ("maxLatency", "Tick", max),
            ("minLatency", "Tick", min),
            ("avgLatency", "Tick", avg),
        ] {
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    let aggregate_latency_avg_ticks = aggregate_latency_ticks / aggregate_latency_samples;
    for (field, alias, unit, value) in [
        (
            "lsq_data_latency_samples",
            "samples",
            "Count",
            aggregate_latency_samples,
        ),
        (
            "lsq_data_latency_ticks",
            "totalLatency",
            "Tick",
            aggregate_latency_ticks,
        ),
        (
            "lsq_data_latency_max_ticks",
            "maxLatency",
            "Tick",
            aggregate_latency_max_ticks,
        ),
        (
            "lsq_data_latency_min_ticks",
            "minLatency",
            "Tick",
            aggregate_latency_min_ticks,
        ),
        (
            "lsq_data_latency_avg_ticks",
            "avgLatency",
            "Tick",
            aggregate_latency_avg_ticks,
        ),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose aggregate {field}: {o3_runtime}"
        );
        assert_json_stat(
            &json,
            &format!("sim.cpu0.o3.{field}"),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            json,
            &format!("system.cpu.lsq0.dataResponse.{alias}"),
            unit,
            value,
            "monotonic",
        );
    }
    for (operation, alias_operation) in [
        ("load", "load"),
        ("store", "store"),
        ("load_reserved", "loadReserved"),
        ("store_conditional", "storeConditional"),
        ("atomic", "atomic"),
    ] {
        for (field, alias, unit) in [
            ("latency_samples", "samples", "Count"),
            ("latency_ticks", "totalLatency", "Tick"),
            ("latency_max_ticks", "maxLatency", "Tick"),
            ("latency_min_ticks", "minLatency", "Tick"),
            ("latency_avg_ticks", "avgLatency", "Tick"),
        ] {
            let runtime_field = format!("lsq_operation_{operation}_{field}");
            assert_eq!(
                o3_runtime
                    .pointer(&format!("/{runtime_field}"))
                    .and_then(Value::as_u64),
                Some(0),
                "structured O3 runtime JSON should expose inactive {runtime_field}: {o3_runtime}"
            );
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{field}")
                ),
                0,
                "stat registry should expose inactive {operation}_{field}"
            );
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{alias}"),
                unit,
                0,
                "monotonic",
            );
        }
    }
    ["float_load", "float_store", "vector_load", "vector_store"]
        .into_iter()
        .map(|operation| {
            o3_runtime
                .pointer(&format!("/lsq_operation_{operation}_latency_ticks"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing latency total for {operation}: {o3_runtime}"))
        })
        .sum()
}

#[test]
fn rem6_run_text_stats_alias_o3_lsq_store_load_matches_after_detailed_switch() {
    let path = detailed_o3_lsq_store_load_match_binary(
        "m5-switch-cpu-detailed-o3-lsq-store-load-text-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingCandidates", 1);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingMatches", 1);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.forwLoads", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.store_forwarding_candidates",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.store_forwarding_matches",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.store.storeLoadForwardingCandidates",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.store.storeLoadForwardingMatches",
        0,
    );
}
