use super::*;

const DIV_PC: &str = "0x8000000c";
const FIRST_PC: &str = "0x80000010";
const SECOND_PC: &str = "0x80000014";
const THIRD_PC: &str = "0x80000018";
const FAN_IN_RESULTS: &str = "0c000000050000001000000015000000";
const HEAD_DEPENDENT_RESULTS: &str = "0c000000050000000100000002000000";

#[derive(Clone, Copy)]
enum FuWindowProgram {
    FanIn,
    HeadDependent,
}

#[test]
fn rem6_run_o3_detailed_four_row_fu_window_direct() {
    let path = fu_live_window_binary("o3-four-row-fu-window-direct", FuWindowProgram::FanIn);
    let json = run_fu_live_window(&path, "direct", "detailed", 320);

    assert_completed_fan_in_window(&json);
    assert_direct_memory_boundary(&json);
}

#[test]
fn rem6_run_o3_detailed_four_row_fu_window_cache_fabric_dram() {
    let path = fu_live_window_binary(
        "o3-four-row-fu-window-cache-fabric-dram",
        FuWindowProgram::FanIn,
    );
    let json = run_fu_live_window(&path, "cache-fabric-dram", "detailed", 1000);

    assert_completed_fan_in_window(&json);
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
            "cache-backed FU-window run should expose {pointer}: {json}"
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
fn rem6_run_o3_detailed_four_row_fu_window_remains_resident_at_tick_limit() {
    let path = fu_live_window_binary("o3-four-row-fu-window-resident", FuWindowProgram::FanIn);
    let completed = run_fu_live_window(&path, "direct", "detailed", 320);
    let divide = event_at_pc(&completed, DIV_PC);
    let issue_tick = event_u64(divide, "issue_tick");
    let stop_tick = event_u64(divide, "writeback_tick").saturating_sub(1);
    assert!(issue_tick < stop_tick);

    let json = run_fu_live_window(&path, "direct", "detailed", stop_tick);

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
        Some("00000000000000000000000000000000")
    );
    assert_live_rob_rows(&json, &[DIV_PC, FIRST_PC, SECOND_PC, THIRD_PC]);
    assert_live_integer_rename_owners(&json, &[3, 4, 5, 6]);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_detailed_head_dependency_stops_fourth_fu_row_prefetch() {
    let path = fu_live_window_binary(
        "o3-head-dependent-fu-window",
        FuWindowProgram::HeadDependent,
    );
    let completed = run_fu_live_window(&path, "direct", "detailed", 320);

    assert_final_architecture(&completed, HEAD_DEPENDENT_RESULTS, [12, 5, 1, 2]);
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    let divide = event_at_pc(&completed, DIV_PC);
    let independent = event_at_pc(&completed, FIRST_PC);
    let dependent = event_at_pc(&completed, SECOND_PC);
    let downstream = event_at_pc(&completed, THIRD_PC);
    let divide_writeback = event_u64(divide, "writeback_tick");
    assert!(event_u64(independent, "issue_tick") < divide_writeback);
    assert!(event_u64(dependent, "issue_tick") >= divide_writeback);
    assert!(event_u64(downstream, "issue_tick") >= event_u64(dependent, "writeback_tick"));

    let stop_tick = divide_writeback.saturating_sub(1);
    let resident = run_fu_live_window(&path, "direct", "detailed", stop_tick);
    assert_eq!(
        resident
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        resident
            .pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    assert_eq!(
        resident.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("00000000000000000000000000000000")
    );
    assert_live_rob_rows(&resident, &[DIV_PC, FIRST_PC, SECOND_PC]);
    assert_live_integer_rename_owners(&resident, &[3, 4, 5]);
    assert!(resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .is_some_and(|entries| entries
            .iter()
            .all(|entry| entry.pointer("/pc").and_then(Value::as_str) != Some(THIRD_PC))));
}

#[test]
fn rem6_run_o3_timing_fu_window_preserves_architecture_without_live_rows() {
    let path = fu_live_window_binary("o3-four-row-fu-window-timing", FuWindowProgram::FanIn);
    let json = run_fu_live_window(&path, "direct", "timing", 320);

    assert_final_architecture(&json, FAN_IN_RESULTS, [12, 5, 16, 21]);
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
        "timing mode should suppress four-row FU-window aliases: {unexpected:?}"
    );
    for path in [
        "sim.debug.o3_trace.records",
        "sim.debug.o3_trace.instructions",
        "sim.debug.o3_trace.max_rob_occupancy",
        "sim.debug.o3_trace.execution_mode.timing",
        "sim.debug.o3_trace.execution_mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.mode.timing",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
}

fn assert_completed_fan_in_window(json: &Value) {
    assert_final_architecture(json, FAN_IN_RESULTS, [12, 5, 16, 21]);
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(json, "system.cpu.rob.maxOccupancy", "Count", 4, "monotonic");
    for (path, value, unit) in [
        ("sim.cpu0.o3.live_retire_gate.scheduled_waits", 1, "Count"),
        ("sim.cpu0.o3.live_retire_gate.wait_ticks", 19, "Cycle"),
        ("sim.cpu0.o3.live_retire_gate.max_wait_ticks", 19, "Cycle"),
    ] {
        assert_json_stat(json, path, unit, value, "monotonic");
    }

    let events = [
        event_at_pc(json, DIV_PC),
        event_at_pc(json, FIRST_PC),
        event_at_pc(json, SECOND_PC),
        event_at_pc(json, THIRD_PC),
    ];
    assert_eq!(
        events[0]
            .pointer("/fu_latency_class")
            .and_then(Value::as_str),
        Some("scalar_integer_div")
    );
    assert_eq!(event_u64(events[0], "fu_latency_cycles"), 19);
    let divide_issue = event_u64(events[0], "issue_tick");
    let divide_writeback = event_u64(events[0], "writeback_tick");
    assert_eq!(divide_writeback, divide_issue + 19);
    assert_eq!(event_u64(events[0], "commit_tick"), divide_writeback);

    for event in &events[1..] {
        assert_eq!(event_u64(event, "fu_latency_cycles"), 0);
        assert_eq!(
            event_u64(event, "writeback_tick"),
            event_u64(event, "issue_tick")
        );
        assert!(event_u64(event, "issue_tick") < divide_writeback);
    }
    assert!(event_u64(events[1], "issue_tick") < event_u64(events[2], "issue_tick"));
    assert!(event_u64(events[2], "issue_tick") < event_u64(events[3], "issue_tick"));
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
    for (event, occupancy) in events.iter().zip([4, 3, 2, 1]) {
        assert_eq!(event_u64(event, "rob_occupancy"), occupancy);
        assert_eq!(event_u64(event, "rob_commits_at_tick"), 1);
        assert_eq!(
            event
                .pointer("/rob_commit_blocked")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
    assert!(event_u64(events[3], "iew_dependency_producers") >= 1);
}

fn assert_final_architecture(json: &Value, expected_memory: &str, expected_registers: [u64; 4]) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory)
    );
    for (register, expected) in [3, 4, 5, 6].into_iter().zip(expected_registers) {
        let expected = format!("{expected:#x}");
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/x{register}"))
                .and_then(Value::as_str),
            Some(expected.as_str())
        );
    }
}

fn assert_live_rob_rows(json: &Value, expected_pcs: &[&str]) {
    let entries = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident FU-window run should expose ROB rows: {json}"));
    assert_eq!(entries.len(), expected_pcs.len());
    for (entry, pc) in entries.iter().zip(expected_pcs) {
        assert_eq!(entry.pointer("/pc").and_then(Value::as_str), Some(*pc));
        assert_eq!(
            entry.pointer("/ready").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            entry.pointer("/live_staged").and_then(Value::as_bool),
            Some(true)
        );
        assert!(entry
            .pointer("/destination")
            .and_then(Value::as_u64)
            .is_some());
    }
}

fn assert_live_integer_rename_owners(json: &Value, architectural: &[u64]) {
    let entries = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident FU-window run should expose rename rows: {json}"));
    for register in architectural {
        assert!(
            entries.iter().any(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64) == Some(*register)
            }),
            "live rename map should include x{register}: {entries:?}"
        );
    }
}

fn assert_direct_memory_boundary(json: &Value) {
    assert!(
        json.pointer("/memory_resources/transport/data/activity")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0),
        "direct FU-window run should retain transport activity: {json}"
    );
    assert_json_stat_at_least(
        json,
        "sim.memory.resources.transport.data.activity",
        "Count",
        1,
        "monotonic",
    );
    for (pointer, path) in [
        (
            "/memory_resources/cache/data/activity",
            "sim.memory.resources.cache.data.activity",
        ),
        (
            "/memory_resources/fabric/activity",
            "sim.memory.resources.fabric.activity",
        ),
        (
            "/memory_resources/dram/activity",
            "sim.memory.resources.dram.activity",
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0),
            0
        );
        assert_json_stat(json, path, "Count", 0, "monotonic");
    }
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}

fn run_fu_live_window(path: &Path, memory_system: &str, switch_mode: &str, max_tick: u64) -> Value {
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
            "--debug-flags",
            "O3",
            "--memory-system",
            memory_system,
            "--m5-switch-cpu-mode",
            switch_mode,
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
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn fu_live_window_binary(name: &str, program: FuWindowProgram) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        r_type(1, 2, 1, 0x4, 3, 0x33),
        i_type(5, 0, 0x0, 4, 0x13),
    ];
    match program {
        FuWindowProgram::FanIn => {
            words.extend([i_type(11, 4, 0x0, 5, 0x13), r_type(0, 5, 4, 0x0, 6, 0x33)])
        }
        FuWindowProgram::HeadDependent => {
            words.extend([i_type(-11, 3, 0x0, 5, 0x13), i_type(1, 5, 0x0, 6, 0x13)])
        }
    }
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13),
        s_type(0, 3, 12, 0b010),
        s_type(4, 4, 12, 0b010),
        s_type(8, 5, 12, 0b010),
        s_type(12, 6, 12, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
