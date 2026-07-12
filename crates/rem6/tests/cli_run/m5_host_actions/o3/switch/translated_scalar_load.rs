use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::*;

const COLD_LOAD_PC: &str = "0x80000008";
const CACHED_LOAD_PC: &str = "0x8000000c";
const FIRST_ALU_PC: &str = "0x80000010";
const SECOND_ALU_PC: &str = "0x80000014";
const THIRD_ALU_PC: &str = "0x80000018";
const STORE_PCS: [&str; 3] = ["0x8000001c", "0x80000020", "0x80000024"];
const VIRTUAL_PAGE: u64 = 0x4000;
const PHYSICAL_PAGE: u64 = 0x8000_0000;
const DATA_OFFSET: u64 = 0x80;
const MAX_TICK: u64 = 800;
const COLD_RESULTS: &str = "2a000000000000000000000000000000";
const CACHED_RESULTS: &str = "2a00000005000000100000003a000000";

#[test]
fn rem6_run_host_switch_transfers_outstanding_o3_translated_scalar_load_direct() {
    let path = cold_translated_scalar_load_binary("host-switch-live-o3-translated-scalar-load");
    let baseline = run_translated_scalar_load(&path, None, "cold");
    let baseline_load = event_at_pc(&baseline, COLD_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    assert_stopped_with_headroom(&baseline, response_tick);
    let switch_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < switch_tick && switch_tick < response_tick);

    let json = run_translated_scalar_load(&path, Some(switch_tick), "cold");

    assert_stopped_with_headroom(&json, response_tick);
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(COLD_RESULTS)
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
            "cold translated handoff must preserve {register}: {json}"
        );
    }
    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing cold translated Data trace: {json}"));
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0].pointer("/tick").and_then(Value::as_u64),
        Some(response_tick)
    );
    assert_eq!(
        data[0].pointer("/address").and_then(Value::as_str),
        Some("0x80000080")
    );

    let (transfer, action_tick) = translated_live_transfer(&json);
    assert!(issue_tick < action_tick && action_tick < response_tick);
    let handoff = assert_translated_handoff(transfer, 1, 0, issue_tick);
    assert_translated_request_identity(
        &json,
        handoff,
        event_at_pc(&json, COLD_LOAD_PC),
        action_tick,
        response_tick,
    );
    assert!([CACHED_LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC]
        .iter()
        .all(|pc| event_at_pc_if_present(&json, pc).is_none()));

    let transferred_load = event_at_pc(&json, COLD_LOAD_PC);
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            event_u64(transferred_load, field),
            event_u64(baseline_load, field),
            "cold translated handoff must preserve {field}: {transferred_load}"
        );
    }
}

#[test]
fn rem6_run_host_switch_transfers_cached_translated_scalar_load_younger_window_direct() {
    let path =
        cached_translated_scalar_load_binary("host-switch-live-o3-cached-translated-scalar-load");
    let baseline = run_translated_scalar_load(&path, None, "cached");
    let baseline_load = event_at_pc(&baseline, CACHED_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    assert_stopped_with_headroom(&baseline, response_tick);
    let switch_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < switch_tick && switch_tick < response_tick);

    let json = run_translated_scalar_load(&path, Some(switch_tick), "cached");

    assert_stopped_with_headroom(&json, response_tick);
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(CACHED_RESULTS)
    );
    for (register, value) in [
        ("x11", "0x2a"),
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x3a"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "cached translated handoff must preserve {register}: {json}"
        );
    }

    let (transfer, action_tick) = translated_live_transfer(&json);
    assert!(issue_tick < action_tick && action_tick < response_tick);
    let handoff = assert_translated_handoff(transfer, 4, 3, issue_tick);
    assert_translated_request_identity(
        &json,
        handoff,
        event_at_pc(&json, CACHED_LOAD_PC),
        action_tick,
        response_tick,
    );

    for pc in [CACHED_LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, THIRD_ALU_PC] {
        let baseline_event = event_at_pc(&baseline, pc);
        let transferred = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "translated handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    let transferred_load = event_at_pc(&json, CACHED_LOAD_PC);
    assert_eq!(
        event_u64(transferred_load, "lsq_data_response_tick"),
        event_u64(baseline_load, "lsq_data_response_tick")
    );
    assert!(event_u64(event_at_pc(&json, FIRST_ALU_PC), "issue_tick") < response_tick);
    assert!(event_u64(event_at_pc(&json, SECOND_ALU_PC), "issue_tick") < response_tick);
    assert!(event_u64(event_at_pc(&json, THIRD_ALU_PC), "issue_tick") > response_tick);
    assert!(STORE_PCS
        .iter()
        .all(|pc| event_at_pc_if_present(&json, pc).is_none()));

    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing cached translated Data trace: {json}"));
    assert_eq!(data.len(), 5);
    let observed = data
        .iter()
        .map(|record| {
            (
                record.pointer("/kind").and_then(Value::as_str).unwrap(),
                record.pointer("/address").and_then(Value::as_str).unwrap(),
                record.pointer("/size").and_then(Value::as_u64).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        vec![
            ("load", "0x80000080", 4),
            ("load", "0x80000080", 4),
            ("store", "0x80000084", 4),
            ("store", "0x80000088", 4),
            ("store", "0x8000008c", 4),
        ]
    );
    assert!(data
        .iter()
        .filter(|record| record.pointer("/kind").and_then(Value::as_str) == Some("store"))
        .all(|record| record
            .pointer("/tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > action_tick)));
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
        Some(CACHED_RESULTS)
    );
    for pc in [CACHED_LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, THIRD_ALU_PC] {
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

fn translated_live_transfer(json: &Value) -> (&Value, u64) {
    let timing_switch = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing translated timing switch: {json}"));
    let action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("translated switch action tick");
    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing translated live handoff: {timing_switch}"));
    (transfer, action_tick)
}

fn assert_translated_handoff(
    transfer: &Value,
    rob_rows: u64,
    younger_rows: u64,
    issue_tick: u64,
) -> &Value {
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        transfer
            .pointer("/quiescence_gate/validated")
            .and_then(Value::as_bool),
        Some(false)
    );
    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    for (pointer, expected) in [
        ("/snapshot_rob_entries", rob_rows),
        ("/snapshot_lsq_entries", 1),
        ("/stats_max_rob_occupancy", rob_rows),
        ("/stats_max_lsq_occupancy", 1),
    ] {
        assert_eq!(
            runtime.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated runtime field {pointer}: {runtime}"
        );
    }

    let handoff = translated_handoff_chunk(transfer);
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", younger_rows),
        ("/first_issue_tick", issue_tick),
        ("/last_issue_tick", issue_tick),
        ("/first_bytes", 4),
    ] {
        assert_eq!(
            handoff.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated handoff field {pointer}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80000080")
    );
    assert_eq!(
        handoff
            .pointer("/first_target/kind")
            .and_then(Value::as_str),
        Some("memory")
    );
    handoff
}

fn assert_translated_request_identity(
    json: &Value,
    handoff: &Value,
    load: &Value,
    action_tick: u64,
    response_tick: u64,
) {
    assert_eq!(
        handoff
            .pointer("/first_o3_sequence")
            .and_then(Value::as_u64),
        load.pointer("/sequence").and_then(Value::as_u64)
    );
    let fetch_agent = handoff
        .pointer("/first_fetch_request_agent")
        .and_then(Value::as_u64)
        .expect("translated fetch request agent");
    let fetch_sequence = handoff
        .pointer("/first_fetch_request_sequence")
        .and_then(Value::as_u64)
        .expect("translated fetch request sequence");
    let data_agent = handoff
        .pointer("/first_data_request_agent")
        .and_then(Value::as_u64)
        .expect("translated data request agent");
    let data_sequence = handoff
        .pointer("/first_data_request_sequence")
        .and_then(Value::as_u64)
        .expect("translated data request sequence");
    let load_pc = load
        .pointer("/pc")
        .and_then(Value::as_str)
        .expect("translated load PC");
    let fetch = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated Fetch trace: {json}"));
    assert!(fetch.iter().any(|record| {
        record.pointer("/cpu").and_then(Value::as_u64) == Some(0)
            && record.pointer("/pc").and_then(Value::as_str) == Some(load_pc)
            && record.pointer("/sequence").and_then(Value::as_u64) == Some(fetch_sequence)
    }));
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated Memory trace: {json}"));
    assert!(memory.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("fetch")
            && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(fetch_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(fetch_sequence)
    }));
    assert!(memory.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
            && record
                .pointer("/tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick < action_tick)
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(data_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(data_sequence)
    }));
    assert!(memory.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            && record.pointer("/tick").and_then(Value::as_u64) == Some(response_tick)
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(data_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(data_sequence)
    }));
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
            "multicore",
            translated_config(&path, None, "", "cores = 2\n"),
            &[][..],
            "RISC-V data translation currently requires exactly one core",
        ),
        (
            "cache-fabric-dram",
            base.replace(
                "memory_system = \"direct\"",
                "memory_system = \"cache-fabric-dram\"",
            ),
            &[][..],
            "RISC-V data translation currently requires memory_system = \"direct\"",
        ),
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
    let phase = if switch_tick.is_some() {
        "switch"
    } else {
        "baseline"
    };
    let workspace = temp_workspace(&format!("translated-scalar-load-{scenario}-{phase}"));
    let config = workspace.join("run.toml");
    std::fs::write(&config, translated_config(path, switch_tick, "", "")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "translated {scenario} switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn translated_config(
    path: &Path,
    switch_tick: Option<u64>,
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
memory_system = "direct"
memory_route_delay = 16
m5_switch_cpu_mode = "detailed"
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
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(DATA_OFFSET as i32, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 12, 14, 0x0, 15, 0x33),
    ];
    append_host_stop(&mut words);
    while words.len() * 4 < DATA_OFFSET as usize {
        words.push(0);
    }
    words.push(42);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(PHYSICAL_PAGE, PHYSICAL_PAGE, &program);
    temp_binary(name, &elf)
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
    while words.len() * 4 < DATA_OFFSET as usize {
        words.push(0);
    }
    words.push(42);
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

fn translated_handoff_chunk(transfer: &Value) -> &Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components
                .iter()
                .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some("cpu0"))
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    == Some(RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_live_data_handoff"))
        .unwrap_or_else(|| panic!("missing translated live-data handoff chunk: {transfer}"))
}
