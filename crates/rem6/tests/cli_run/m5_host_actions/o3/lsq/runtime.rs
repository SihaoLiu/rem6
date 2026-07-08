use super::*;

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
            "/lsq/operation/store/count",
            "sim.cpu0.o3.lsq_operation.store",
            3,
        ),
        (
            "/lsq/operation/load_reserved/count",
            "sim.cpu0.o3.lsq_operation.load_reserved",
            1,
        ),
        (
            "/lsq/operation/store_conditional/count",
            "sim.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "/lsq/operation/atomic/count",
            "sim.cpu0.o3.lsq_operation.atomic",
            1,
        ),
        (
            "/lsq/operation/vector_load/count",
            "sim.cpu0.o3.lsq_operation.vector_load",
            0,
        ),
        (
            "/lsq/operation/vector_store/count",
            "sim.cpu0.o3.lsq_operation.vector_store",
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
        for (metric, stat_suffix) in [
            ("samples", "latency_samples"),
            ("ticks", "latency_ticks"),
            ("max_ticks", "latency_max_ticks"),
            ("min_ticks", "latency_min_ticks"),
            ("avg_ticks", "latency_avg_ticks"),
        ] {
            let pointer = format!("/lsq/operation/{operation}/latency/{metric}");
            let stat_path = format!("sim.cpu0.o3.lsq_operation.{operation}_{stat_suffix}");
            let structured = o3_runtime
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
                });
            assert_eq!(
                structured,
                json_stat_value(&json, &stat_path),
                "nested O3 runtime {pointer} should match stat path {stat_path}"
            );
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
            "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        ),
        (
            "/lsq/operation/load/forwarding_matches",
            "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
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
