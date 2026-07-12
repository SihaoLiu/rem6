use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::*;

const LOAD_PC: &str = "0x80000008";
const VIRTUAL_PAGE: u64 = 0x4000;
const PHYSICAL_PAGE: u64 = 0x8000_0000;
const DATA_OFFSET: u64 = 0x80;

#[test]
fn rem6_run_host_switch_transfers_outstanding_o3_translated_scalar_load_direct() {
    let path = translated_scalar_load_binary("host-switch-live-o3-translated-scalar-load");
    let baseline = run_translated_scalar_load(&path, None);
    let baseline_load = event_at_pc(&baseline, LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    let switch_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < switch_tick && switch_tick < response_tick);

    let json = run_translated_scalar_load(&path, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000")
    );

    let data_record = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("load")
                    && record.pointer("/address").and_then(Value::as_str) == Some("0x80000080")
            })
        })
        .unwrap_or_else(|| panic!("missing translated physical data trace: {json}"));
    assert_eq!(
        data_record.pointer("/tick").and_then(Value::as_u64),
        Some(response_tick)
    );

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
    assert!(issue_tick < action_tick && action_tick < response_tick);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing translated live handoff: {timing_switch}"));
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
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1)
    );

    let handoff = translated_handoff_chunk(transfer);
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 0),
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
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated Memory trace: {json}"));
    let request = memory
        .iter()
        .find(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < action_tick)
        })
        .unwrap_or_else(|| panic!("missing translated pre-switch request: {memory:?}"));
    let request_agent = request
        .pointer("/request_agent")
        .and_then(Value::as_u64)
        .expect("translated request agent");
    let request_sequence = request
        .pointer("/request")
        .and_then(Value::as_u64)
        .expect("translated request sequence");
    assert_eq!(
        handoff
            .pointer("/first_data_request_agent")
            .and_then(Value::as_u64),
        Some(request_agent)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_sequence")
            .and_then(Value::as_u64),
        Some(request_sequence)
    );
    assert!(memory.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            && record.pointer("/tick").and_then(Value::as_u64) == Some(response_tick)
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(request_sequence)
    }));

    let transferred_load = event_at_pc(&json, LOAD_PC);
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            event_u64(transferred_load, field),
            event_u64(baseline_load, field),
            "translated handoff must preserve {field}: {transferred_load}"
        );
    }
}

#[test]
fn rem6_run_rejects_overlapping_riscv_data_translation_mappings() {
    let path = translated_scalar_load_binary("overlapping-riscv-data-translation");
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
    let path = translated_scalar_load_binary("instruction-limit-riscv-data-translation");
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
    let path = translated_scalar_load_binary("unsupported-riscv-data-translation");
    let readfile = temp_binary("unsupported-riscv-data-translation-readfile", &[0; 8]);
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
        (
            "readfile",
            translated_config(
                &path,
                None,
                "",
                &format!(
                    "readfiles = [\"0x10000000:0x100:{}\"]\n",
                    readfile.display()
                ),
            ),
            &[][..],
            "RISC-V data translation does not yet support readfile MMIO",
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

fn run_translated_scalar_load(path: &Path, switch_tick: Option<u64>) -> Value {
    let workspace = temp_workspace(match switch_tick {
        Some(_) => "translated-scalar-load-switch",
        None => "translated-scalar-load-baseline",
    });
    let config = workspace.join("run.toml");
    std::fs::write(&config, translated_config(path, switch_tick, "", "")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "translated switch {switch_tick:?}; stderr: {}",
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
max_tick = 400
stats_format = "json"
execute = true
memory_system = "direct"
memory_route_delay = 16
m5_switch_cpu_mode = "detailed"
debug_flags = ["O3", "Data", "Memory", "HostAction"]
memory_dumps = ["0x80000080:4"]
{switch}{extra_run}
[run.riscv_data_translation]
queue_capacity = 4
latency = 2
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

fn translated_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(DATA_OFFSET as i32, 10, 0b010, 12, 0x03),
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
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("missing translated O3 event at {pc}: {json}"))
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
