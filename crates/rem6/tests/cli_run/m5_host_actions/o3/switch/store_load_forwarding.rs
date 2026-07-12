use std::collections::BTreeSet;

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::*;

const OLDER_STORE_PC: &str = "0x80000010";
const YOUNGER_LOAD_PC: &str = "0x80000014";
const DEPENDENT_ALU_PC: &str = "0x80000018";
const DATA_ADDRESS: &str = "0x80000100";
const RESULTS: &str = "2a0000002a0000002b000000";

#[test]
fn rem6_run_host_switch_transfers_full_forwarded_store_load_direct() {
    assert_full_forwarded_store_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_full_forwarded_store_load_cache_fabric_dram() {
    assert_full_forwarded_store_load_handoff("cache-fabric-dram");
}

#[test]
fn rem6_run_host_switch_rejects_partial_forwarded_store_load_handoff() {
    let path = partial_forwarded_store_load_binary("host-switch-partial-forwarded-store-load");
    let baseline = run_full_forwarded_store_load_handoff(&path, "direct", None);
    let store = event_at_pc(&baseline, OLDER_STORE_PC);
    let load = event_at_pc(&baseline, YOUNGER_LOAD_PC);
    let load_issue = event_u64(load, "issue_tick");
    let first_response =
        event_u64(load, "lsq_data_response_tick").min(event_u64(store, "lsq_data_response_tick"));
    let switch_tick = load_issue.saturating_add(first_response.saturating_sub(load_issue) / 2);
    assert!(load_issue < switch_tick && switch_tick < first_response);

    let output = full_forwarded_store_load_command(&path, "direct", Some(switch_tick))
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("checkpoint component is not quiescent: cpu0"),
        "unexpected partial-forward handoff error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_full_forwarded_store_load_handoff(memory_system: &str) {
    let path = full_forwarded_store_load_binary(&format!(
        "host-switch-full-forwarded-store-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_full_forwarded_store_load_handoff(&path, memory_system, None);
    let baseline_store = event_at_pc(&baseline, OLDER_STORE_PC);
    let baseline_load = event_at_pc(&baseline, YOUNGER_LOAD_PC);
    let load_issue = event_u64(baseline_load, "issue_tick");
    let load_response = event_u64(baseline_load, "lsq_data_response_tick");
    let store_response = event_u64(baseline_store, "lsq_data_response_tick");
    let switch_tick =
        load_response.saturating_add(store_response.saturating_sub(load_response) / 2);
    assert!(load_response < switch_tick && switch_tick < store_response);

    let json = run_full_forwarded_store_load_handoff(&path, memory_system, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(RESULTS)
    );
    for (register, value) in [("x12", "0x2a"), ("x13", "0x2b")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "forwarded handoff must preserve {register}: {json}"
        );
    }

    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .unwrap_or_else(|| panic!("missing forwarded store-load timing switch: {switches:?}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch action tick");
    assert!(load_response < timing_action_tick && timing_action_tick < store_response);
    assert!(timing_action_tick >= switch_tick);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing forwarded store-load handoff: {timing_switch}"));
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
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(2)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    for (field, expected) in [
        ("outstanding_requests", 1),
        ("resident_rows", 2),
        ("transport_owned_rows", 1),
        ("forwarded_rows", 1),
        ("younger_rows", 0),
        ("first_bytes", 4),
        ("first_forwarded_bytes", 4),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "forwarded handoff field {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_operation")
            .and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_address")
            .and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_data_hex")
            .and_then(Value::as_str),
        Some("2a000000")
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_issue_tick")
            .and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_response_tick")
            .and_then(Value::as_u64),
        Some(load_response)
    );
    assert_eq!(
        handoff.pointer("/first_forwarding_source_data_request_agent"),
        handoff.pointer("/first_data_request_agent")
    );
    assert_eq!(
        handoff.pointer("/first_forwarding_source_data_request_sequence"),
        handoff.pointer("/first_data_request_sequence")
    );

    for pc in [OLDER_STORE_PC, YOUNGER_LOAD_PC] {
        let baseline_event = event_at_pc(&baseline, pc);
        let transferred = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    assert!(event_at_pc_if_present(&json, DEPENDENT_ALU_PC).is_none());
    assert_eq!(data_memory_request_count(&json), 3);
    assert_forwarding_trace(&json, timing_action_tick, handoff);
    assert_memory_resources(&json, memory_system);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction forwarded handoff trace: {json}"));
    assert_eq!(
        transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn assert_forwarding_trace(json: &Value, switch_tick: u64, handoff: &Value) {
    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing forwarded Data trace: {json}"));
    assert_eq!(data.len(), 4);
    let observed = data
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
        observed,
        BTreeSet::from([
            ("load", DATA_ADDRESS, 4),
            ("store", DATA_ADDRESS, 4),
            ("store", "0x80000104", 4),
            ("store", "0x80000108", 4),
        ])
    );

    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing forwarded Memory trace: {json}"));
    let request = memory
        .iter()
        .find(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < switch_tick)
        })
        .unwrap_or_else(|| panic!("missing pre-switch store request: {memory:?}"));
    assert_eq!(
        handoff
            .pointer("/first_data_request_agent")
            .and_then(Value::as_u64),
        request.pointer("/request_agent").and_then(Value::as_u64)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_sequence")
            .and_then(Value::as_u64),
        request.pointer("/request").and_then(Value::as_u64)
    );
}

fn assert_memory_resources(json: &Value, memory_system: &str) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        let value = json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
        if memory_system == "direct" && pointer != "/memory_resources/transport/data/activity" {
            assert_eq!(value, 0, "direct handoff should bypass {pointer}: {json}");
        } else {
            assert!(value > 0, "handoff should exercise {pointer}: {json}");
        }
    }
}

fn run_full_forwarded_store_load_handoff(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
) -> Value {
    let output = full_forwarded_store_load_command(path, memory_system, switch_tick)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{memory_system} switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn full_forwarded_store_load_command(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "1500",
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "2",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        "0x80000100:12",
    ]);
    if let Some(tick) = switch_tick {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu0:timing")]);
    }
    command
}

fn full_forwarded_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 10, 0b010),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0; 16]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn partial_forwarded_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(1, 11, 10, 0b000),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x8033_2211, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc).unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
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

fn data_memory_request_count(json: &Value) -> usize {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("run JSON Memory trace")
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .map(|record| {
            (
                record.pointer("/request_agent").and_then(Value::as_u64),
                record.pointer("/request").and_then(Value::as_u64),
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn transfer_handoff_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    == Some(RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_live_data_handoff"))
        .unwrap_or_else(|| panic!("missing decoded live-data handoff chunk: {transfer}"))
}
