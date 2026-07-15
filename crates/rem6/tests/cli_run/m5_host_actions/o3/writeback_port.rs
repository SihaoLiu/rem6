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

#[test]
fn rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision() {
    let json = writeback_json(1);
    let multiply = event_at_pc(&json, MUL_PC);
    let dependent = event_at_pc(&json, DEPENDENT_PC);

    assert_eq!(
        event_u64(dependent, "issue_tick"),
        event_u64(multiply, "writeback_tick"),
        "dependent ADDI must issue from the admitted MUL writeback tick: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        event_u64(dependent, "writeback_tick"),
        event_u64(multiply, "writeback_tick") + 1,
        "width-one writeback must serialize the dependent zero-latency ADDI completion: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x3")
            .and_then(Value::as_str),
        Some("0x2a")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x4")
            .and_then(Value::as_str),
        Some("0x2b")
    );
    assert_width_one_writeback_port_evidence(&json);
}

#[test]
fn rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision() {
    let json = writeback_json(2);
    let multiply = event_at_pc(&json, MUL_PC);
    let dependent = event_at_pc(&json, DEPENDENT_PC);

    assert_eq!(
        event_u64(dependent, "issue_tick"),
        event_u64(multiply, "writeback_tick"),
        "dependent ADDI must issue from the admitted MUL writeback tick: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        event_u64(dependent, "writeback_tick"),
        event_u64(multiply, "writeback_tick"),
        "width-two writeback should admit the MUL and dependent ADDI in the same cycle: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x3")
            .and_then(Value::as_str),
        Some("0x2a")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x4")
            .and_then(Value::as_str),
        Some("0x2b")
    );
}

#[test]
fn rem6_run_o3_writeback_port_json_exposes_counters() {
    let json = writeback_json(1);

    assert_width_one_writeback_port_evidence(&json);
}

#[test]
fn rem6_run_o3_writeback_port_text_stats_expose_counters() {
    let path = writeback_binary();
    let config = WritebackRunConfig::direct_detailed_json(1, 1, 600).with_stats_format("text");
    let output = writeback_command(&path, config).output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    for (field, unit) in WRITEBACK_PORT_STATS {
        let path = format!("sim.cpu0.o3.writeback_port.{field}");
        let value = expected_width_one_writeback_port_value(field);
        match unit {
            "Cycle" => assert_text_cycle_stat(&stdout, &path, value),
            "Count" => assert_text_count_stat(&stdout, &path, value),
            _ => panic!("unexpected writeback-port stat unit {unit} for {path}"),
        }
        assert_text_stat_occurs_once(&stdout, &path);
    }
}

#[test]
fn rem6_run_o3_writeback_port_stats_dump_exposes_counters() {
    let path = writeback_stats_dump_binary();
    let json = writeback_json_for_path(&path, 1, 1, 600, &[]);

    let dump_event = event_at_pc(&json, DUMP_STATS_PC);
    assert_eq!(
        dump_event.pointer("/system_event").and_then(Value::as_bool),
        Some(true),
        "fixture should execute a real m5_dump_stats row: {dump_event}"
    );
    let dump_issue_tick = event_u64(dump_event, "issue_tick");
    for pc in [MUL_PC, DEPENDENT_PC] {
        let event = event_at_pc(&json, pc);
        assert!(
            event_u64(event, "writeback_tick") < dump_issue_tick,
            "colliding row {pc} must write back before m5_dump_stats: row={event}, dump={dump_event}"
        );
    }

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "writeback-port dump fixture should deliver one m5_dump_stats action: {host_actions}"
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing writeback-port stats dump action: {host_actions}"));
    assert!(
        dump.pointer("/tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick >= dump_issue_tick),
        "stats dump action must not precede its O3 issue boundary: dump={dump}, event={dump_event}"
    );
    for (field, unit) in WRITEBACK_PORT_STATS {
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.writeback_port.{field}"),
            "counter",
            unit,
            expected_width_one_writeback_port_value(field),
            "resettable",
        );
    }
    assert_width_one_writeback_port_evidence(&json);
    for (field, _) in WRITEBACK_PORT_STATS {
        let dump_path = format!("sim.host_actions.stats_dump.cpu0.o3.writeback_port.{field}");
        let dump_value = dump
            .pointer("/samples")
            .and_then(Value::as_array)
            .and_then(|samples| {
                samples.iter().find(|sample| {
                    sample.pointer("/path").and_then(Value::as_str) == Some(dump_path.as_str())
                })
            })
            .and_then(|sample| sample.pointer("/value").and_then(Value::as_u64));
        let native_path = format!("sim.cpu0.o3.writeback_port.{field}");
        assert_eq!(
            dump_value,
            Some(json_stat_u64(&json, &native_path)),
            "stats-dump {field} must match the final native writeback-port value"
        );
    }
}

#[test]
fn rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission() {
    let path = scalar_load_admission_binary();
    let mut collision_runs: Vec<_> = [4, 8, 9, 12, 16, 20]
        .into_iter()
        .filter_map(|route_delay| {
            let json = scalar_load_admission_json(&path, 2, route_delay, 600);
            let load = event_at_pc(&json, SCALAR_LOAD_PC);
            let fu = event_at_pc(&json, SCALAR_DIV_PC);
            (event_u64(load, "lsq_data_response_tick") + 1 == event_u64(fu, "issue_tick") + 19)
                .then_some((route_delay, json))
        })
        .collect();
    assert_eq!(
        collision_runs.len(),
        1,
        "route-delay calibration must find exactly one scalar-load/DIV raw-ready collision"
    );
    let (route_delay, calibration) = collision_runs.pop().unwrap();
    assert_eq!(route_delay, 9);
    let calibrated_load = event_at_pc(&calibration, SCALAR_LOAD_PC);
    let calibrated_fu = event_at_pc(&calibration, SCALAR_DIV_PC);
    assert_eq!(
        event_u64(calibrated_load, "lsq_data_response_tick") + 1,
        event_u64(calibrated_fu, "issue_tick") + 19,
        "width-two calibration must align the scalar-load and DIV raw-ready ticks"
    );

    let full = scalar_load_admission_json(&path, 1, route_delay, 600);
    let load = event_at_pc(&full, SCALAR_LOAD_PC);
    let fu = event_at_pc(&full, SCALAR_DIV_PC);
    let dependent = event_at_pc(&full, SCALAR_LOAD_DEPENDENT_PC);
    let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
    let fu_raw_ready = event_u64(fu, "issue_tick") + 19;
    let admitted_tick = event_u64(load, "writeback_tick");
    assert_eq!(load_raw_ready, fu_raw_ready);
    assert_eq!(admitted_tick, load_raw_ready + 1);
    assert_eq!(event_u64(fu, "writeback_tick"), fu_raw_ready);
    assert!(event_u64(dependent, "issue_tick") >= admitted_tick);
    assert_eq!(
        full.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a")
    );

    let before = scalar_load_admission_json(&path, 1, route_delay, admitted_tick - 1);
    assert_eq!(
        before
            .pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        None,
        "the zero-valued architectural load destination must remain absent before admission"
    );
    let load_row = rob_entry_at_pc(&before, SCALAR_LOAD_PC);
    assert_eq!(
        load_row.pointer("/ready").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        load_row.pointer("/live_staged").and_then(Value::as_bool),
        Some(true)
    );

    let at_admission = scalar_load_admission_json(&path, 1, route_delay, admitted_tick);
    assert_eq!(
        at_admission
            .pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a"),
        "the architectural load value must appear exactly at admission"
    );
}

#[test]
fn rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram() {
    let path = scalar_load_admission_binary();
    let mut collision_runs = Vec::new();
    let mut observations = Vec::new();
    for route_delay in [1, 2, 4, 8, 9, 12, 16, 20] {
        let json = scalar_load_admission_json_for_memory_system(
            &path,
            "cache-fabric-dram",
            2,
            route_delay,
            1_500,
        );
        let load = event_at_pc(&json, SCALAR_LOAD_PC);
        let fu = event_at_pc(&json, SCALAR_DIV_PC);
        let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
        let fu_raw_ready = event_u64(fu, "issue_tick") + 19;
        observations.push((route_delay, load_raw_ready, fu_raw_ready));
        if load_raw_ready == fu_raw_ready {
            collision_runs.push((route_delay, json));
        }
    }
    assert_eq!(
        collision_runs.len(),
        1,
        "route-delay calibration must find exactly one hierarchy-backed scalar-load/DIV raw-ready collision: {observations:?}"
    );
    let (route_delay, _) = collision_runs.pop().unwrap();
    let json = scalar_load_admission_json_for_memory_system(
        &path,
        "cache-fabric-dram",
        1,
        route_delay,
        1_500,
    );

    assert_scalar_load_writeback_collision(&json);
    assert_memory_hierarchy_activity(&json);
}

#[test]
fn rem6_run_timing_suppresses_o3_writeback_port_surface() {
    let path = writeback_timing_binary();
    let dump_args = ["--dump-memory", "0x80000040:8"];
    let detailed_config = WritebackRunConfig::direct_detailed_json(1, 1, 600);
    let detailed = writeback_json_for_path_with_config(&path, detailed_config, &dump_args);
    let timing = writeback_json_for_path_with_config(
        &path,
        detailed_config.with_switch_mode("timing"),
        &dump_args,
    );

    assert!(
        detailed
            .pointer("/cores/0/o3_runtime/writeback_port")
            .is_some(),
        "detailed mode should expose the writeback-port surface: {detailed}"
    );
    for (register, value) in [("x3", "0x2a"), ("x4", "0x2b")] {
        let pointer = format!("/cores/0/registers/{register}");
        assert_eq!(
            detailed.pointer(&pointer).and_then(Value::as_str),
            Some(value)
        );
        assert_eq!(
            timing.pointer(&pointer),
            detailed.pointer(&pointer),
            "timing mode must preserve final register {register}"
        );
    }
    assert_eq!(
        detailed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a0000002b000000"),
        "detailed writeback fixture should store both collided results"
    );
    assert_eq!(
        timing.pointer("/memory/0/hex"),
        detailed.pointer("/memory/0/hex"),
        "timing mode must preserve final memory"
    );
    assert!(
        timing
            .pointer("/cores/0/o3_runtime/writeback_port")
            .is_none(),
        "timing mode should not expose detailed O3 writeback-port JSON: {timing}"
    );
    for (field, _) in WRITEBACK_PORT_STATS {
        assert_json_stat_absent(&timing, &format!("sim.cpu0.o3.writeback_port.{field}"));
    }
}

#[test]
fn rem6_run_o3_writeback_wrong_path_reservation_never_publishes() {
    let path = wrong_path_writeback_binary();
    let branch_args = [
        "--riscv-branch-lookahead",
        "2",
        "--riscv-o3-scalar-memory-depth",
        "4",
    ];
    let before = writeback_json_for_path(&path, 1, 8, WRONG_PATH_PRE_SQUASH_TICK, &branch_args);
    let wrong_path_div = rob_entry_at_pc(&before, WRONG_PATH_DIV_PC);
    let wrong_path_sequence = wrong_path_div
        .pointer("/sequence")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("wrong-path DIV must expose its sequence: {wrong_path_div}"));
    let wrong_path_reservation = writeback_reservation_at_sequence(&before, wrong_path_sequence);
    let wrong_path_raw_ready_tick = wrong_path_reservation
        .pointer("/raw_ready_tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!("wrong-path DIV reservation must expose its raw-ready tick: {wrong_path_reservation}")
        });
    let wrong_path_admitted_tick = wrong_path_reservation
        .pointer("/admitted_tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!("wrong-path DIV reservation must expose its admitted tick: {wrong_path_reservation}")
        });
    assert_eq!(wrong_path_admitted_tick, wrong_path_raw_ready_tick);
    assert!(
        wrong_path_admitted_tick > WRONG_PATH_PRE_SQUASH_TICK,
        "the rollback must discard a genuinely future reservation: {wrong_path_reservation}"
    );
    assert_eq!(
        before
            .pointer("/cores/0/o3_runtime/issue/issued_rows")
            .and_then(Value::as_u64),
        Some(3),
        "both branches and the wrong-path DIV must issue before retirement: {before}"
    );
    assert_eq!(
        before
            .pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        None,
        "the issued wrong-path DIV must not publish before its admitted tick"
    );

    let after = writeback_json_for_path(&path, 1, 8, WRONG_PATH_POST_SQUASH_TICK, &branch_args);

    let outer_branch = event_at_pc(&after, WRONG_PATH_OUTER_BRANCH_PC);
    assert_eq!(
        outer_branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
    let branch = event_at_pc(&after, WRONG_PATH_BRANCH_PC);
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.pointer("/branch_squash").and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&after, WRONG_PATH_DIV_PC).is_none());
    assert!(event_at_pc_if_present(&after, WRONG_PATH_DEPENDENT_PC).is_none());
    assert!(event_at_pc_if_present(&after, WRONG_PATH_TARGET_PC).is_none());
    assert!(writeback_reservation_at_sequence_if_present(&after, wrong_path_sequence).is_none());
    assert_eq!(
        after
            .pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        None
    );
    assert_eq!(
        after
            .pointer("/cores/0/registers/x14")
            .and_then(Value::as_str),
        None
    );

    let json = writeback_json_for_path(&path, 1, 8, 800, &branch_args);
    assert_eq!(
        json.pointer("/cores/0/registers/x15")
            .and_then(Value::as_str),
        Some("0x9")
    );
    let target = event_at_pc(&json, WRONG_PATH_TARGET_PC);
    assert_eq!(
        event_u64(target, "issue_tick"),
        event_u64(target, "writeback_tick"),
        "the correct-path row must publish without writeback deferral"
    );
}

#[test]
fn rem6_run_o3_writeback_port_checkpoint_boundary() {
    let path = writeback_checkpoint_binary();
    let baseline = writeback_json_for_path(&path, 1, 1, 600, &[]);
    let multiply = event_at_pc(&baseline, MUL_PC);
    let dependent = event_at_pc(&baseline, DEPENDENT_PC);
    let live_tick = event_u64(multiply, "issue_tick") + 1;
    let live_arg = format!("{live_tick}:writeback-live");
    let live = writeback_output_for_path(&path, 1, 1, 600, &["--host-checkpoint", &live_arg]);
    if live.status.success() {
        let live_json: Value = serde_json::from_slice(&live.stdout).unwrap();
        panic!(
            "expected live checkpoint failure: multiply={multiply}, dependent={dependent}, checkpoint={}",
            live_json
                .pointer("/host_actions/checkpoints/0")
                .unwrap_or(&Value::Null)
        );
    }
    assert!(live.stdout.is_empty());
    let live_stderr = String::from_utf8_lossy(&live.stderr);
    assert!(
        live_stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live writeback checkpoint should fail closed: {live_stderr}"
    );

    let checkpoint_tick = event_u64(dependent, "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:writeback-drained");
    let restore_arg = format!("{restore_tick}:writeback-drained");
    let restored = writeback_json_for_path(
        &path,
        1,
        1,
        600,
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained writeback checkpoint");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("restored writeback checkpoint");
    let captured_runtime = checkpoint_runtime(checkpoint);
    let restored_runtime = checkpoint_runtime(restore);
    assert_eq!(
        captured_runtime
            .pointer("/checkpoint_version")
            .and_then(Value::as_u64),
        Some(23)
    );
    assert_eq!(
        captured_runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        captured_runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    for (field, expected) in [
        ("stats_writeback_port_cycles", 2),
        ("stats_writeback_port_admitted_rows", 2),
        ("stats_writeback_port_deferred_rows", 1),
        ("stats_writeback_port_deferred_row_cycles", 1),
        ("stats_writeback_port_max_ready_rows_per_cycle", 2),
        ("stats_writeback_port_max_deferred_rows", 1),
    ] {
        assert_eq!(
            captured_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "captured writeback checkpoint should preserve {field}: {captured_runtime}"
        );
        assert_eq!(
            restored_runtime.pointer(&format!("/{field}")),
            captured_runtime.pointer(&format!("/{field}")),
            "restored writeback checkpoint should preserve {field}"
        );
    }
}

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
    assert_eq!(admitted_tick, load_raw_ready + 1);
    assert_eq!(event_u64(fu, "writeback_tick"), fu_raw_ready);
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
