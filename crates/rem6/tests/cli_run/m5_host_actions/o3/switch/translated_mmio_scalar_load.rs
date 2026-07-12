use super::*;

const COLD_LOAD_PC: &str = "0x80000008";
const CACHED_LOAD_PC: &str = "0x8000000c";
const YOUNGER_ALU_PCS: [&str; 3] = ["0x80000010", "0x80000014", "0x80000018"];
const VIRTUAL_PAGE: u64 = 0x4000;
const MMIO_PAGE: u64 = 0x1000_0000;
const MMIO_ADDRESS: &str = "0x10000000";
const MMIO_VALUE: &str = "0x123456789abcdef";
const ROUTE_DELAY: u64 = 16;
const HOST_EVENT_DELAY: u64 = 1;
const MAX_TICK: u64 = 600;

#[test]
fn rem6_run_host_switch_transfers_cached_translated_mmio_load_without_younger_window() {
    let path = translated_mmio_scalar_load_binary();
    let readfile = temp_binary(
        "host-switch-cached-translated-mmio-data",
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );
    let baseline = run_translated_mmio_scalar_load(&path, &readfile, None);
    let baseline_load = event_at_pc(&baseline, CACHED_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    let earliest_source_tick = issue_tick + 1;
    let latest_source_tick = response_tick
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("translated MMIO response leaves room for a host action");
    assert!(earliest_source_tick <= latest_source_tick);
    let switch_tick = earliest_source_tick + (latest_source_tick - earliest_source_tick) / 2;
    let expected_action_tick = switch_tick + HOST_EVENT_DELAY;

    let json = run_translated_mmio_scalar_load(&path, &readfile, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick < MAX_TICK),
        "translated MMIO run exhausted tick headroom: {json}"
    );
    for (register, value) in [
        ("x11", MMIO_VALUE),
        ("x12", MMIO_VALUE),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x123456789abcdff"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "translated MMIO handoff must preserve {register}: {json}"
        );
    }

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
        .unwrap_or_else(|| panic!("missing translated MMIO timing switch: {json}"));
    let action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("translated MMIO switch action tick");
    assert_eq!(action_tick, expected_action_tick);
    assert!(issue_tick < action_tick && action_tick < response_tick);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing translated MMIO live handoff: {timing_switch}"));
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
    for (pointer, expected) in [("/snapshot_rob_entries", 1), ("/snapshot_lsq_entries", 1)] {
        assert_eq!(
            runtime.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated MMIO runtime field {pointer}: {runtime}"
        );
    }

    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu0");
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 0),
        ("/first_issue_tick", issue_tick),
        ("/last_issue_tick", issue_tick),
        ("/first_partition", 0),
        ("/first_bytes", 8),
    ] {
        assert_eq!(
            handoff.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated MMIO handoff field {pointer}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(MMIO_ADDRESS)
    );
    assert_eq!(handoff.pointer("/first_route"), Some(&Value::Null));
    assert_eq!(
        handoff
            .pointer("/first_o3_sequence")
            .and_then(Value::as_u64),
        event_at_pc(&json, CACHED_LOAD_PC)
            .pointer("/sequence")
            .and_then(Value::as_u64)
    );
    let target = handoff
        .pointer("/first_target")
        .unwrap_or_else(|| panic!("missing typed translated MMIO target: {handoff}"));
    assert_eq!(
        target.pointer("/kind").and_then(Value::as_str),
        Some("mmio")
    );
    for (pointer, expected) in [
        ("/source_partition", 0),
        ("/target_partition", 1),
        ("/request_latency", ROUTE_DELAY),
        ("/response_latency", ROUTE_DELAY),
    ] {
        assert_eq!(
            target.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "translated MMIO target field {pointer}: {target}"
        );
    }

    let transferred_load = event_at_pc(&json, CACHED_LOAD_PC);
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            event_u64(transferred_load, field),
            event_u64(baseline_load, field),
            "translated MMIO handoff must preserve {field}: {transferred_load}"
        );
    }
    assert!(event_at_pc(&json, COLD_LOAD_PC).is_object());
    assert!(YOUNGER_ALU_PCS
        .iter()
        .all(|pc| event_at_pc_if_present(&json, pc).is_none()));
    let exec_trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated MMIO Exec trace: {json}"));
    for pc in YOUNGER_ALU_PCS {
        let execution = exec_trace
            .iter()
            .find(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("missing timing-mode execution at {pc}: {exec_trace:?}"));
        assert_eq!(
            execution.pointer("/retired").and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            execution
                .pointer("/tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick > action_tick),
            "younger ALU {pc} must execute after the mode switch: {execution}"
        );
    }

    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated MMIO Data trace: {json}"));
    let completed_loads = data_trace
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some(MMIO_ADDRESS)
                && record.pointer("/size").and_then(Value::as_u64) == Some(8)
        })
        .collect::<Vec<_>>();
    assert_eq!(completed_loads.len(), 2, "Data trace: {data_trace:?}");
    assert_eq!(data_trace.len(), 2, "Data trace: {data_trace:?}");
    assert_eq!(
        completed_loads[1].pointer("/tick").and_then(Value::as_u64),
        Some(response_tick)
    );

    let fetch_sequence = handoff
        .pointer("/first_fetch_request_sequence")
        .and_then(Value::as_u64)
        .expect("translated MMIO fetch sequence");
    let fetch_trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated MMIO Fetch trace: {json}"));
    assert!(fetch_trace.iter().any(|record| {
        record.pointer("/pc").and_then(Value::as_str) == Some(CACHED_LOAD_PC)
            && record.pointer("/sequence").and_then(Value::as_u64) == Some(fetch_sequence)
    }));
    let memory_trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated MMIO Memory trace: {json}"));
    assert!(memory_trace
        .iter()
        .all(|record| { record.pointer("/channel").and_then(Value::as_str) != Some("data") }));
    assert_eq!(
        json.pointer("/memory_resources/transport/data/activity")
            .and_then(Value::as_u64),
        Some(0),
        "translated readfile MMIO must bypass ordinary data transport: {json}"
    );

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing translated MMIO HostAction trace: {json}"));
    assert_eq!(
        super::scalar_load::transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction translated MMIO state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn translated_mmio_scalar_load_binary() -> std::path::PathBuf {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(0, 10, 0b011, 11, 0x03),
        i_type(0, 10, 0b011, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 12, 14, 0x0, 15, 0x33),
    ];
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("host-switch-cached-translated-mmio", &elf)
}

fn run_translated_mmio_scalar_load(
    path: &Path,
    readfile: &Path,
    switch_tick: Option<u64>,
) -> Value {
    let phase = if switch_tick.is_some() {
        "switch"
    } else {
        "baseline"
    };
    let workspace = temp_workspace(&format!("cached-translated-mmio-{phase}"));
    let config = workspace.join("run.toml");
    let switch = switch_tick
        .map(|tick| format!("host_execution_mode_switches = [\"{tick}:cpu0:timing\"]\n"))
        .unwrap_or_default();
    std::fs::write(
        &config,
        format!(
            r#"[run]
isa = "riscv"
binary = "{}"
max_tick = {MAX_TICK}
stats_format = "json"
execute = true
memory_system = "direct"
memory_route_delay = {ROUTE_DELAY}
host_event_delay = {HOST_EVENT_DELAY}
m5_switch_cpu_mode = "detailed"
riscv_o3_scalar_memory_depth = 4
debug_flags = ["O3", "Data", "Exec", "Fetch", "Memory", "HostAction"]
readfiles = ["0x10000000:0x100:{}"]
{switch}
[run.riscv_data_translation]
queue_capacity = 4
latency = 2
tlb_capacity = 4
page_size = 4096

[[run.riscv_data_translation.mappings]]
virtual_base = {VIRTUAL_PAGE}
physical_base = {MMIO_PAGE}
pages = 1
read = true
write = true
"#,
            path.display(),
            readfile.display()
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "translated MMIO switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc)
        .unwrap_or_else(|| panic!("missing translated MMIO O3 event at {pc}: {json}"))
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
