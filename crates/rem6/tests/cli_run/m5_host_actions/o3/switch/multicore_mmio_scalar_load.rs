use super::*;

const MMIO_LOAD_PC: &str = "0x80000010";
const MMIO_ADDRESS: &str = "0x10000000";
const MMIO_VALUE: &str = "0x123456789abcdef";
const CPU0_DATA_ADDRESS: &str = "0x80000204";
const ROUTE_DELAY: u64 = 64;
const HOST_EVENT_DELAY: u64 = 1;

#[test]
fn rem6_run_host_switch_transfers_multicore_cpu1_mmio_with_peer_memory() {
    let path = multicore_cpu1_mmio_handoff_binary();
    let readfile_path = temp_binary(
        "host-switch-multicore-cpu1-mmio-data",
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );
    let baseline = run_multicore_cpu1_mmio_handoff(&path, &readfile_path, None, false);
    super::multicore_scalar_load::assert_parallel_two_worker_run(&baseline);
    let cpu0_baseline = super::multicore_scalar_load::first_data_window(&baseline, 0);
    let mmio_response = data_completion_tick(&baseline, 1, MMIO_ADDRESS);
    let mmio_issue = mmio_response
        .checked_sub(ROUTE_DELAY * 2)
        .expect("MMIO route leaves a request window");
    let overlap_start = cpu0_baseline.request_tick.max(mmio_issue);
    let overlap_end = cpu0_baseline.response_tick.min(mmio_response);
    assert!(
        overlap_start.saturating_add(HOST_EVENT_DELAY + 1) < overlap_end,
        "CPU0 memory and CPU1 MMIO windows must overlap: cpu0={cpu0_baseline:?}, mmio={mmio_issue}..{mmio_response}"
    );
    let expected_action_tick = overlap_start + (overlap_end - overlap_start) / 2;
    let switch_tick = expected_action_tick - HOST_EVENT_DELAY;

    let json = run_multicore_cpu1_mmio_handoff(&path, &readfile_path, Some(switch_tick), false);

    super::multicore_scalar_load::assert_parallel_two_worker_run(&json);
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x21")
            .and_then(Value::as_str),
        Some("0x63"),
        "CPU0 peer memory must complete across the CPU1 MMIO switch: {json}"
    );
    assert_eq!(
        json.pointer("/cores/1/registers/x12")
            .and_then(Value::as_str),
        Some(MMIO_VALUE),
        "CPU1 must receive the readfile MMIO value: {json}"
    );

    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    assert!(
        switches
            .iter()
            .all(|switch| switch.pointer("/target").and_then(Value::as_str) == Some("cpu1")),
        "CPU1 guest and host switches must not target CPU0: {switches:?}"
    );
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .unwrap_or_else(|| panic!("missing CPU1 MMIO timing switch: {switches:?}"));
    let action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("CPU1 timing action tick");
    assert_eq!(action_tick, expected_action_tick);

    let cpu0_window = super::multicore_scalar_load::first_data_window(&json, 0);
    let transferred_mmio_response = data_completion_tick(&json, 1, MMIO_ADDRESS);
    assert_eq!(transferred_mmio_response, mmio_response);
    assert_eq!(cpu0_window.request_tick, cpu0_baseline.request_tick);
    assert_eq!(cpu0_window.response_tick, cpu0_baseline.response_tick);
    assert_eq!(cpu0_window.request_sequence, cpu0_baseline.request_sequence);
    assert_eq!(cpu0_window.route, cpu0_baseline.route);
    assert!(
        cpu0_window.request_tick < action_tick && action_tick < cpu0_window.response_tick,
        "CPU0 request must remain transport-owned across the CPU1 switch: {cpu0_window:?}"
    );

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing CPU1 MMIO live-data transfer: {timing_switch}"));
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
    let components = transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing CPU1 MMIO transfer components: {transfer}"));
    assert!(components.iter().any(|component| {
        component.pointer("/component").and_then(Value::as_str) == Some("cpu1")
    }));
    assert!(
        components.iter().all(|component| {
            component.pointer("/component").and_then(Value::as_str) != Some("cpu0")
        }),
        "target-scoped CPU1 MMIO handoff must not capture CPU0: {components:?}"
    );

    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu1");
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 0),
        ("/first_issue_tick", mmio_issue),
        ("/last_issue_tick", mmio_issue),
        ("/first_partition", 1),
        ("/first_fetch_request_agent", 1),
        ("/first_data_request_agent", 1),
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
    assert_eq!(handoff.pointer("/first_route"), Some(&Value::Null));
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(MMIO_ADDRESS)
    );
    let target = handoff
        .pointer("/first_target")
        .unwrap_or_else(|| panic!("missing typed CPU1 MMIO target: {handoff}"));
    assert_eq!(
        target.pointer("/kind").and_then(Value::as_str),
        Some("mmio")
    );
    for (pointer, expected) in [
        ("/source_partition", 1),
        ("/target_partition", 2),
        ("/request_latency", ROUTE_DELAY),
        ("/response_latency", ROUTE_DELAY),
    ] {
        assert_eq!(
            target.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "MMIO target field {pointer}: {target}"
        );
    }
    assert!(mmio_issue < action_tick && action_tick < transferred_mmio_response);

    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing mixed data trace: {json}"));
    assert_eq!(
        data_trace.len(),
        2,
        "the fixture performs one CPU0 memory load and one CPU1 MMIO load"
    );
    let cpu1_mmio = data_trace
        .iter()
        .filter(|record| {
            record.pointer("/cpu").and_then(Value::as_u64) == Some(1)
                && record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some(MMIO_ADDRESS)
        })
        .collect::<Vec<_>>();
    assert_eq!(cpu1_mmio.len(), 1, "CPU1 MMIO must complete exactly once");
    assert_eq!(
        cpu1_mmio[0].pointer("/tick").and_then(Value::as_u64),
        Some(mmio_response)
    );
    assert!(data_trace.iter().any(|record| {
        record.pointer("/cpu").and_then(Value::as_u64) == Some(0)
            && record.pointer("/address").and_then(Value::as_str) == Some(CPU0_DATA_ADDRESS)
    }));

    let memory_trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing mixed Memory trace: {json}"));
    let data_transport = memory_trace
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .collect::<Vec<_>>();
    assert_eq!(
        data_transport.len(),
        3,
        "ordinary data transport must contain only CPU0's direct route: {memory_trace:?}"
    );
    for (kind, tick, endpoint) in [
        ("request_sent", cpu0_window.request_tick, "cpu0.dmem"),
        (
            "request_arrived",
            cpu0_window.request_tick + ROUTE_DELAY,
            "memory",
        ),
        ("response_arrived", cpu0_window.response_tick, "cpu0.dmem"),
    ] {
        let matches = data_transport
            .iter()
            .filter(|record| record.pointer("/kind").and_then(Value::as_str) == Some(kind))
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "expected one data {kind}: {data_transport:?}"
        );
        let record = matches[0];
        assert_eq!(
            record.pointer("/endpoint").and_then(Value::as_str),
            Some(endpoint),
            "CPU0 data {kind} endpoint: {record}"
        );
        for (pointer, expected) in [
            ("/tick", tick),
            ("/request_agent", 0),
            ("/request", cpu0_window.request_sequence),
            ("/route", cpu0_window.route),
        ] {
            assert_eq!(
                record.pointer(pointer).and_then(Value::as_u64),
                Some(expected),
                "CPU0 data {kind} field {pointer}: {record}"
            );
        }
    }

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
    assert_json_stat_at_least(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.components",
        "Count",
        1,
        "monotonic",
    );
    for path in [
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.components",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu0.components",
    ] {
        assert_json_stat_absent(&json, path);
    }

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing CPU1 MMIO HostAction trace: {json}"));
    assert_eq!(
        super::scalar_load::transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction state transfer"),
            "cpu1",
        ),
        handoff
    );

    assert_o3_timing_replay(&path, &readfile_path);
}

fn data_completion_tick(json: &Value, cpu: u64, address: &str) -> u64 {
    let matches = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing Data trace: {json}"))
        .iter()
        .filter(|record| {
            record.pointer("/cpu").and_then(Value::as_u64) == Some(cpu)
                && record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some(address)
        })
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one CPU{cpu} load at {address}");
    matches[0]
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("data completion tick")
}

fn run_multicore_cpu1_mmio_handoff(
    path: &Path,
    readfile_path: &Path,
    switch_tick: Option<u64>,
    o3_debug: bool,
) -> Value {
    let debug_flags = if o3_debug {
        "O3,Data,HostAction"
    } else {
        "Data,Fetch,Memory,HostAction"
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "3000",
        "--stats-format",
        "json",
        "--execute",
        "--cores",
        "2",
        "--parallel-workers",
        "2",
        "--debug-flags",
        debug_flags,
        "--riscv-o3-scalar-memory-depth",
        "4",
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
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu1:timing")]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "CPU1 MMIO switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_o3_timing_replay(path: &Path, readfile_path: &Path) {
    let baseline = run_multicore_cpu1_mmio_handoff(path, readfile_path, None, true);
    let baseline_load = o3_event_at_pc(&baseline, 1, MMIO_LOAD_PC);
    let issue_tick = event_u64(baseline_load, "issue_tick");
    let response_tick = event_u64(baseline_load, "lsq_data_response_tick");
    assert_eq!(
        data_completion_tick(&baseline, 1, MMIO_ADDRESS),
        response_tick
    );
    let earliest_source_tick = issue_tick + 1;
    let latest_source_tick = response_tick
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("MMIO response leaves room for a traced host action");
    assert!(earliest_source_tick <= latest_source_tick);
    let switch_tick = earliest_source_tick + (latest_source_tick - earliest_source_tick) / 2;
    let expected_action_tick = switch_tick + HOST_EVENT_DELAY;

    let json = run_multicore_cpu1_mmio_handoff(path, readfile_path, Some(switch_tick), true);
    let timing_switch = cpu1_timing_switch(&json);
    assert_eq!(
        timing_switch.pointer("/tick").and_then(Value::as_u64),
        Some(expected_action_tick)
    );
    assert!(issue_tick < expected_action_tick && expected_action_tick < response_tick);
    let transferred_load = o3_event_at_pc(&json, 1, MMIO_LOAD_PC);
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            event_u64(transferred_load, field),
            event_u64(baseline_load, field),
            "CPU1 MMIO handoff must preserve {field}: {transferred_load}"
        );
    }
    assert_eq!(data_completion_tick(&json, 1, MMIO_ADDRESS), response_tick);
}

fn cpu1_timing_switch(json: &Value) -> &Value {
    json.pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing CPU1 MMIO timing switch: {json}"))
}

fn o3_event_at_pc<'a>(json: &'a Value, cpu: u64, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records
                .iter()
                .find(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(cpu))
        })
        .and_then(|record| record.pointer("/events"))
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("missing CPU{cpu} O3 event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}

fn multicore_cpu1_mmio_handoff_binary() -> std::path::PathBuf {
    let data_start = 512_i32;
    let mut words = vec![
        csr_read(0xf14, 5), // csrr x5, mhartid
        0,                  // hart 0 branch patched below
        m5op(M5_SWITCH_CPU),
        u_type(0x1000_0000, 10, 0x37),
        i_type(0, 10, 0b011, 12, 0x03),
    ];
    append_host_stop(&mut words);

    let cpu0_path = words.len();
    let cpu0_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 20, 0x17),
        i_type(data_start - cpu0_auipc_pc, 20, 0x0, 20, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(4, 20, 0b010, 21, 0x03),
        b_type(0, 0, 0, 0x0),
    ]);
    words[1] = b_type(((cpu0_path - 1) * 4) as i32, 0, 5, 0x0);

    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 99]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("host-switch-multicore-cpu1-mmio", &elf)
}
