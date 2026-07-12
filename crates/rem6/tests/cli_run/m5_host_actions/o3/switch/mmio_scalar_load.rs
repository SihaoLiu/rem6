use super::*;

const MMIO_LOAD_PC: &str = "0x80000008";
const MMIO_ADDRESS: &str = "0x10000000";
const MMIO_VALUE: &str = "0x123456789abcdef";
const ROUTE_DELAY: u64 = 16;
const HOST_EVENT_DELAY: u64 = 1;

#[test]
fn rem6_run_host_switch_transfers_outstanding_o3_scalar_load_mmio() {
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        u_type(0x1000_0000, 10, 0x37),
        i_type(0, 10, 0b011, 5, 0x03),
    ];
    append_host_stop(&mut words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("host-switch-live-o3-scalar-load-mmio", &elf);
    let readfile_path = temp_binary(
        "host-switch-live-o3-scalar-load-mmio-data",
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );
    let baseline = run_mmio_scalar_load_handoff(&path, &readfile_path, None);
    let baseline_load = event_at_pc(&baseline, MMIO_LOAD_PC);
    let load_issue = event_u64(baseline_load, "issue_tick");
    let load_response = event_u64(baseline_load, "lsq_data_response_tick");
    let earliest_source_tick = load_issue + 1;
    let latest_source_tick = load_response
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("MMIO response leaves room for a host action");
    assert!(earliest_source_tick <= latest_source_tick);
    let switch_tick = earliest_source_tick + (latest_source_tick - earliest_source_tick) / 2;
    let expected_action_tick = switch_tick + HOST_EVENT_DELAY;

    let json = run_mmio_scalar_load_handoff(&path, &readfile_path, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some(MMIO_VALUE)
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
        .unwrap_or_else(|| panic!("missing live MMIO timing switch: {json}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch action tick");
    assert_eq!(timing_action_tick, expected_action_tick);
    assert!(load_issue < timing_action_tick && timing_action_tick < load_response);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing MMIO live-data handoff: {timing_switch}"));
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

    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu0");
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 0),
        ("/first_issue_tick", load_issue),
        ("/last_issue_tick", load_issue),
        ("/first_partition", 0),
        ("/first_bytes", 8),
    ] {
        assert_eq!(
            handoff.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "handoff field {pointer}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(handoff.pointer("/first_route"), Some(&Value::Null));
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(MMIO_ADDRESS)
    );
    let target = handoff
        .pointer("/first_target")
        .unwrap_or_else(|| panic!("missing typed MMIO target: {handoff}"));
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
            "MMIO target field {pointer}: {target}"
        );
    }

    let transferred_load = event_at_pc(&json, MMIO_LOAD_PC);
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            event_u64(transferred_load, field),
            event_u64(baseline_load, field),
            "MMIO handoff must preserve {field}: {transferred_load}"
        );
    }
    assert!(event_u64(transferred_load, "issue_tick") < timing_action_tick);
    assert!(timing_action_tick < event_u64(transferred_load, "lsq_data_response_tick"));

    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing MMIO data trace: {json}"));
    let completed_loads = data_trace
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some(MMIO_ADDRESS)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        completed_loads.len(),
        1,
        "MMIO handoff must accept exactly one device completion: {data_trace:?}"
    );
    assert_eq!(
        data_trace.len(),
        1,
        "the fixture performs no other data access: {data_trace:?}"
    );
    let completed_load = completed_loads[0];
    assert_eq!(
        completed_load.pointer("/tick").and_then(Value::as_u64),
        Some(load_response)
    );
    assert_eq!(
        completed_load.pointer("/size").and_then(Value::as_u64),
        Some(8)
    );

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
    assert_eq!(
        json.pointer("/memory_resources/transport/data/activity")
            .and_then(Value::as_u64),
        Some(0),
        "readfile MMIO must bypass the ordinary data transport: {json}"
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
        .unwrap_or_else(|| panic!("missing HostAction MMIO handoff trace: {json}"));
    assert_eq!(
        super::scalar_load::transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn run_mmio_scalar_load_handoff(
    path: &Path,
    readfile_path: &Path,
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
        "300",
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,HostAction",
        "--memory-system",
        "direct",
        "--memory-route-delay",
        &ROUTE_DELAY.to_string(),
        "--host-event-delay",
        &HOST_EVENT_DELAY.to_string(),
        "--m5-switch-cpu-mode",
        "detailed",
        "--readfile",
        &format!("0x10000000:0x100:{}", readfile_path.display()),
    ]);
    if let Some(tick) = switch_tick {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu0:timing")]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "MMIO switch {switch_tick:?}; stderr: {}",
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
        .unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}
