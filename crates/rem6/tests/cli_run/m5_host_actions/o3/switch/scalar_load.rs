use std::collections::BTreeSet;

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::*;

const LOAD_PC: &str = "0x80000014";
const FIRST_ALU_PC: &str = "0x80000018";
const SECOND_ALU_PC: &str = "0x8000001c";
const THIRD_ALU_PC: &str = "0x80000020";
const STORE_PCS: [&str; 3] = ["0x80000024", "0x80000028", "0x8000002c"];
const RESULTS: &str = "2a000000050000001000000015000000";

#[test]
fn rem6_run_host_switch_transfers_outstanding_o3_scalar_load_direct() {
    assert_live_scalar_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_outstanding_o3_scalar_load_cache_fabric_dram() {
    assert_live_scalar_load_handoff("cache-fabric-dram");
}

fn assert_live_scalar_load_handoff(memory_system: &str) {
    let path = live_scalar_load_handoff_binary(&format!(
        "host-switch-live-o3-scalar-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_live_scalar_load_handoff(&path, memory_system, None);
    let baseline_load = event_at_pc(&baseline, LOAD_PC);
    let load_issue = event_u64(baseline_load, "issue_tick");
    let load_response = event_u64(baseline_load, "lsq_data_response_tick");
    let switch_tick = load_issue.saturating_add(load_response.saturating_sub(load_issue) / 2);
    assert!(load_issue < switch_tick && switch_tick < load_response);

    let json = run_live_scalar_load_handoff(&path, memory_system, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(RESULTS)
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
            "mode handoff must preserve {register}: {json}"
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
        .unwrap_or_else(|| panic!("missing live scalar-load timing switch: {switches:?}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch action tick");
    assert!(load_issue < timing_action_tick && timing_action_tick < load_response);
    assert!(timing_action_tick >= switch_tick);
    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing live scalar-load handoff: {timing_switch}"));
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
    assert!(transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .is_some_and(|components| components.iter().any(|component| {
            component.pointer("/component").and_then(Value::as_str) == Some("memory0")
        })));

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        runtime
            .pointer("/stats_max_rob_occupancy")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime
            .pointer("/stats_max_lsq_occupancy")
            .and_then(Value::as_u64),
        Some(1)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        handoff
            .pointer("/outstanding_requests")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff.pointer("/resident_rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff
            .pointer("/transport_owned_rows")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff
            .pointer("/buffered_store_rows")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        handoff
            .pointer("/partial_overlay_source_rows")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        handoff.pointer("/first_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80000080")
    );
    assert_eq!(
        handoff.pointer("/first_bytes").and_then(Value::as_u64),
        Some(4)
    );

    for pc in [LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, THIRD_ALU_PC] {
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
    let transferred_load = event_at_pc(&json, LOAD_PC);
    assert!(event_u64(transferred_load, "issue_tick") < timing_action_tick);
    assert!(timing_action_tick < event_u64(transferred_load, "lsq_data_response_tick"));
    assert!(STORE_PCS
        .iter()
        .all(|pc| event_at_pc_if_present(&json, pc).is_none()));

    assert_data_and_memory_trace(&json, timing_action_tick, load_response, handoff);
    assert_memory_resources(&json, memory_system);
    for (path, expected) in [
        (
            "sim.host_actions.execution_mode_switch_state_transfer.live_data_handoffs",
            1,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.non_restorable",
            1,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_live_data_handoff",
            1,
        ),
        (
            "sim.host_actions.execution_mode_switch_state_transfer.latest_restorable",
            0,
        ),
    ] {
        assert_json_stat(&json, path, "Count", expected, "monotonic");
    }
    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction handoff trace: {json}"));
    assert_eq!(
        trace_switch.pointer("/state_transfer/live_data_handoff"),
        transfer.pointer("/live_data_handoff")
    );
    assert_eq!(
        trace_switch.pointer("/state_transfer/restorable"),
        transfer.pointer("/restorable")
    );
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

fn assert_data_and_memory_trace(
    json: &Value,
    switch_tick: u64,
    response_tick: u64,
    handoff: &Value,
) {
    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing Data trace: {json}"));
    assert_eq!(
        data.len(),
        4,
        "handoff must execute one load and three stores"
    );
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
            ("load", "0x80000080", 4),
            ("store", "0x80000084", 4),
            ("store", "0x80000088", 4),
            ("store", "0x8000008c", 4),
        ])
    );
    assert!(data
        .iter()
        .filter(|record| { record.pointer("/kind").and_then(Value::as_str) == Some("store") })
        .all(|record| {
            record
                .pointer("/tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick > switch_tick)
        }));

    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing Memory trace: {json}"));
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
        .unwrap_or_else(|| panic!("missing pre-switch load request: {memory:?}"));
    let request_agent = request
        .pointer("/request_agent")
        .and_then(Value::as_u64)
        .expect("load request agent");
    let request_sequence = request
        .pointer("/request")
        .and_then(Value::as_u64)
        .expect("load request sequence");
    let request_route = request
        .pointer("/route")
        .and_then(Value::as_u64)
        .expect("load request route");
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
    assert_eq!(
        handoff.pointer("/first_route").and_then(Value::as_u64),
        Some(request_route)
    );
    let target = handoff
        .pointer("/first_target")
        .unwrap_or_else(|| panic!("missing typed memory target: {handoff}"));
    assert_eq!(
        target.pointer("/kind").and_then(Value::as_str),
        Some("memory")
    );
    assert_eq!(
        target.pointer("/source_partition").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        target.pointer("/route").and_then(Value::as_u64),
        Some(request_route)
    );
    assert_eq!(
        handoff.pointer("/first_partition").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        handoff
            .pointer("/first_o3_sequence")
            .and_then(Value::as_u64),
        event_at_pc(json, LOAD_PC)
            .pointer("/sequence")
            .and_then(Value::as_u64)
    );
    assert_eq!(handoff.pointer("/first_trace_sequence"), Some(&Value::Null));

    let fetch = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records
                .iter()
                .find(|record| record.pointer("/pc").and_then(Value::as_str) == Some(LOAD_PC))
        })
        .unwrap_or_else(|| panic!("missing load fetch trace: {json}"));
    let fetch_sequence = fetch
        .pointer("/sequence")
        .and_then(Value::as_u64)
        .expect("load fetch sequence");
    let fetch_request = memory
        .iter()
        .find(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("fetch")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record.pointer("/request").and_then(Value::as_u64) == Some(fetch_sequence)
        })
        .unwrap_or_else(|| panic!("missing load fetch request: {memory:?}"));
    assert_eq!(
        handoff
            .pointer("/first_fetch_request_sequence")
            .and_then(Value::as_u64),
        Some(fetch_sequence)
    );
    assert_eq!(
        handoff
            .pointer("/first_fetch_request_agent")
            .and_then(Value::as_u64),
        fetch_request
            .pointer("/request_agent")
            .and_then(Value::as_u64)
    );
    assert!(memory.iter().any(|record| {
        record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            && record.pointer("/tick").and_then(Value::as_u64) == Some(response_tick)
            && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
            && record.pointer("/request").and_then(Value::as_u64) == Some(request_sequence)
    }));
}

pub(super) fn assert_memory_resources(json: &Value, memory_system: &str) {
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        let value = json.pointer(pointer).and_then(Value::as_u64).unwrap();
        if memory_system == "direct" {
            assert_eq!(value, 0, "direct handoff should bypass {pointer}: {json}");
        } else {
            assert!(
                value > 0,
                "hierarchy handoff should exercise {pointer}: {json}"
            );
        }
    }
}

fn run_live_scalar_load_handoff(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
) -> Value {
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
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        "0x80000080:16",
    ]);
    if let Some(tick) = switch_tick {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu0:timing")]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn live_scalar_load_handoff_binary(name: &str) -> std::path::PathBuf {
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

pub(super) fn transfer_handoff_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
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
