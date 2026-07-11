use super::*;

const SCALAR_STORE_PC: &str = "0x80000010";
const SCALAR_LOAD_PC: &str = "0x80000014";
const YOUNGER_STORE_PC: &str = "0x8000001c";
const RESIDENT_LOAD_PC: &str = "0x8000000c";
const RESIDENT_LOAD_DEPENDENT_PC: &str = "0x80000010";
const MMIO_LOAD_PC: &str = "0x80000008";

#[test]
fn rem6_run_o3_detailed_scalar_memory_issue_response_retire_direct() {
    let path = detailed_o3_scalar_memory_lifecycle_binary("m5-switch-cpu-o3-scalar-memory-direct");
    let json = scalar_memory_lifecycle_json(&path, "direct", 220, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_scalar_memory_lifecycle(&json);
}

#[test]
fn rem6_run_o3_detailed_scalar_memory_issue_response_retire_cache_fabric_dram() {
    let path = detailed_o3_scalar_memory_lifecycle_binary(
        "m5-switch-cpu-o3-scalar-memory-cache-fabric-dram",
    );
    let json = scalar_memory_lifecycle_json(&path, "cache-fabric-dram", 360, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_scalar_memory_lifecycle(&json);
    assert_cache_fabric_dram_scalar_memory_resources(&json);
}

#[test]
fn rem6_run_o3_detailed_scalar_memory_instruction_limit_retires_response_direct() {
    let path = detailed_o3_scalar_memory_lifecycle_binary(
        "m5-switch-cpu-o3-scalar-memory-instruction-limit-direct",
    );
    let json = scalar_memory_instruction_limit_json(&path, "direct", 220);

    assert_response_owned_instruction_limit(&json, "direct");
}

#[test]
fn rem6_run_o3_detailed_scalar_memory_instruction_limit_retires_response_cache_fabric_dram() {
    let path = detailed_o3_scalar_memory_lifecycle_binary(
        "m5-switch-cpu-o3-scalar-memory-instruction-limit-cache-fabric-dram",
    );
    let json = scalar_memory_instruction_limit_json(&path, "cache-fabric-dram", 360);

    assert_response_owned_instruction_limit(&json, "cache-fabric-dram");
    assert_cache_fabric_dram_scalar_memory_resources(&json);
}

#[test]
fn rem6_run_o3_detailed_mmio_instruction_limit_retires_response() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        u_type(0x1000_0000, 10, 0x37),
        i_type(0, 10, 0b011, 5, 0x03),
        m5op(M5_EXIT),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-o3-mmio-instruction-limit", &elf);
    let readfile_path = temp_binary(
        "m5-switch-cpu-o3-mmio-instruction-limit-data",
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--max-instructions",
            "3",
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
            "--readfile",
            &format!("0x10000000:0x100:{}", readfile_path.display()),
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

    assert_instruction_limit_summary(&json, 3);
    assert_eq!(
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some("0x123456789abcdef")
    );
    assert_empty_scalar_memory_snapshot(&json);
    let load = scalar_memory_event_at_pc(&json, MMIO_LOAD_PC);
    assert_scalar_memory_event(load, "load", "0x10000000", 8, 0, 1, 1, false);
}

#[test]
fn rem6_run_o3_detailed_scalar_load_is_resident_before_response() {
    let path = detailed_o3_scalar_load_resident_binary("m5-switch-cpu-o3-scalar-memory-resident");
    let completed = scalar_memory_lifecycle_json(&path, "cache-fabric-dram", 360, None);
    let completed_load = scalar_memory_event_at_pc(&completed, RESIDENT_LOAD_PC);
    let issue_tick = event_u64(completed_load, "issue_tick");
    let writeback_tick = event_u64(completed_load, "writeback_tick");
    let stop_tick = issue_tick.saturating_add(writeback_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < writeback_tick);

    let json = scalar_memory_lifecycle_json(&path, "cache-fabric-dram", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a00000000000000")
    );

    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("early run should expose resident ROB rows: {json}"));
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries/0")
        .unwrap_or_else(|| panic!("early run should expose one resident LSQ row: {json}"));
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        rob[0].pointer("/pc").and_then(Value::as_str),
        Some(RESIDENT_LOAD_PC)
    );
    assert_eq!(
        rob[1].pointer("/pc").and_then(Value::as_str),
        Some(RESIDENT_LOAD_DEPENDENT_PC)
    );
    assert!(rob.iter().all(|entry| {
        entry.pointer("/ready").and_then(Value::as_bool) == Some(false)
            && entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
    }));
    assert_eq!(lsq.pointer("/kind").and_then(Value::as_str), Some("load"));
    assert_eq!(
        lsq.pointer("/address").and_then(Value::as_str),
        Some("0x80000060")
    );
    assert_eq!(lsq.pointer("/bytes").and_then(Value::as_u64), Some(4));
    assert_eq!(
        lsq.pointer("/completed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(rob[0].pointer("/sequence"), lsq.pointer("/sequence"));
}

#[test]
fn rem6_run_o3_timing_scalar_memory_has_no_live_o3_lsq() {
    let path =
        detailed_o3_scalar_memory_lifecycle_binary("m5-switch-cpu-o3-scalar-memory-timing-control");
    let json = scalar_memory_lifecycle_json(&path, "direct", 220, Some("timing"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002b000000")
    );
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array");
    let o3_paths = stats
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        o3_paths.is_empty(),
        "timing mode should not expose core or gem5-style O3 stat paths: {o3_paths:?}"
    );
    for path in [
        "sim.debug.o3_trace.records",
        "sim.debug.o3_trace.instructions",
        "sim.debug.o3_trace.max_lsq_occupancy",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.execution_mode.timing",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.execution_mode.detailed",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.debug.o3_trace.execution_mode_authority.mode.timing",
        "Count",
        0,
        "monotonic",
    );
}

fn scalar_memory_lifecycle_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3",
        "--memory-system",
        memory_system,
        "--dump-memory",
        "0x80000060:8",
    ]);
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
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

fn detailed_o3_scalar_load_resident_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn scalar_memory_instruction_limit_json(path: &Path, memory_system: &str, max_tick: u64) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick.to_string(),
            "--stats-format",
            "json",
            "--execute",
            "--max-instructions",
            "6",
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
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_response_owned_instruction_limit(json: &Value, memory_system: &str) {
    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some(memory_system)
    );
    assert_instruction_limit_summary(json, 6);
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a"),
        "the final load must complete before the instruction limit stops execution"
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a00000000000000"),
        "the instruction limit must stop before the dependent younger store"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0),
        "the instruction limit must clean up the uncommitted dependent row after the forwarded load retires"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0),
        "the response-owned scalar LSQ row must drain before the instruction limit stops"
    );
    let load = scalar_memory_event_at_pc(json, SCALAR_LOAD_PC);
    assert_scalar_memory_event(load, "load", "0x80000060", 4, 0, 2, 2, true);
}

fn assert_instruction_limit_summary(json: &Value, committed: u64) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_instruction_limit")
    );
    assert_eq!(
        json.pointer("/simulation/stop_reason")
            .and_then(Value::as_str),
        Some("instruction_limit")
    );
    assert_eq!(
        json.pointer("/cores/0/committed_instructions")
            .and_then(Value::as_u64),
        Some(committed)
    );
    assert_eq!(
        json.pointer("/simulation/instruction_probes/tracked_instructions")
            .and_then(Value::as_u64),
        Some(committed)
    );
}

fn assert_empty_scalar_memory_snapshot(json: &Value) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

fn assert_completed_scalar_memory_lifecycle(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002b000000"),
        "scalar memory lifecycle should preserve completed load/store data and the younger store"
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(json, "system.cpu.lsq0.loadBytes", "Byte", 4, "monotonic");
    assert_json_stat(json, "system.cpu.lsq0.storeBytes", "Byte", 8, "monotonic");

    let runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        runtime
            .pointer("/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/rob/max_occupancy")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        runtime
            .pointer("/lsq/max_occupancy")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        runtime
            .pointer("/event_window/max_lsq_occupancy/lsq_occupancy")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_store = scalar_memory_event_at_pc(json, SCALAR_STORE_PC);
    assert_scalar_memory_event(first_store, "store", "0x80000060", 0, 4, 1, 1, false);
    let load = scalar_memory_event_at_pc(json, SCALAR_LOAD_PC);
    assert_scalar_memory_event(load, "load", "0x80000060", 4, 0, 2, 2, true);
    let younger_store = scalar_memory_event_at_pc(json, YOUNGER_STORE_PC);
    assert_scalar_memory_event(younger_store, "store", "0x80000064", 0, 4, 1, 1, false);
}

fn assert_scalar_memory_event(
    event: &Value,
    operation: &str,
    address: &str,
    load_bytes: u64,
    store_bytes: u64,
    expected_rob_occupancy: u64,
    expected_lsq_occupancy: u64,
    forwarded: bool,
) {
    let issue_tick = event_u64(event, "issue_tick");
    let writeback_tick = event_u64(event, "writeback_tick");
    let commit_tick = event_u64(event, "commit_tick");
    if forwarded {
        assert_eq!(
            issue_tick, writeback_tick,
            "forwarded loads must complete locally at their issue tick: {event}"
        );
    } else {
        assert!(
            issue_tick < writeback_tick,
            "transport request issue must precede response: {event}"
        );
    }
    assert!(
        writeback_tick <= commit_tick,
        "response must precede commit: {event}"
    );
    assert_eq!(event_u64(event, "lsq_data_response_tick"), writeback_tick);
    assert_eq!(
        event_u64(event, "lsq_data_latency_ticks"),
        writeback_tick.saturating_sub(issue_tick)
    );
    assert_eq!(
        event.pointer("/rob_occupancy").and_then(Value::as_u64),
        Some(expected_rob_occupancy)
    );
    assert_eq!(
        event.pointer("/lsq_occupancy").and_then(Value::as_u64),
        Some(expected_lsq_occupancy)
    );
    assert_eq!(
        event.pointer("/lsq_operation").and_then(Value::as_str),
        Some(operation)
    );
    assert_eq!(
        event.pointer("/lsq_load_bytes").and_then(Value::as_u64),
        Some(load_bytes)
    );
    assert_eq!(
        event.pointer("/lsq_store_bytes").and_then(Value::as_u64),
        Some(store_bytes)
    );
    let address_pointer = if operation == "load" {
        "/lsq_load_address"
    } else {
        "/lsq_store_address"
    };
    assert_eq!(
        event.pointer(address_pointer).and_then(Value::as_str),
        Some(address)
    );
}

fn scalar_memory_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include scalar memory event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn assert_cache_fabric_dram_scalar_memory_resources(json: &Value) {
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
            "cache-fabric-dram scalar memory run should expose {label}: {json}"
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
