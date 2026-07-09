use super::*;

#[test]
fn rem6_run_o3_detailed_mode_exposes_live_lsq_overlap() {
    let path = detailed_o3_live_lsq_overlap_binary("m5-switch-cpu-o3-live-lsq-overlap");
    let json = o3_live_lsq_overlap_json(&path, "direct", "220");

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_o3_live_lsq_overlap(&json);
}

#[test]
fn rem6_run_o3_cache_fabric_dram_preserves_live_lsq_overlap() {
    let path = detailed_o3_live_lsq_overlap_binary("m5-switch-cpu-o3-live-lsq-cache-fabric-dram");
    let json = o3_live_lsq_overlap_json(&path, "cache-fabric-dram", "360");

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_o3_live_lsq_overlap(&json);
    assert_cache_fabric_dram_lsq_resources(&json);
}

fn o3_live_lsq_overlap_json(path: &Path, memory_system: &str, max_tick: &str) -> Value {
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
            memory_system,
            "--dump-memory",
            "0x80000060:8",
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
    json
}

fn assert_o3_live_lsq_overlap(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002b000000"),
        "O3 live-LSQ overlap run should preserve forwarded load data and the younger store result"
    );

    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(&json, "system.cpu.lsq0.loadBytes", "Byte", 4, "monotonic");
    assert_json_stat(&json, "system.cpu.lsq0.storeBytes", "Byte", 8, "monotonic");

    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0),
        "LSQ should drain after ordered memory operations retire: {o3_runtime}"
    );
    assert!(
        o3_runtime
            .pointer("/lsq/max_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 2),
        "O3 runtime JSON should expose live LSQ overlap: {o3_runtime}"
    );
    let max_lsq_event = o3_runtime
        .pointer("/event_window/max_lsq_occupancy")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max LSQ row: {o3_runtime}")
        });
    assert!(
        max_lsq_event
            .pointer("/lsq_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 2),
        "event window should identify the live LSQ overlap row: {max_lsq_event}"
    );
    assert_eq!(
        max_lsq_event.pointer("/pc").and_then(Value::as_str),
        Some("0x80000014"),
        "max LSQ occupancy should occur when the load overlaps the older resident store: {max_lsq_event}"
    );
}

fn assert_cache_fabric_dram_lsq_resources(json: &Value) {
    for (pointer, label) in [
        (
            "/memory_resources/cache/activity",
            "aggregate cache activity",
        ),
        (
            "/memory_resources/cache/data/activity",
            "data cache activity",
        ),
        (
            "/memory_resources/cache/data/msi_runs",
            "data cache MSI runs",
        ),
        (
            "/memory_resources/transport/data/activity",
            "data transport activity",
        ),
        ("/memory_resources/fabric/activity", "fabric activity"),
        ("/memory_resources/dram/activity", "DRAM activity"),
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "cache-fabric-dram live-LSQ run should expose {label}: {json}"
        );
    }

    for path in [
        "sim.memory.resources.cache.activity",
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.cache.data.msi.runs",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
    }
}
