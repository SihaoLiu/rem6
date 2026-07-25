use super::*;

const COLD_LOAD_PC: &str = "0x80000008";
const CACHED_LOAD_PC: &str = "0x8000000c";
const COLD_WINDOW_PCS: [&str; 4] = [COLD_LOAD_PC, "0x8000000c", "0x80000010", "0x80000014"];
const CACHED_WINDOW_PCS: [&str; 4] = [CACHED_LOAD_PC, "0x80000010", "0x80000014", "0x80000018"];
const CHECKPOINT_MUTATION_PC: &str = "0x80000034";
const VIRTUAL_PAGE: u64 = 0x4000;
const PHYSICAL_PAGE: u64 = 0x8000_0000;
const DATA_OFFSET: u64 = 0x80;
const MAX_TICK: u64 = 1_200;
const TRANSLATED_RESULTS: &str = "2a00000005000000100000003a000000";

#[test]
fn rem6_run_host_switch_rejects_cold_translated_scalar_load_younger_window_direct() {
    assert_cold_translated_scalar_load_younger_window_switch_rejects("direct");
}

#[test]
fn rem6_run_host_switch_rejects_cold_translated_scalar_load_younger_window_cache_fabric_dram() {
    assert_cold_translated_scalar_load_younger_window_switch_rejects("cache-fabric-dram");
}

fn assert_cold_translated_scalar_load_younger_window_switch_rejects(memory_system: &str) {
    let path = cold_translated_scalar_load_binary(&format!(
        "host-switch-live-o3-cold-translated-scalar-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_translated_scalar_load_in_memory_system(&path, memory_system, None, "cold");
    let baseline_load = event_at_pc(&baseline, COLD_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    assert_stopped_with_headroom(&baseline, response_tick);
    let switch_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < switch_tick && switch_tick < response_tick);

    assert_translated_scalar_load_switch_rejects(
        &path,
        memory_system,
        switch_tick,
        &format!("cold-{}", memory_system.replace('-', "_")),
    );
}

#[test]
fn rem6_run_rejects_live_cold_translated_scalar_load_younger_window_checkpoint() {
    let path = cold_translated_scalar_load_binary("cold-translated-younger-window-live-checkpoint");
    let baseline = run_translated_scalar_load(&path, None, "cold-live-checkpoint-baseline");
    for pc in COLD_WINDOW_PCS {
        event_at_pc(&baseline, pc);
    }
    let load = event_at_pc(&baseline, COLD_LOAD_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let checkpoint_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < checkpoint_tick && checkpoint_tick < response_tick);

    let output = translated_scalar_load_output(
        &path,
        "direct",
        None,
        "cold-live-checkpoint",
        "detailed",
        &format!("host_checkpoints = [\"{checkpoint_tick}:cold-translated-live\"]\n"),
    );

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "live translated checkpoint must not emit partial success JSON: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live translated checkpoint should report cpu0 quiescence rejection: {stderr}"
    );
}

#[test]
fn rem6_run_restores_drained_cold_translated_scalar_load_younger_window_and_stats() {
    let path = cold_translated_scalar_load_checkpoint_binary(
        "cold-translated-younger-window-drained-restore",
    );
    let first = run_translated_scalar_load_with_options(
        &path,
        "direct",
        None,
        "cold-drained-checkpoint-baseline",
        "detailed",
        "",
    );
    assert_cold_translated_architecture(&first);
    let first_host_actions = first
        .pointer("/host_actions")
        .unwrap_or_else(|| panic!("missing first translated host actions: {first}"));
    assert_eq!(
        first_host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1),
        "first translated checkpoint actions: {first_host_actions}"
    );
    assert_eq!(
        first_host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "first translated stats actions: {first_host_actions}"
    );
    let first_dump_tick = first_host_actions
        .pointer("/stats_dumps/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first translated dump tick: {first_host_actions}"));
    let mutation_commit_tick =
        event_u64(event_at_pc(&first, CHECKPOINT_MUTATION_PC), "commit_tick");
    assert!(first_dump_tick < mutation_commit_tick);
    let restore_tick = mutation_commit_tick + 1;

    let restored = run_translated_scalar_load_with_options(
        &path,
        "direct",
        None,
        "cold-drained-restore",
        "detailed",
        &format!("host_checkpoint_restores = [\"{restore_tick}:gem5-m5-checkpoint\"]\n"),
    );
    assert_cold_translated_architecture(&restored);
    assert_eq!(
        restored.pointer("/cores/0/registers/x16").and_then(Value::as_str),
        Some("0x1"),
        "checkpoint restore must replay one post-checkpoint increment from restored x16=0: {restored}"
    );
    let host_actions = restored
        .pointer("/host_actions")
        .unwrap_or_else(|| panic!("missing restored translated host actions: {restored}"));
    for (pointer, expected) in [
        ("/checkpoint_count", 1),
        ("/checkpoint_restored_count", 1),
        ("/stats_dump_count", 2),
    ] {
        assert_eq!(
            host_actions.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated lifecycle count {pointer}: {host_actions}"
        );
    }

    let checkpoint = host_actions
        .pointer("/checkpoints/0")
        .unwrap_or_else(|| panic!("missing translated checkpoint: {host_actions}"));
    let restore = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing translated restore: {host_actions}"));
    let checkpoint_runtime = checkpoint_component_chunk(checkpoint, "cpu0", "o3-runtime-state")
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing captured translated O3 runtime: {checkpoint}"));
    let restored_runtime = checkpoint_component_chunk(restore, "cpu0", "o3-runtime-state")
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("missing restored translated O3 runtime: {restore}"));
    assert_eq!(
        restored_runtime, checkpoint_runtime,
        "drained translated O3 payload must survive restore exactly"
    );
    for (pointer, expected) in [
        ("/snapshot_rob_entries", 0),
        ("/snapshot_lsq_entries", 0),
        ("/stats_max_rob_occupancy", 4),
        ("/stats_max_lsq_occupancy", 1),
        ("/stats_lsq_operation_load", 1),
        ("/stats_lsq_operation_store", 3),
    ] {
        assert_eq!(
            checkpoint_runtime.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated checkpoint runtime field {pointer}: {checkpoint_runtime}"
        );
    }

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-restore translated stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored translated stats dump: {host_actions}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored translated dump tick: {restored_dump}"));
    assert!(first_dump_tick < restore_tick && restore_tick < restored_dump_tick);
    for (path, expected) in [
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load", 1),
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store", 3),
        ("system.cpu.lsq0.operation.load", 1),
        ("system.cpu.lsq0.operation.store", 3),
        ("system.cpu.lsq0.operation.total", 4),
    ] {
        assert_stats_dump_sample(first_dump, path, "counter", "Count", expected, "resettable");
        assert_eq!(
            stats_dump_sample_value(restored_dump, path),
            stats_dump_sample_value(first_dump, path),
            "restored translated stats dump must preserve {path}"
        );
    }
}

#[test]
fn rem6_run_timing_suppresses_cold_translated_scalar_load_younger_window_o3_artifacts() {
    let path = cold_translated_scalar_load_binary("cold-translated-younger-window-timing");
    let json =
        run_translated_scalar_load_with_options(&path, "direct", None, "cold-timing", "timing", "");

    assert_cold_translated_architecture(&json);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("translated timing stats array")
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
        "timing mode should suppress translated O3 aliases: {unexpected:?}"
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

#[test]
fn rem6_run_host_switch_rejects_cached_translated_scalar_load_younger_window_direct() {
    assert_cached_translated_scalar_load_younger_window_switch_rejects("direct");
}

#[test]
fn rem6_run_host_switch_rejects_cached_translated_scalar_load_younger_window_cache_fabric_dram() {
    assert_cached_translated_scalar_load_younger_window_switch_rejects("cache-fabric-dram");
}

fn assert_cached_translated_scalar_load_younger_window_switch_rejects(memory_system: &str) {
    let path = cached_translated_scalar_load_binary(&format!(
        "host-switch-live-o3-cached-translated-scalar-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline =
        run_translated_scalar_load_in_memory_system(&path, memory_system, None, "cached");
    let baseline_load = event_at_pc(&baseline, CACHED_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    assert_stopped_with_headroom(&baseline, response_tick);
    let switch_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < switch_tick && switch_tick < response_tick);

    assert_translated_scalar_load_switch_rejects(
        &path,
        memory_system,
        switch_tick,
        &format!("cached-{}", memory_system.replace('-', "_")),
    );
}

fn assert_translated_scalar_load_switch_rejects(
    path: &Path,
    memory_system: &str,
    switch_tick: u64,
    scenario: &str,
) {
    let artifact = temp_output(&format!("translated-scalar-load-{scenario}-live-switch"));
    let extra_run = format!("output = \"{}\"\n", artifact.display());
    let output = translated_scalar_load_output(
        path,
        memory_system,
        Some(switch_tick),
        scenario,
        "detailed",
        &extra_run,
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "translated {scenario} live switch: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "translated {scenario} live switch: {output:?}"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
    );
    assert!(
        !artifact.exists(),
        "translated {scenario} live switch emitted {}",
        artifact.display()
    );
}

#[test]
fn rem6_run_unused_readfile_preserves_cached_translated_memory_younger_window() {
    let path = cached_translated_scalar_load_binary(
        "unused-readfile-cached-translated-memory-younger-window",
    );
    let readfile = temp_binary("unused-readfile-cached-translated-memory-data", &[0; 8]);
    let baseline = run_translated_scalar_load(&path, None, "cached-unused-readfile-baseline");
    let workspace = temp_workspace("cached-translated-memory-unused-readfile");
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        translated_config(
            &path,
            None,
            "",
            &format!(
                "readfiles = [\"0x10000000:0x100:{}\"]\n",
                readfile.display()
            ),
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "unused readfile translated run; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));

    assert_stopped_with_headroom(
        &json,
        event_u64(
            event_at_pc(&baseline, CACHED_LOAD_PC),
            "lsq_data_response_tick",
        ),
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(TRANSLATED_RESULTS)
    );
    for pc in CACHED_WINDOW_PCS {
        let baseline_event = event_at_pc(&baseline, pc);
        let event = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(event, field),
                event_u64(baseline_event, field),
                "unused readfile changed translated-memory {field} for {pc}: {event}"
            );
        }
    }
    assert_eq!(
        event_u64(event_at_pc(&json, CACHED_LOAD_PC), "lsq_data_response_tick"),
        event_u64(
            event_at_pc(&baseline, CACHED_LOAD_PC),
            "lsq_data_response_tick"
        )
    );
    assert!(json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records.iter().all(|record| {
            record.pointer("/address").and_then(Value::as_str) != Some("0x10000000")
        })));
}

fn assert_stopped_with_headroom(json: &Value, response_tick: u64) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    let final_tick = json
        .pointer("/simulation/final_tick")
        .and_then(Value::as_u64)
        .expect("translated run final tick");
    assert!(final_tick < MAX_TICK, "run exhausted tick headroom: {json}");
    assert!(
        final_tick.saturating_sub(response_tick) <= 400,
        "post-response completion exceeded the fixture budget: {json}"
    );
}

fn assert_cold_translated_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(TRANSLATED_RESULTS)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x3a"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "cold translated run must preserve {register}: {json}"
        );
    }
}

#[test]
fn rem6_run_rejects_overlapping_riscv_data_translation_mappings() {
    let path = cold_translated_scalar_load_binary("overlapping-riscv-data-translation");
    let workspace = temp_workspace("overlapping-riscv-data-translation-config");
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        translated_config(
            &path,
            None,
            "\n[[run.riscv_data_translation.mappings]]\nvirtual_base = 16384\nphysical_base = 2147487744\npages = 1\nread = true\nwrite = true\n",
            "",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("overlaps existing mapping"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_rejects_riscv_data_translation_with_instruction_limit() {
    let path = cold_translated_scalar_load_binary("instruction-limit-riscv-data-translation");
    let workspace = temp_workspace("instruction-limit-riscv-data-translation-config");
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        translated_config(&path, None, "", "max_instructions = 1\n"),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("RISC-V data translation does not yet support max_instructions"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_rejects_unsupported_riscv_data_translation_combinations() {
    let path = cold_translated_scalar_load_binary("unsupported-riscv-data-translation");
    let base = translated_config(&path, None, "", "");
    let cases = [
        (
            "riscv-se",
            translated_config(&path, None, "", "riscv_se = true\n"),
            &[][..],
            "RISC-V data translation does not yet support RISC-V SE",
        ),
        (
            "gdb",
            base.clone(),
            &["--gdb-listen", "127.0.0.1:1"][..],
            "RISC-V data translation does not yet support GDB run control",
        ),
    ];

    for (name, config, extra_args, expected) in cases {
        let workspace = temp_workspace(&format!("unsupported-riscv-data-translation-{name}"));
        let config_path = workspace.join("run.toml");
        std::fs::write(&config_path, config).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command
            .args(["run", "--config", config_path.to_str().unwrap()])
            .args(extra_args);
        let output = command.output().unwrap();
        assert!(!output.status.success(), "case {name} unexpectedly passed");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "case {name}; stderr: {stderr}");
    }
}

fn run_translated_scalar_load(path: &Path, switch_tick: Option<u64>, scenario: &str) -> Value {
    run_translated_scalar_load_in_memory_system(path, "direct", switch_tick, scenario)
}

fn run_translated_scalar_load_in_memory_system(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    scenario: &str,
) -> Value {
    run_translated_scalar_load_with_options(
        path,
        memory_system,
        switch_tick,
        scenario,
        "detailed",
        "",
    )
}

fn run_translated_scalar_load_with_options(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    scenario: &str,
    cpu_mode: &str,
    extra_run: &str,
) -> Value {
    let output = translated_scalar_load_output(
        path,
        memory_system,
        switch_tick,
        scenario,
        cpu_mode,
        extra_run,
    );
    assert!(
        output.status.success(),
        "translated {scenario} {memory_system} {cpu_mode} switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn translated_scalar_load_output(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    scenario: &str,
    cpu_mode: &str,
    extra_run: &str,
) -> std::process::Output {
    let phase = if switch_tick.is_some() {
        "switch"
    } else {
        "baseline"
    };
    let workspace = temp_workspace(&format!(
        "translated-scalar-load-{scenario}-{}-{phase}",
        memory_system.replace('-', "_")
    ));
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        translated_config_with_options(path, switch_tick, memory_system, cpu_mode, "", extra_run),
    )
    .unwrap();
    Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap()
}

fn translated_config(
    path: &Path,
    switch_tick: Option<u64>,
    extra_mapping: &str,
    extra_run: &str,
) -> String {
    translated_config_in_memory_system(path, switch_tick, "direct", extra_mapping, extra_run)
}

fn translated_config_in_memory_system(
    path: &Path,
    switch_tick: Option<u64>,
    memory_system: &str,
    extra_mapping: &str,
    extra_run: &str,
) -> String {
    translated_config_with_options(
        path,
        switch_tick,
        memory_system,
        "detailed",
        extra_mapping,
        extra_run,
    )
}

fn translated_config_with_options(
    path: &Path,
    switch_tick: Option<u64>,
    memory_system: &str,
    cpu_mode: &str,
    extra_mapping: &str,
    extra_run: &str,
) -> String {
    let switch = switch_tick
        .map(|tick| format!("host_execution_mode_switches = [\"{tick}:cpu0:timing\"]\n"))
        .unwrap_or_default();
    format!(
        r#"[run]
isa = "riscv"
binary = "{}"
max_tick = {MAX_TICK}
stats_format = "json"
execute = true
memory_system = "{memory_system}"
memory_route_delay = 16
m5_switch_cpu_mode = "{cpu_mode}"
riscv_o3_scalar_memory_depth = 4
debug_flags = ["O3", "Data", "Fetch", "Memory", "HostAction"]
memory_dumps = ["0x80000080:16"]
{switch}{extra_run}
[run.riscv_data_translation]
queue_capacity = 4
latency = 2
tlb_capacity = 4
page_size = 4096

[[run.riscv_data_translation.mappings]]
virtual_base = {VIRTUAL_PAGE}
physical_base = {PHYSICAL_PAGE}
pages = 1
read = true
write = true
{extra_mapping}"#,
        path.display()
    )
}

fn cold_translated_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = cold_translated_scalar_load_body();
    append_host_stop(&mut words);
    translated_scalar_load_binary(name, words)
}

fn cold_translated_scalar_load_checkpoint_binary(name: &str) -> std::path::PathBuf {
    let mut words = cold_translated_scalar_load_body();
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        m5op(M5_CHECKPOINT),
        m5op(M5_DUMP_STATS),
    ]);
    words.push(i_type(1, 16, 0x0, 16, 0x13));
    for _ in 0..4 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    append_host_stop(&mut words);
    translated_scalar_load_binary(name, words)
}

fn cold_translated_scalar_load_body() -> Vec<u32> {
    vec![
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(DATA_OFFSET as i32, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 12, 14, 0x0, 15, 0x33),
        s_type((DATA_OFFSET + 4) as i32, 13, 10, 0b010),
        s_type((DATA_OFFSET + 8) as i32, 14, 10, 0b010),
        s_type((DATA_OFFSET + 12) as i32, 15, 10, 0b010),
    ]
}

fn cached_translated_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(DATA_OFFSET as i32, 10, 0b010, 11, 0x03),
        i_type(DATA_OFFSET as i32, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 12, 14, 0x0, 15, 0x33),
        s_type((DATA_OFFSET + 4) as i32, 13, 10, 0b010),
        s_type((DATA_OFFSET + 8) as i32, 14, 10, 0b010),
        s_type((DATA_OFFSET + 12) as i32, 15, 10, 0b010),
    ];
    append_host_stop(&mut words);
    translated_scalar_load_binary(name, words)
}

fn translated_scalar_load_binary(name: &str, mut words: Vec<u32>) -> std::path::PathBuf {
    while words.len() * 4 < DATA_OFFSET as usize {
        words.push(0);
    }
    words.push(42);
    words.extend([0; 15]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(PHYSICAL_PAGE, PHYSICAL_PAGE, &program);
    temp_binary(name, &elf)
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc)
        .unwrap_or_else(|| panic!("missing translated O3 event at {pc}: {json}"))
}

fn event_at_pc_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}

fn checkpoint_component_chunk<'a>(action: &'a Value, component: &str, chunk: &str) -> &'a Value {
    action
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks
                .iter()
                .find(|entry| entry.pointer("/name").and_then(Value::as_str) == Some(chunk))
        })
        .unwrap_or_else(|| {
            panic!("missing translated checkpoint component/chunk {component}/{chunk}: {action}")
        })
}
