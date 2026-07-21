use super::*;

const MUL_PC: &str = "0x8000000c";
const DEPENDENT_PC: &str = "0x80000010";
const SCALAR_LOAD_PC: &str = "0x80000014";
const SCALAR_DIV_PC: &str = "0x80000018";
const SCALAR_LOAD_DEPENDENT_PC: &str = "0x8000001c";
const WRONG_PATH_OUTER_BRANCH_PC: &str = "0x80000024";
const WRONG_PATH_BRANCH_PC: &str = "0x80000028";
const WRONG_PATH_DIV_PC: &str = "0x8000002c";
const WRONG_PATH_DEPENDENT_PC: &str = "0x80000030";
const WRONG_PATH_TARGET_PC: &str = "0x80000034";
const WRONG_PATH_PRE_SQUASH_TICK: u64 = 197;
const WRONG_PATH_POST_SQUASH_TICK: u64 = 230;
const DUMP_STATS_PC: &str = "0x80000028";
const WRITEBACK_PORT_STATS: [(&str, &str); 6] = [
    ("cycles", "Cycle"),
    ("admitted_rows", "Count"),
    ("deferred_rows", "Count"),
    ("deferred_row_cycles", "Cycle"),
    ("max_ready_rows_per_cycle", "Count"),
    ("max_deferred_rows", "Count"),
];

fn writeback_json(writeback_width: usize) -> Value {
    let path = writeback_binary();
    writeback_json_for_path(&path, writeback_width, 1, 600, &[])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WritebackRunConfig<'a> {
    memory_system: &'a str,
    writeback_width: usize,
    route_delay: u64,
    max_tick: u64,
    switch_mode: &'a str,
    stats_format: &'a str,
}

impl<'a> WritebackRunConfig<'a> {
    const fn detailed_json(
        memory_system: &'a str,
        writeback_width: usize,
        route_delay: u64,
        max_tick: u64,
    ) -> Self {
        Self {
            memory_system,
            writeback_width,
            route_delay,
            max_tick,
            switch_mode: "detailed",
            stats_format: "json",
        }
    }

    const fn direct_detailed_json(writeback_width: usize, route_delay: u64, max_tick: u64) -> Self {
        Self::detailed_json("direct", writeback_width, route_delay, max_tick)
    }

    const fn with_switch_mode(mut self, switch_mode: &'a str) -> Self {
        self.switch_mode = switch_mode;
        self
    }

    const fn with_stats_format(mut self, stats_format: &'a str) -> Self {
        self.stats_format = stats_format;
        self
    }
}

fn writeback_json_for_path(
    path: &std::path::Path,
    writeback_width: usize,
    route_delay: u64,
    max_tick: u64,
    extra_args: &[&str],
) -> Value {
    writeback_json_for_path_with_config(
        path,
        WritebackRunConfig::direct_detailed_json(writeback_width, route_delay, max_tick),
        extra_args,
    )
}

fn writeback_json_for_path_with_config(
    path: &std::path::Path,
    config: WritebackRunConfig<'_>,
    extra_args: &[&str],
) -> Value {
    let output = writeback_output_for_path_with_config(path, config, extra_args);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn writeback_output_for_path(
    path: &std::path::Path,
    writeback_width: usize,
    route_delay: u64,
    max_tick: u64,
    extra_args: &[&str],
) -> std::process::Output {
    writeback_output_for_path_with_config(
        path,
        WritebackRunConfig::direct_detailed_json(writeback_width, route_delay, max_tick),
        extra_args,
    )
}

fn writeback_output_for_path_with_config(
    path: &std::path::Path,
    config: WritebackRunConfig<'_>,
    extra_args: &[&str],
) -> std::process::Output {
    let mut command = writeback_command(path, config);
    command.args(extra_args);
    command.output().unwrap()
}

fn writeback_command(path: &std::path::Path, config: WritebackRunConfig<'_>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &config.max_tick.to_string(),
        "--execute",
        "--stats-format",
        config.stats_format,
    ]);
    if config.stats_format == "json" {
        command.args(["--debug-flags", "O3,Data,Fetch,Memory,HostAction"]);
    }
    command.args([
        "--riscv-o3-issue-width",
        "4",
        "--riscv-o3-writeback-width",
        &config.writeback_width.to_string(),
        "--memory-system",
        config.memory_system,
        "--memory-route-delay",
        &config.route_delay.to_string(),
        "--m5-switch-cpu-mode",
        config.switch_mode,
    ]);
    command
}

fn scalar_load_admission_json(
    path: &std::path::Path,
    writeback_width: usize,
    route_delay: u64,
    max_tick: u64,
) -> Value {
    scalar_load_admission_json_for_memory_system(
        path,
        "direct",
        writeback_width,
        route_delay,
        max_tick,
    )
}

fn scalar_load_admission_json_for_memory_system(
    path: &std::path::Path,
    memory_system: &str,
    writeback_width: usize,
    route_delay: u64,
    max_tick: u64,
) -> Value {
    writeback_json_for_path_with_config(
        path,
        WritebackRunConfig::detailed_json(memory_system, writeback_width, route_delay, max_tick),
        &[],
    )
}

fn writeback_port_artifact(json: &Value) -> &Value {
    json.pointer("/cores/0/o3_runtime/writeback_port")
        .unwrap_or_else(|| panic!("missing O3 writeback-port summary: {json}"))
}

fn writeback_port_u64(writeback: &Value, field: &str) -> u64 {
    writeback
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 writeback-port summary should expose {field}: {writeback}"))
}

fn expected_width_one_writeback_port_value(field: &str) -> u64 {
    match field {
        "cycles" | "admitted_rows" | "max_ready_rows_per_cycle" => 2,
        "deferred_rows" | "deferred_row_cycles" | "max_deferred_rows" => 1,
        _ => panic!("unexpected writeback-port field {field}"),
    }
}

fn assert_width_one_writeback_port_evidence(json: &Value) {
    let writeback = writeback_port_artifact(json);
    assert!(
        writeback_port_u64(writeback, "cycles") > 0,
        "width-one fixture should record writeback-port cycles: {writeback}"
    );
    for (field, expected) in [
        ("admitted_rows", 2),
        ("deferred_rows", 1),
        ("deferred_row_cycles", 1),
        ("max_ready_rows_per_cycle", 2),
        ("max_deferred_rows", 1),
    ] {
        assert_eq!(
            writeback_port_u64(writeback, field),
            expected,
            "unexpected width-one writeback-port {field}: {writeback}"
        );
    }
    for (field, unit) in WRITEBACK_PORT_STATS {
        assert_json_stat(
            json,
            &format!("sim.cpu0.o3.writeback_port.{field}"),
            unit,
            writeback_port_u64(writeback, field),
            "monotonic",
        );
    }
}

fn assert_scalar_load_writeback_collision(json: &Value) {
    let load = event_at_pc(json, SCALAR_LOAD_PC);
    let fu = event_at_pc(json, SCALAR_DIV_PC);
    let dependent = event_at_pc(json, SCALAR_LOAD_DEPENDENT_PC);
    let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
    let fu_raw_ready = event_u64(fu, "issue_tick") + 19;
    let admitted_tick = event_u64(load, "writeback_tick");
    assert_eq!(load_raw_ready, fu_raw_ready);
    assert_eq!(admitted_tick, load_raw_ready);
    assert_eq!(event_u64(fu, "writeback_tick"), fu_raw_ready + 1);
    assert!(event_u64(dependent, "issue_tick") >= admitted_tick);
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a")
    );
}

fn assert_memory_hierarchy_activity(json: &Value) {
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
            "hierarchy-backed writeback run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
    }
}

fn o3_trace_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("O3 trace should expose events: {json}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    o3_trace_events(json)
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_at_pc_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    o3_trace_events(json)
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn rob_entry_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 ROB should retain live row at {pc}: {json}"))
}

fn writeback_reservation_at_sequence(json: &Value, sequence: u64) -> &Value {
    writeback_reservation_at_sequence_if_present(json, sequence).unwrap_or_else(|| {
        panic!("O3 writeback calendar should retain sequence {sequence}: {json}")
    })
}

fn writeback_reservation_at_sequence_if_present(json: &Value, sequence: u64) -> Option<&Value> {
    json.pointer("/cores/0/o3_runtime/writeback_calendar/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
        })
}

fn checkpoint_runtime(checkpoint: &Value) -> &Value {
    checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .unwrap_or_else(|| panic!("missing decoded O3 runtime checkpoint: {checkpoint}"))
}

fn writeback_binary() -> std::path::PathBuf {
    let words = [
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(1, 2, 1, 0x0, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-port", &elf)
}

fn writeback_stats_dump_binary() -> std::path::PathBuf {
    let words = [
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(1, 2, 1, 0x0, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_DUMP_STATS),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-port-stats-dump", &elf)
}

fn writeback_timing_binary() -> std::path::PathBuf {
    let data_start = 64_i32;
    let mut words = vec![
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(1, 2, 1, 0x0, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        s_type(0, 3, 10, 0b010),
        s_type(4, 4, 10, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-port-timing", &elf)
}

fn writeback_checkpoint_binary() -> std::path::PathBuf {
    let mut words = vec![
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(0x01, 2, 1, 0b100, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
    ];
    words.extend(std::iter::repeat_n(i_type(0, 0, 0x0, 0, 0x13), 8));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-checkpoint", &elf)
}

fn wrong_path_writeback_binary() -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        i_type(42, 0, 0x0, 6, 0x13),
        i_type(84, 0, 0x0, 7, 0x13),
        i_type(2, 0, 0x0, 8, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(16, 1, 2, 0b000),
        b_type(12, 6, 6, 0b000),
        r_type(0x01, 8, 7, 0b100, 13, 0x33),
        i_type(1, 13, 0x0, 14, 0x13),
        i_type(-33, 12, 0x0, 15, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-wrong-path", &elf)
}

fn scalar_load_admission_binary() -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        r_type(1, 2, 1, 0b100, 3, 0x33),
        i_type(1, 12, 0x0, 14, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-scalar-load-admission", &elf)
}

#[path = "writeback_port/result_support.rs"]
mod result_support;

#[path = "writeback_port/result_classes.rs"]
mod result_classes;

#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;

#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;

#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;

#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;

#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
