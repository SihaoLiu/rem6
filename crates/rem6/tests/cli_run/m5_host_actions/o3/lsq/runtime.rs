use super::*;

pub(super) fn assert_o3_lsq_matrix_dump_nested_aliases(
    pre_reset_dump: &Value,
    post_reset_dump: &Value,
) {
    for (path, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.load_bytes",
            8,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.store_bytes",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store.store_bytes",
            24,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved.load_bytes",
            8,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional.store_bytes",
            8,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic.load_bytes",
            8,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic.store_bytes",
            8,
        ),
    ] {
        assert_stats_dump_sample(pre_reset_dump, path, "counter", "Byte", value, "resettable");
    }
    for (dump, path, unit, value) in [
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional.store_conditional_failures",
            "Count",
            0,
        ),
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.latency.samples",
            "Count",
            1,
        ),
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.latency.ticks",
            "Tick",
            2,
        ),
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store.latency.samples",
            "Count",
            3,
        ),
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store.latency.ticks",
            "Tick",
            6,
        ),
        (
            pre_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic.latency.ticks",
            "Tick",
            2,
        ),
        (
            post_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load.latency.samples",
            "Count",
            0,
        ),
        (
            post_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store.latency.samples",
            "Count",
            1,
        ),
        (
            post_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store.latency.ticks",
            "Tick",
            2,
        ),
        (
            post_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional.latency.ticks",
            "Tick",
            0,
        ),
        (
            post_reset_dump,
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional.store_conditional_failures",
            "Count",
            1,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_ordered_atomic_lsq_matrix() {
    let path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-ordered-atomic-lsq-runtime-json",
    );

    let direct_json = ordered_atomic_lsq_runtime_json(&path, Some("direct"), "220");
    let cache_json = ordered_atomic_lsq_runtime_json(&path, None, "320");

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
    let direct_latency = assert_ordered_atomic_lsq_runtime_json(&direct_json);
    let cache_latency = assert_ordered_atomic_lsq_runtime_json(&cache_json);
    assert!(
        cache_latency >= direct_latency,
        "cache-backed LSQ latency should include at least the direct latency: direct={direct_latency}, cache={cache_latency}"
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_nested_rob_lsq_matrices() {
    let path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-nested-rob-lsq-runtime-json",
    );
    let json = ordered_atomic_lsq_runtime_json(&path, Some("direct"), "220");
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (pointer, stat_path) in [
        ("/rob/allocations", "sim.cpu0.o3.rob_allocations"),
        ("/rob/commits", "sim.cpu0.o3.rob_commits"),
        ("/rob/max_occupancy", "sim.cpu0.o3.max_rob_occupancy"),
        ("/rename/writes", "sim.cpu0.o3.rename_writes"),
        ("/rename/map_entries", "sim.cpu0.o3.rename_map_entries"),
        ("/lsq/loads", "sim.cpu0.o3.lsq_loads"),
        ("/lsq/stores", "sim.cpu0.o3.lsq_stores"),
        (
            "/lsq/data_latency/samples",
            "sim.cpu0.o3.lsq_data_latency_samples",
        ),
        (
            "/lsq/data_latency/ticks",
            "sim.cpu0.o3.lsq_data_latency_ticks",
        ),
        (
            "/lsq/data_latency/max_ticks",
            "sim.cpu0.o3.lsq_data_latency_max_ticks",
        ),
        (
            "/lsq/data_latency/min_ticks",
            "sim.cpu0.o3.lsq_data_latency_min_ticks",
        ),
        (
            "/lsq/data_latency/avg_ticks",
            "sim.cpu0.o3.lsq_data_latency_avg_ticks",
        ),
        ("/lsq/max_occupancy", "sim.cpu0.o3.max_lsq_occupancy"),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        assert_eq!(
            structured,
            json_stat_value(&json, stat_path),
            "nested O3 runtime {pointer} should match stat path {stat_path}"
        );
        assert!(
            structured > 0,
            "representative O3 runtime nested lane {pointer} should be positive: {o3_runtime}"
        );
    }

    for (pointer, stat_path, value) in [
        ("/lsq/load_bytes", "sim.cpu0.o3.lsq_load_bytes", 24),
        ("/lsq/store_bytes", "sim.cpu0.o3.lsq_store_bytes", 40),
        (
            "/lsq/store_conditional_failures",
            "sim.cpu0.o3.lsq_store_conditional_failures",
            0,
        ),
        (
            "/lsq/operation/load/count",
            "sim.cpu0.o3.lsq_operation.load",
            1,
        ),
        (
            "/lsq/operation/load/load_bytes",
            "sim.cpu0.o3.lsq_operation.load.load_bytes",
            8,
        ),
        (
            "/lsq_operation_load_load_bytes",
            "sim.cpu0.o3.lsq_operation.load_load_bytes",
            8,
        ),
        (
            "/lsq/operation/load/store_bytes",
            "sim.cpu0.o3.lsq_operation.load.store_bytes",
            0,
        ),
        (
            "/lsq/operation/store/count",
            "sim.cpu0.o3.lsq_operation.store",
            3,
        ),
        (
            "/lsq/operation/store/load_bytes",
            "sim.cpu0.o3.lsq_operation.store.load_bytes",
            0,
        ),
        (
            "/lsq/operation/store/store_bytes",
            "sim.cpu0.o3.lsq_operation.store.store_bytes",
            24,
        ),
        (
            "/lsq/operation/load_reserved/count",
            "sim.cpu0.o3.lsq_operation.load_reserved",
            1,
        ),
        (
            "/lsq/operation/load_reserved/load_bytes",
            "sim.cpu0.o3.lsq_operation.load_reserved.load_bytes",
            8,
        ),
        (
            "/lsq/operation/load_reserved/store_bytes",
            "sim.cpu0.o3.lsq_operation.load_reserved.store_bytes",
            0,
        ),
        (
            "/lsq/operation/store_conditional/count",
            "sim.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "/lsq/operation/store_conditional/load_bytes",
            "sim.cpu0.o3.lsq_operation.store_conditional.load_bytes",
            0,
        ),
        (
            "/lsq/operation/store_conditional/store_bytes",
            "sim.cpu0.o3.lsq_operation.store_conditional.store_bytes",
            8,
        ),
        (
            "/lsq/operation/atomic/count",
            "sim.cpu0.o3.lsq_operation.atomic",
            1,
        ),
        (
            "/lsq/operation/atomic/load_bytes",
            "sim.cpu0.o3.lsq_operation.atomic.load_bytes",
            8,
        ),
        (
            "/lsq/operation/atomic/store_bytes",
            "sim.cpu0.o3.lsq_operation.atomic.store_bytes",
            8,
        ),
        (
            "/lsq_operation_atomic_store_bytes",
            "sim.cpu0.o3.lsq_operation.atomic_store_bytes",
            8,
        ),
        (
            "/lsq/operation/vector_load/count",
            "sim.cpu0.o3.lsq_operation.vector_load",
            0,
        ),
        (
            "/lsq/operation/vector_load/load_bytes",
            "sim.cpu0.o3.lsq_operation.vector_load.load_bytes",
            0,
        ),
        (
            "/lsq/operation/vector_load/store_bytes",
            "sim.cpu0.o3.lsq_operation.vector_load.store_bytes",
            0,
        ),
        (
            "/lsq/operation/vector_store/count",
            "sim.cpu0.o3.lsq_operation.vector_store",
            0,
        ),
        (
            "/lsq/operation/vector_store/load_bytes",
            "sim.cpu0.o3.lsq_operation.vector_store.load_bytes",
            0,
        ),
        (
            "/lsq/operation/vector_store/store_bytes",
            "sim.cpu0.o3.lsq_operation.vector_store.store_bytes",
            0,
        ),
        (
            "/lsq/ordering/acquire",
            "sim.cpu0.o3.lsq_ordering.acquire",
            1,
        ),
        (
            "/lsq/ordering/release",
            "sim.cpu0.o3.lsq_ordering.release",
            1,
        ),
        (
            "/lsq/ordering/acquire_release",
            "sim.cpu0.o3.lsq_ordering.acquire_release",
            1,
        ),
    ] {
        assert_eq!(
            o3_runtime.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {pointer}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, stat_path),
            value,
            "stat path {stat_path} should match nested O3 runtime expectation"
        );
    }
    for operation in [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
        "float_load",
        "float_store",
        "vector_load",
        "vector_store",
    ] {
        for lane in ["load_bytes", "store_bytes"] {
            let pointer = format!("/lsq/operation/{operation}/{lane}");
            let stat_path = format!("sim.cpu0.o3.lsq_operation.{operation}.{lane}");
            let value = o3_runtime
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
                });
            assert_json_stat(&json, &stat_path, "Byte", value, "monotonic");
        }
    }

    let snapshot = o3_runtime
        .pointer("/snapshot")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose final snapshot: {o3_runtime}"));
    let snapshot_count = |pointer: &str| {
        snapshot
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("O3 runtime snapshot should expose {pointer}: {snapshot}"))
    };
    assert_eq!(snapshot_count("/rob/count"), 0);
    assert_eq!(snapshot_count("/lsq/count"), 0);
    assert!(snapshot_count("/rename_map/count") > 0);
    for (pointer, stat_path) in [
        ("/rob/count", "sim.cpu0.o3.snapshot.rob.count"),
        ("/lsq/count", "sim.cpu0.o3.snapshot.lsq.count"),
        ("/rename_map/count", "sim.cpu0.o3.snapshot.rename_map.count"),
    ] {
        let expected = snapshot_count(pointer);
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }
    for (pointer, stat_path) in [
        ("/rob/entries", "sim.cpu0.o3.snapshot.rob.entries"),
        ("/lsq/entries", "sim.cpu0.o3.snapshot.lsq.entries"),
        (
            "/rename_map/entries",
            "sim.cpu0.o3.snapshot.rename_map.entries",
        ),
    ] {
        let expected = snapshot
            .pointer(pointer)
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_else(|| panic!("O3 runtime snapshot should expose {pointer}: {snapshot}"))
            as u64;
        assert_eq!(
            expected,
            snapshot_count(&pointer.replace("/entries", "/count")),
            "snapshot {pointer} entries should match its count: {snapshot}"
        );
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }

    for operation in [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
    ] {
        for metric in ["samples", "ticks", "max_ticks", "min_ticks", "avg_ticks"] {
            let pointer = format!("/lsq/operation/{operation}/latency/{metric}");
            let stat_path = format!("sim.cpu0.o3.lsq_operation.{operation}.latency.{metric}");
            let unit = if metric == "samples" { "Count" } else { "Tick" };
            let structured = o3_runtime
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
                });
            assert_json_stat(&json, &stat_path, unit, structured, "monotonic");
            assert!(
                structured > 0,
                "active LSQ operation latency metric {pointer} should be positive: {o3_runtime}"
            );
        }
    }
    for operation in ["float_load", "float_store", "vector_load", "vector_store"] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/lsq/operation/{operation}/latency/ticks"))
                .and_then(Value::as_u64),
            Some(0),
            "inactive LSQ operation latency should stay zero for {operation}: {o3_runtime}"
        );
    }

    let forwarding_path =
        detailed_o3_lsq_store_load_match_binary("m5-switch-cpu-o3-nested-lsq-forwarding-json");
    let forwarding_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            forwarding_path.to_str().unwrap(),
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
        forwarding_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&forwarding_output.stderr)
    );
    let forwarding_json: Value = serde_json::from_slice(&forwarding_output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let forwarding_o3 = forwarding_json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| {
            panic!("run JSON should include core O3 runtime state: {forwarding_json}")
        });
    for (pointer, stat_path) in [
        (
            "/lsq/store_load_forwarding_candidates",
            "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        ),
        (
            "/lsq/store_load_forwarding_matches",
            "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        ),
        (
            "/lsq/operation/load/forwarding_candidates",
            "sim.cpu0.o3.lsq_operation.load.forwarding_candidates",
        ),
        (
            "/lsq/operation/load/forwarding_matches",
            "sim.cpu0.o3.lsq_operation.load.forwarding_matches",
        ),
    ] {
        assert_eq!(
            forwarding_o3.pointer(pointer).and_then(Value::as_u64),
            Some(1),
            "nested O3 forwarding JSON should expose {pointer}: {forwarding_o3}"
        );
        assert_json_stat(&forwarding_json, stat_path, "Count", 1, "monotonic");
    }
    for pointer in [
        "/lsq/operation/store/forwarding_candidates",
        "/lsq/operation/store/forwarding_matches",
        "/lsq/operation/atomic/forwarding_candidates",
        "/lsq/operation/atomic/forwarding_matches",
    ] {
        assert_eq!(
            forwarding_o3.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "inactive O3 forwarding lane should stay zero at {pointer}: {forwarding_o3}"
        );
    }
}

#[test]
fn rem6_run_o3_debug_lsq_json_exposes_float_vector_operation_byte_lanes() {
    let path = detailed_o3_float_vector_lsq_binary(
        "m5-switch-cpu-detailed-o3-debug-lsq-float-vector-byte-lanes",
    );
    let json = o3_lsq_debug_runtime_json(&path, "220");
    let runtime_operations = json
        .pointer("/cores/0/o3_runtime/lsq/operation")
        .unwrap_or_else(|| panic!("run JSON should include runtime LSQ operations: {json}"));
    let debug_operations = json
        .pointer("/debug/o3_trace/0/lsq/operation")
        .unwrap_or_else(|| panic!("O3 debug trace should include LSQ operations: {json}"));

    for (operation, active_lane, inactive_lane) in [
        ("float_load", "load_bytes", "store_bytes"),
        ("float_store", "store_bytes", "load_bytes"),
        ("vector_load", "load_bytes", "store_bytes"),
        ("vector_store", "store_bytes", "load_bytes"),
    ] {
        for lane in ["load_bytes", "store_bytes"] {
            let pointer = format!("/{operation}/{lane}");
            let expected = runtime_operations
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "runtime LSQ operation byte lane {pointer} missing: {runtime_operations}"
                    )
                });
            assert_eq!(
                debug_operations.pointer(&pointer).and_then(Value::as_u64),
                Some(expected),
                "debug LSQ operation byte lane {pointer} should mirror runtime LSQ operations: {debug_operations}"
            );
        }
        assert!(
            debug_operations
                .pointer(&format!("/{operation}/{active_lane}"))
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "active debug LSQ byte lane should be positive for {operation}: {debug_operations}"
        );
        assert_eq!(
            debug_operations
                .pointer(&format!("/{operation}/{inactive_lane}"))
                .and_then(Value::as_u64),
            Some(0),
            "inactive debug LSQ byte lane should stay zero for {operation}: {debug_operations}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_event_summary_lsq_matrix_stats() {
    let ordered_path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-event-summary-lsq-matrix-stats",
    );
    let ordered_json = o3_lsq_debug_runtime_json(&ordered_path, "220");
    let ordered_o3 = ordered_json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {ordered_json}"));
    let ordered_summary = ordered_o3
        .pointer("/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {ordered_o3}"));
    let ordered_debug_summary = ordered_json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {ordered_json}"));
    let ordered_events = ordered_json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("O3 debug trace should expose events: {ordered_json}"));
    let expected_lsq_load_bytes = ordered_events
        .iter()
        .map(|event| {
            event
                .pointer("/lsq_load_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .sum::<u64>();
    let expected_lsq_store_bytes = ordered_events
        .iter()
        .map(|event| {
            event
                .pointer("/lsq_store_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .sum::<u64>();
    let operation_byte_totals = |operation: &str| {
        ordered_events
            .iter()
            .filter(|event| {
                event.pointer("/lsq_operation").and_then(Value::as_str) == Some(operation)
            })
            .map(|event| {
                (
                    event
                        .pointer("/lsq_load_bytes")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    event
                        .pointer("/lsq_store_bytes")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                )
            })
            .fold(
                (0_u64, 0_u64),
                |(load_total, store_total), (load, store)| (load_total + load, store_total + store),
            )
    };

    for pointer in [
        "/lsq_operation/load/latency/ticks",
        "/lsq_operation/store/latency/ticks",
        "/lsq_operation/load_reserved/latency/samples",
        "/lsq_operation/store_conditional/latency/ticks",
        "/lsq_operation/atomic/latency/avg_ticks",
        "/lsq_ordering/acquire",
        "/lsq_ordering/release",
        "/lsq_ordering/acquire_release",
    ] {
        assert_eq!(
            ordered_summary.pointer(pointer),
            ordered_debug_summary.pointer(pointer),
            "runtime event-summary LSQ lane {pointer} should mirror debug trace event summary"
        );
        assert!(
            ordered_summary
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative runtime event-summary LSQ lane {pointer} should be positive: {ordered_summary}"
        );
    }
    for (pointer, stat_path, expected) in [
        (
            "/lsq_load_bytes",
            "sim.cpu0.o3.event_summary.lsq_load_bytes",
            expected_lsq_load_bytes,
        ),
        (
            "/lsq_store_bytes",
            "sim.cpu0.o3.event_summary.lsq_store_bytes",
            expected_lsq_store_bytes,
        ),
    ] {
        assert!(
            expected > 0,
            "representative O3 event stream should have byte traffic for {pointer}: {ordered_events:?}"
        );
        assert_eq!(
            ordered_summary.pointer(pointer),
            ordered_debug_summary.pointer(pointer),
            "runtime event-summary byte lane {pointer} should mirror debug trace event summary"
        );
        assert_eq!(
            ordered_summary.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "event summary byte lane {pointer} should match emitted O3 events: {ordered_summary}"
        );
        assert_json_stat(&ordered_json, stat_path, "Byte", expected, "monotonic");
    }
    for operation in [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
    ] {
        let (expected_load_bytes, expected_store_bytes) = operation_byte_totals(operation);
        assert!(
            expected_load_bytes > 0 || expected_store_bytes > 0,
            "representative O3 event stream should have operation byte traffic for {operation}: {ordered_events:?}"
        );
        for (lane, expected) in [
            ("load_bytes", expected_load_bytes),
            ("store_bytes", expected_store_bytes),
        ] {
            let pointer = format!("/lsq_operation/{operation}/{lane}");
            assert_eq!(
                ordered_summary.pointer(&pointer),
                ordered_debug_summary.pointer(&pointer),
                "runtime event-summary operation byte lane {pointer} should mirror debug trace event summary"
            );
            assert_eq!(
                ordered_summary.pointer(&pointer).and_then(Value::as_u64),
                Some(expected),
                "event-summary operation byte lane {pointer} should match emitted O3 events: {ordered_summary}"
            );
            assert_json_stat(
                &ordered_json,
                &format!("sim.cpu0.o3.event_summary.lsq_operation.{operation}.{lane}"),
                "Byte",
                expected,
                "monotonic",
            );
        }
    }
    for (pointer, stat_path, unit) in [
        (
            "/lsq_operation/load/latency/ticks",
            "sim.cpu0.o3.event_summary.lsq_operation.load.latency.ticks",
            "Tick",
        ),
        (
            "/lsq_operation/store/latency/ticks",
            "sim.cpu0.o3.event_summary.lsq_operation.store.latency.ticks",
            "Tick",
        ),
        (
            "/lsq_operation/load_reserved/latency/samples",
            "sim.cpu0.o3.event_summary.lsq_operation.load_reserved.latency.samples",
            "Count",
        ),
        (
            "/lsq_operation/store_conditional/latency/ticks",
            "sim.cpu0.o3.event_summary.lsq_operation.store_conditional.latency.ticks",
            "Tick",
        ),
        (
            "/lsq_operation/atomic/latency/avg_ticks",
            "sim.cpu0.o3.event_summary.lsq_operation.atomic.latency.avg_ticks",
            "Tick",
        ),
        (
            "/lsq_ordering/acquire",
            "sim.cpu0.o3.event_summary.lsq_ordering.acquire",
            "Count",
        ),
        (
            "/lsq_ordering/release",
            "sim.cpu0.o3.event_summary.lsq_ordering.release",
            "Count",
        ),
        (
            "/lsq_ordering/acquire_release",
            "sim.cpu0.o3.event_summary.lsq_ordering.acquire_release",
            "Count",
        ),
    ] {
        let expected = ordered_summary
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime event summary should expose u64 lane {pointer}: {ordered_summary}")
            });
        assert_json_stat(&ordered_json, stat_path, unit, expected, "monotonic");
    }

    let forwarding_path = detailed_o3_lsq_store_load_match_binary(
        "m5-switch-cpu-detailed-o3-event-summary-lsq-forwarding-stats",
    );
    let forwarding_json = o3_lsq_debug_runtime_json(&forwarding_path, "140");
    let forwarding_summary = forwarding_json
        .pointer("/cores/0/o3_runtime/event_summary")
        .unwrap_or_else(|| {
            panic!("O3 runtime JSON should expose forwarding event summary: {forwarding_json}")
        });
    for (pointer, stat_path) in [
        (
            "/store_load_forwarding_candidates",
            "sim.cpu0.o3.event_summary.store_load_forwarding_candidates",
        ),
        (
            "/store_load_forwarding_matches",
            "sim.cpu0.o3.event_summary.store_load_forwarding_matches",
        ),
        (
            "/lsq_operation/load/forwarding_candidates",
            "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_candidates",
        ),
        (
            "/lsq_operation/load/forwarding_matches",
            "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_matches",
        ),
    ] {
        assert_eq!(
            forwarding_summary.pointer(pointer).and_then(Value::as_u64),
            Some(1),
            "runtime event-summary forwarding lane {pointer} should be positive: {forwarding_summary}"
        );
        assert_json_stat(&forwarding_json, stat_path, "Count", 1, "monotonic");
    }

    for (
        suppressed_path,
        mismatch_pointer,
        mismatch_stat_path,
        operation_mismatch_pointer,
        operation_mismatch_stat_path,
    ) in [
        (
            detailed_o3_lsq_store_load_mismatch_binary(
                "m5-switch-cpu-detailed-o3-event-summary-lsq-forwarding-address-stats",
            ),
            "/store_load_forwarding_address_mismatches",
            "sim.cpu0.o3.event_summary.store_load_forwarding_address_mismatches",
            "/lsq_operation/load/forwarding_address_mismatches",
            "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_address_mismatches",
        ),
        (
            detailed_o3_lsq_store_load_byte_mismatch_binary(
                "m5-switch-cpu-detailed-o3-event-summary-lsq-forwarding-byte-stats",
            ),
            "/store_load_forwarding_byte_mismatches",
            "sim.cpu0.o3.event_summary.store_load_forwarding_byte_mismatches",
            "/lsq_operation/load/forwarding_byte_mismatches",
            "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_byte_mismatches",
        ),
    ] {
        let suppressed_json = o3_lsq_debug_runtime_json(&suppressed_path, "140");
        let suppressed_summary = suppressed_json
            .pointer("/cores/0/o3_runtime/event_summary")
            .unwrap_or_else(|| {
                panic!("O3 runtime JSON should expose suppressed event summary: {suppressed_json}")
            });
        for (pointer, stat_path) in [
            (
                "/store_load_forwarding_suppressed",
                "sim.cpu0.o3.event_summary.store_load_forwarding_suppressed",
            ),
            (
                "/lsq_operation/load/forwarding_suppressed",
                "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_suppressed",
            ),
            (mismatch_pointer, mismatch_stat_path),
            (operation_mismatch_pointer, operation_mismatch_stat_path),
        ] {
            assert_eq!(
                suppressed_summary.pointer(pointer).and_then(Value::as_u64),
                Some(1),
                "runtime event-summary forwarding-suppression lane {pointer} should be positive: {suppressed_summary}"
            );
            assert_json_stat(&suppressed_json, stat_path, "Count", 1, "monotonic");
        }
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_event_summary_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-event-summary-store-conditional-failures",
    );
    let json = o3_lsq_debug_runtime_json(&path, "180");
    let runtime_summary = json
        .pointer("/cores/0/o3_runtime/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {json}"));
    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));
    let debug_events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("O3 debug trace should expose events: {json}"));
    let failed_store_conditional_events = debug_events
        .iter()
        .filter(|event| {
            event.pointer("/lsq_operation").and_then(Value::as_str) == Some("store_conditional")
                && event
                    .pointer("/lsq_store_conditional_failed")
                    .and_then(Value::as_bool)
                    == Some(true)
        })
        .count() as u64;

    for summary in [runtime_summary, debug_summary] {
        assert_eq!(
            summary
                .pointer("/lsq_store_conditional_failures")
                .and_then(Value::as_u64),
            Some(failed_store_conditional_events),
            "event summary should count failed store-conditionals: {summary}"
        );
    }
    assert_eq!(
        failed_store_conditional_events, 1,
        "debug events should expose exactly one failed store-conditional: {debug_events:?}"
    );
    assert!(
        debug_events.iter().all(|event| {
            event
                .pointer("/lsq_store_conditional_failed")
                .and_then(Value::as_bool)
                != Some(true)
                || event.pointer("/lsq_operation").and_then(Value::as_str)
                    == Some("store_conditional")
        }),
        "only store-conditional events should be flagged as failed: {debug_events:?}"
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.event_summary.lsq_store_conditional_failures",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_store_conditional_failure_operation_lane() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-store-conditional-failure-operation-lane",
    );
    let json = o3_lsq_debug_runtime_json(&path, "180");
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    let debug_lsq = json
        .pointer("/debug/o3_trace/0/lsq")
        .unwrap_or_else(|| panic!("O3 debug trace should expose LSQ matrix: {json}"));

    for (pointer, value) in [
        (
            "/lsq/operation/store_conditional/store_conditional_failures",
            1,
        ),
        ("/lsq/operation/store/store_conditional_failures", 0),
        (
            "/lsq_operation_store_conditional_store_conditional_failures",
            1,
        ),
        ("/lsq_operation_store_store_conditional_failures", 0),
        (
            "/event_summary/lsq_operation/store_conditional/store_conditional_failures",
            1,
        ),
        (
            "/event_summary/lsq_operation/store/store_conditional_failures",
            0,
        ),
    ] {
        assert_eq!(
            o3_runtime.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "O3 runtime JSON should expose failed-SC operation lane {pointer}: {o3_runtime}"
        );
    }
    assert_eq!(
        debug_lsq
            .pointer("/operation/store_conditional/store_conditional_failures")
            .and_then(Value::as_u64),
        Some(1),
        "O3 debug LSQ JSON should expose the failed-SC operation lane: {debug_lsq}"
    );

    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));
    for (pointer, value) in [
        (
            "/lsq_operation/store_conditional/store_conditional_failures",
            1,
        ),
        ("/lsq_operation/store/store_conditional_failures", 0),
    ] {
        assert_eq!(
            debug_summary.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "O3 debug event summary should expose failed-SC operation lane {pointer}: {debug_summary}"
        );
    }
    for operation in [
        "load",
        "store",
        "load_reserved",
        "atomic",
        "float_load",
        "float_store",
        "vector_load",
        "vector_store",
    ] {
        for (scope, pointer) in [
            (
                "runtime LSQ",
                format!("/lsq/operation/{operation}/store_conditional_failures"),
            ),
            (
                "runtime flat",
                format!("/lsq_operation_{operation}_store_conditional_failures"),
            ),
            (
                "runtime event summary",
                format!("/event_summary/lsq_operation/{operation}/store_conditional_failures"),
            ),
        ] {
            assert_eq!(
                o3_runtime.pointer(&pointer).and_then(Value::as_u64),
                Some(0),
                "{scope} failed-SC operation lane should stay zero for {operation}: {o3_runtime}"
            );
        }
        assert_eq!(
            debug_lsq
                .pointer(&format!(
                    "/operation/{operation}/store_conditional_failures"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "O3 debug LSQ failed-SC operation lane should stay zero for {operation}: {debug_lsq}"
        );
        assert_eq!(
            debug_summary
                .pointer(&format!(
                    "/lsq_operation/{operation}/store_conditional_failures"
                ))
                .and_then(Value::as_u64),
            Some(0),
            "O3 debug event-summary failed-SC operation lane should stay zero for {operation}: {debug_summary}"
        );
        for path in [
            format!("sim.cpu0.o3.lsq_operation.{operation}_store_conditional_failures"),
            format!("sim.cpu0.o3.lsq_operation.{operation}.store_conditional_failures"),
            format!(
                "sim.cpu0.o3.event_summary.lsq_operation.{operation}.store_conditional_failures"
            ),
        ] {
            assert_json_stat(&json, &path, "Count", 0, "monotonic");
        }
    }

    for (path, value) in [
        (
            "sim.cpu0.o3.lsq_operation.store_conditional_store_conditional_failures",
            1,
        ),
        (
            "sim.cpu0.o3.lsq_operation.store_conditional.store_conditional_failures",
            1,
        ),
        (
            "sim.cpu0.o3.lsq_operation.store_store_conditional_failures",
            0,
        ),
        (
            "sim.cpu0.o3.event_summary.lsq_operation.store_conditional.store_conditional_failures",
            1,
        ),
        (
            "sim.cpu0.o3.event_summary.lsq_operation.store.store_conditional_failures",
            0,
        ),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }
}

fn ordered_atomic_lsq_runtime_json(
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

fn o3_lsq_debug_runtime_json(path: &Path, max_tick: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
            "--debug-flags",
            "O3",
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
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_ordered_atomic_lsq_runtime_json(json: &Value) -> u64 {
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_load", 1),
        ("lsq_operation_store", 3),
        ("lsq_operation_load_reserved", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_operation_atomic", 1),
        ("lsq_operation_float_load", 0),
        ("lsq_operation_float_store", 0),
        ("lsq_operation_vector_load", 0),
        ("lsq_operation_vector_store", 0),
        ("lsq_ordering_acquire", 1),
        ("lsq_ordering_release", 1),
        ("lsq_ordering_acquire_release", 1),
        ("lsq_store_conditional_failures", 0),
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
        assert_o3_lsq_count_alias(json, field, value);
    }
    assert_o3_lsq_count_alias_totals(json, 7, 3);

    let mut aggregate_latency_samples = 0;
    let mut aggregate_latency_ticks = 0;
    let mut aggregate_latency_max_ticks = 0;
    let mut aggregate_latency_min_ticks = u64::MAX;
    for (operation, alias_operation) in [
        ("load", "load"),
        ("store", "store"),
        ("load_reserved", "loadReserved"),
        ("store_conditional", "storeConditional"),
        ("atomic", "atomic"),
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
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.operation.{alias_operation}.dataResponse.{suffix}"),
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
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
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
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.operation.{alias_operation}.dataResponse.{alias}"),
                unit,
                0,
                "monotonic",
            );
        }
    }
    [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
    ]
    .into_iter()
    .map(|operation| {
        o3_runtime
            .pointer(&format!("/lsq_operation_{operation}_latency_ticks"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing latency total for {operation}: {o3_runtime}"))
    })
    .sum()
}
