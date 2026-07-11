use std::collections::BTreeSet;

use super::*;

const LOAD_PC: &str = "0x80000014";
const FIRST_ALU_PC: &str = "0x80000018";
const SECOND_ALU_PC: &str = "0x8000001c";
const THIRD_ALU_PC: &str = "0x80000020";
const DATA_ADDRESS: &str = "0x80000080";
const POSITIVE_RESULTS: &str = "2a000000050000001000000015000000";
const DEPENDENT_RESULTS: &str = "2a000000050000003500000036000000";

#[test]
fn rem6_run_o3_scalar_load_head_issues_three_younger_alus_before_response_direct() {
    let path = scalar_load_fu_window_binary("o3-scalar-load-fu-window-direct");
    let json = scalar_load_fu_window_json(&path, "direct", 1_500, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "direct load-head FU run should bypass {pointer}: {json}"
        );
    }
    assert_completed_scalar_load_fu_window(&json);
}

#[test]
fn rem6_run_o3_scalar_load_head_issues_three_younger_alus_cache_fabric_dram() {
    let path = scalar_load_fu_window_binary("o3-scalar-load-fu-window-cache-fabric-dram");
    let json = scalar_load_fu_window_json(&path, "cache-fabric-dram", 1_500, None);

    assert_completed_scalar_load_fu_window(&json);
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
            "hierarchy-backed load-head FU run should expose {pointer}: {json}"
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
fn rem6_run_o3_scalar_load_head_exposes_four_resident_rows_before_response() {
    let path = scalar_load_fu_window_binary("o3-scalar-load-fu-window-resident");
    let completed = scalar_load_fu_window_json(&path, "cache-fabric-dram", 1_500, None);
    let load = event_at_pc(&completed, LOAD_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let stop_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < response_tick);

    let json = scalar_load_fu_window_json(&path, "cache-fabric-dram", stop_tick, None);

    assert_resident_scalar_load_fu_window(
        &json,
        &[LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, THIRD_ALU_PC],
        4,
        stop_tick,
    );
}

#[test]
fn rem6_run_o3_scalar_load_head_stops_at_load_dependent_boundary() {
    let path = scalar_load_dependent_window_binary("o3-scalar-load-dependent-window");
    let completed = scalar_load_fu_window_json(&path, "cache-fabric-dram", 1_500, None);

    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(DEPENDENT_RESULTS)
    );
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x35"),
        ("x15", "0x36"),
    ] {
        assert_eq!(
            completed
                .pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "negative load-head register {register} should preserve architecture: {completed}"
        );
    }
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    let load = event_at_pc(&completed, LOAD_PC);
    let independent = event_at_pc(&completed, FIRST_ALU_PC);
    let dependent = event_at_pc(&completed, SECOND_ALU_PC);
    let downstream = event_at_pc(&completed, THIRD_ALU_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(independent, "issue_tick") < response_tick);
    assert!(event_u64(dependent, "issue_tick") >= response_tick);
    assert!(event_u64(downstream, "issue_tick") >= event_u64(dependent, "writeback_tick"));
    assert!(event_u64(load, "commit_tick") <= event_u64(dependent, "commit_tick"));
    assert!(event_u64(dependent, "commit_tick") <= event_u64(downstream, "commit_tick"));

    let issue_tick = event_u64(load, "issue_tick");
    let stop_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < response_tick);
    let resident = scalar_load_fu_window_json(&path, "cache-fabric-dram", stop_tick, None);
    assert_resident_scalar_load_fu_window(
        &resident,
        &[LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC],
        3,
        stop_tick,
    );
}

#[test]
fn rem6_run_timing_scalar_load_head_has_no_detailed_o3_window() {
    let path = scalar_load_fu_window_binary("o3-scalar-load-fu-window-timing");
    let json = scalar_load_fu_window_json(&path, "direct", 1_500, Some("timing"));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(POSITIVE_RESULTS)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x15"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "timing load-head register {register} should preserve architecture: {json}"
        );
    }
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
        "timing mode should suppress load-head FU O3 aliases: {unexpected:?}"
    );
    for path in [
        "sim.debug.o3_trace.records",
        "sim.debug.o3_trace.instructions",
        "sim.debug.o3_trace.max_rob_occupancy",
        "sim.debug.o3_trace.max_lsq_occupancy",
        "sim.debug.o3_trace.execution_mode.timing",
        "sim.debug.o3_trace.execution_mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.mode.timing",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
}

fn assert_completed_scalar_load_fu_window(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(POSITIVE_RESULTS)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x15"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve load-head FU semantics: {json}"
        );
    }
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(json, "system.cpu.rob.maxOccupancy", "Count", 4, "monotonic");

    let load = event_at_pc(json, LOAD_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let mut previous_issue = event_u64(load, "issue_tick");
    let mut previous_commit = event_u64(load, "commit_tick");
    for pc in [FIRST_ALU_PC, SECOND_ALU_PC, THIRD_ALU_PC] {
        let younger = event_at_pc(json, pc);
        let issue_tick = event_u64(younger, "issue_tick");
        let commit_tick = event_u64(younger, "commit_tick");
        assert!(
            previous_issue <= issue_tick && issue_tick < response_tick,
            "younger scalar ALU at {pc} should issue before the load response: {younger}"
        );
        assert!(
            previous_commit <= commit_tick,
            "younger scalar ALU at {pc} must commit after the load: {younger}"
        );
        previous_issue = issue_tick;
        previous_commit = commit_tick;
    }

    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("load-head FU run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), 4);
    let observed_data = data_trace
        .iter()
        .map(|record| {
            (
                record.pointer("/kind").and_then(Value::as_str).unwrap(),
                record.pointer("/address").and_then(Value::as_str).unwrap(),
                record.pointer("/size").and_then(Value::as_u64).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed_data,
        BTreeSet::from([
            ("load", DATA_ADDRESS, 4),
            ("store", "0x80000084", 4),
            ("store", "0x80000088", 4),
            ("store", "0x8000008c", 4),
        ])
    );
    let memory_trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("load-head FU run should expose Memory trace");
    let requests_before_response = memory_trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < response_tick)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        requests_before_response.len(),
        1,
        "only the load data request should precede its response"
    );
    let load_request = requests_before_response[0];
    let request_agent = load_request
        .pointer("/request_agent")
        .and_then(Value::as_u64)
        .expect("load request agent");
    let request_sequence = load_request
        .pointer("/request")
        .and_then(Value::as_u64)
        .expect("load request sequence");
    assert!(memory_trace.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            && record.pointer("/tick").and_then(Value::as_u64) == Some(response_tick)
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(request_sequence)
            && record.pointer("/response_status").and_then(Value::as_str) == Some("completed")
    }));
    assert!(data_trace.iter().any(|record| {
        record.pointer("/kind").and_then(Value::as_str) == Some("load")
            && record.pointer("/address").and_then(Value::as_str) == Some(DATA_ADDRESS)
            && record.pointer("/size").and_then(Value::as_u64) == Some(4)
            && record.pointer("/tick").and_then(Value::as_u64) == Some(response_tick)
    }));
}

fn assert_resident_scalar_load_fu_window(
    json: &Value,
    expected_pcs: &[&str],
    max_rob: u64,
    final_tick: u64,
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(final_tick)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000000000000000000")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident load-head FU run should expose ROB rows: {json}"));
    assert_eq!(rob.len(), expected_pcs.len());
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        expected_pcs
    );
    assert!(rob.iter().all(|entry| {
        entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
            && entry.pointer("/ready").and_then(Value::as_bool) == Some(false)
    }));
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries/0")
        .unwrap_or_else(|| panic!("resident load-head FU run should expose one LSQ row: {json}"));
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(lsq.pointer("/kind").and_then(Value::as_str), Some("load"));
    assert_eq!(
        lsq.pointer("/address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(lsq.pointer("/bytes").and_then(Value::as_u64), Some(4));
    assert_eq!(
        lsq.pointer("/completed").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(lsq.pointer("/sequence"), rob[0].pointer("/sequence"));
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        max_rob,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
}

fn scalar_load_fu_window_json(
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
        "O3,Data,Memory",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000080:16",
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

fn scalar_load_fu_window_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 13, 14, 0x0, 15, 0x33),
        s_type(4, 13, 10, 0b010),
        s_type(8, 14, 10, 0b010),
        s_type(12, 15, 10, 0b010),
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

fn scalar_load_dependent_window_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 12, 0x0, 14, 0x13),
        i_type(1, 14, 0x0, 15, 0x13),
        s_type(4, 13, 10, 0b010),
        s_type(8, 14, 10, 0b010),
        s_type(12, 15, 10, 0b010),
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
