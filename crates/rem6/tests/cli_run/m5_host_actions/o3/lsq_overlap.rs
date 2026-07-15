use super::*;

const LOAD_PC: &str = "0x80000014";
const INDEPENDENT_PC: &str = "0x80000018";
const DEPENDENT_PC: &str = "0x8000001c";

#[test]
fn rem6_run_o3_detailed_scalar_load_overlaps_independent_issue_direct() {
    let path = mixed_scalar_load_overlap_binary("o3-scalar-load-overlap-direct");
    let json = mixed_scalar_load_overlap_json(&path, "direct", 260, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_mixed_scalar_load_overlap(&json);
}

#[test]
fn rem6_run_o3_detailed_scalar_load_overlaps_independent_issue_cache_fabric_dram() {
    let path = mixed_scalar_load_overlap_binary("o3-scalar-load-overlap-cache-fabric-dram");
    let json = mixed_scalar_load_overlap_json(&path, "cache-fabric-dram", 420, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_mixed_scalar_load_overlap(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed mixed load run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(&json, path, "Count", 1, "monotonic");
    }
}

#[test]
fn rem6_run_o3_detailed_scalar_load_exposes_younger_row_before_response() {
    let path = mixed_scalar_load_overlap_binary("o3-scalar-load-overlap-resident");
    let completed = mixed_scalar_load_overlap_json(&path, "cache-fabric-dram", 420, None);
    let load = event_at_pc(&completed, LOAD_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let writeback_tick = event_u64(load, "writeback_tick");
    let stop_tick = issue_tick.saturating_add(writeback_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < writeback_tick);

    let json = mixed_scalar_load_overlap_json(&path, "cache-fabric-dram", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000000000000000000000")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident mixed load run should expose ROB rows: {json}"));
    assert_eq!(rob.len(), 2);
    assert_eq!(rob[0].pointer("/pc").and_then(Value::as_str), Some(LOAD_PC));
    assert_eq!(
        rob[1].pointer("/pc").and_then(Value::as_str),
        Some(INDEPENDENT_PC)
    );
    assert!(rob.iter().all(|entry| {
        entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
            && entry.pointer("/ready").and_then(Value::as_bool) == Some(false)
    }));
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries/0")
        .unwrap_or_else(|| panic!("resident mixed load run should expose one LSQ row: {json}"));
    assert_eq!(lsq.pointer("/kind").and_then(Value::as_str), Some("load"));
    assert_eq!(
        lsq.pointer("/address").and_then(Value::as_str),
        Some("0x80000070")
    );
    assert_eq!(lsq.pointer("/bytes").and_then(Value::as_u64), Some(4));
    assert_eq!(
        lsq.pointer("/completed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(lsq.pointer("/sequence"), rob[0].pointer("/sequence"));
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_scalar_load_has_no_younger_o3_issue_window() {
    let path = mixed_scalar_load_overlap_binary("o3-scalar-load-overlap-timing");
    let json = mixed_scalar_load_overlap_json(&path, "direct", 260, Some("timing"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000070000002b000000")
    );
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array")
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
        unexpected.is_empty(),
        "timing mode should suppress mixed-load O3 aliases: {unexpected:?}"
    );
    for path in [
        "sim.debug.o3_trace.records",
        "sim.debug.o3_trace.instructions",
        "sim.debug.o3_trace.max_lsq_occupancy",
        "sim.debug.o3_trace.execution_mode.timing",
        "sim.debug.o3_trace.execution_mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.mode.timing",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
}

fn assert_completed_mixed_scalar_load_overlap(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000070000002b000000")
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(json, "system.cpu.rob.maxOccupancy", "Count", 2, "monotonic");
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );

    let load = event_at_pc(json, LOAD_PC);
    let independent = event_at_pc(json, INDEPENDENT_PC);
    let dependent = event_at_pc(json, DEPENDENT_PC);
    let load_response_tick = event_u64(load, "lsq_data_response_tick");
    let load_writeback_tick = event_u64(load, "writeback_tick");
    assert!(
        load_response_tick < load_writeback_tick,
        "load data response must precede admitted writeback: {load}"
    );
    assert!(
        event_u64(independent, "issue_tick") < load_response_tick,
        "independent younger ALU should issue before the older load response: {independent}"
    );
    assert!(
        event_u64(dependent, "issue_tick") >= load_writeback_tick,
        "load-dependent younger ALU must not issue before admitted load writeback: {dependent}"
    );
    assert!(
        event_u64(load, "commit_tick") <= event_u64(independent, "commit_tick"),
        "younger architectural commit must remain ordered after the load"
    );
    assert!(
        event_u64(independent, "commit_tick") <= event_u64(dependent, "commit_tick"),
        "dependent architectural commit must remain ordered after the independent row"
    );
}

fn mixed_scalar_load_overlap_json(
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
        "0x80000070:12",
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

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn mixed_scalar_load_overlap_binary(name: &str) -> std::path::PathBuf {
    let data_start = 112_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(7, 0, 0x0, 14, 0x13),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 14, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
