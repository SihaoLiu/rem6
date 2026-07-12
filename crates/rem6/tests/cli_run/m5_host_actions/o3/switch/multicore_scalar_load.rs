use super::*;

const CPU1_RESULTS_ADDRESS: &str = "0x80000210";
const CPU1_RESULTS: &str = "2a000000050000001000000015000000";

#[test]
fn rem6_run_host_switch_transfers_multicore_cpu1_scalar_load_direct() {
    assert_multicore_cpu1_scalar_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_multicore_cpu1_scalar_load_cache_fabric_dram() {
    assert_multicore_cpu1_scalar_load_handoff("cache-fabric-dram");
}

fn assert_multicore_cpu1_scalar_load_handoff(memory_system: &str) {
    let path = multicore_cpu1_scalar_load_handoff_binary(&format!(
        "host-switch-multicore-cpu1-scalar-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_multicore_cpu1_scalar_load_handoff(&path, memory_system, None);
    assert_parallel_two_worker_run(&baseline);
    let cpu0_baseline = first_data_window(&baseline, 0);
    let cpu1_baseline = first_data_window(&baseline, 1);
    let overlap_start = cpu0_baseline.request_tick.max(cpu1_baseline.request_tick);
    let overlap_end = cpu0_baseline.response_tick.min(cpu1_baseline.response_tick);
    assert!(
        overlap_start.saturating_add(1) < overlap_end,
        "peer load windows must overlap: cpu0={cpu0_baseline:?}, cpu1={cpu1_baseline:?}"
    );
    let switch_tick = overlap_start + (overlap_end - overlap_start) / 2;

    let json = run_multicore_cpu1_scalar_load_handoff(&path, memory_system, Some(switch_tick));

    assert_parallel_two_worker_run(&json);
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/address").and_then(Value::as_str),
        Some(CPU1_RESULTS_ADDRESS)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(CPU1_RESULTS)
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x21")
            .and_then(Value::as_str),
        Some("0x63"),
        "the peer load must complete without becoming part of CPU1 authority: {json}"
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x15"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/1/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "CPU1 handoff must preserve {register}: {json}"
        );
    }

    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    assert!(
        switches
            .iter()
            .all(|switch| { switch.pointer("/target").and_then(Value::as_str) == Some("cpu1") }),
        "CPU1 guest and host switches must not target CPU0: {switches:?}"
    );
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .unwrap_or_else(|| panic!("missing CPU1 live scalar-load switch: {switches:?}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("CPU1 timing action tick");
    assert!(timing_action_tick >= switch_tick);

    let cpu0_window = first_data_window(&json, 0);
    let cpu1_window = first_data_window(&json, 1);
    for (cpu, window) in [("cpu0", cpu0_window), ("cpu1", cpu1_window)] {
        assert!(
            window.request_tick < timing_action_tick
                && timing_action_tick < window.response_tick,
            "{cpu} load must remain transport-owned across the CPU1 switch at {timing_action_tick}: {window:?}"
        );
    }

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing CPU1 live-data transfer: {timing_switch}"));
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
        .unwrap_or_else(|| panic!("missing CPU1 transfer components: {transfer}"));
    assert!(components.iter().any(|component| {
        component.pointer("/component").and_then(Value::as_str) == Some("cpu1")
    }));
    assert!(
        components.iter().all(|component| {
            component.pointer("/component").and_then(Value::as_str) != Some("cpu0")
        }),
        "non-restorable CPU1 handoff must not capture peer CPU0: {components:?}"
    );

    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu1");
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
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
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        handoff
            .pointer("/first_fetch_request_agent")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_agent")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff.pointer("/first_partition").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_sequence")
            .and_then(Value::as_u64),
        Some(cpu1_window.request_sequence)
    );
    assert_eq!(
        handoff.pointer("/first_route").and_then(Value::as_u64),
        Some(cpu1_window.route)
    );
    let target = handoff
        .pointer("/first_target")
        .unwrap_or_else(|| panic!("missing typed CPU1 memory target: {handoff}"));
    assert_eq!(
        target.pointer("/kind").and_then(Value::as_str),
        Some("memory")
    );
    assert_eq!(
        target.pointer("/source_partition").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        target.pointer("/route").and_then(Value::as_u64),
        Some(cpu1_window.route)
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80000200")
    );
    assert_eq!(
        handoff.pointer("/first_bytes").and_then(Value::as_u64),
        Some(4)
    );

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
    for path in [
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.component.cpu0.components",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.target.cpu1.component.cpu0.components",
    ] {
        assert_json_stat(&json, path, "Count", 1, "monotonic");
    }
    for path in [
        "sim.host_actions.execution_mode_switch_state_transfer.latest_target.cpu1.component.cpu0.components",
        "sim.debug.host_action_trace.execution_mode_switch.state_transfer.latest_target.cpu1.component.cpu0.components",
    ] {
        assert_json_stat_absent(&json, path);
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
        .unwrap_or_else(|| panic!("missing CPU1 HostAction switch trace: {json}"));
    assert_eq!(
        trace_switch.pointer("/target").and_then(Value::as_str),
        Some("cpu1")
    );
    assert_eq!(
        super::scalar_load::transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction state transfer"),
            "cpu1",
        ),
        handoff
    );

    super::scalar_load::assert_memory_resources(&json, memory_system);
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DataWindow {
    pub(super) request_tick: u64,
    pub(super) response_tick: u64,
    pub(super) request_sequence: u64,
    pub(super) route: u64,
}

pub(super) fn first_data_window(json: &Value, request_agent: u64) -> DataWindow {
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing Memory trace: {json}"));
    let request = trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
        })
        .min_by_key(|record| record.pointer("/tick").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing data request for agent {request_agent}: {trace:?}"));
    let request_tick = request
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("data request tick");
    let request_sequence = request
        .pointer("/request")
        .and_then(Value::as_u64)
        .expect("data request sequence");
    let route = request
        .pointer("/route")
        .and_then(Value::as_u64)
        .expect("data request route");
    let response_tick = trace
        .iter()
        .find(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
                && record
                    .pointer("/request_agent")
                    .and_then(Value::as_u64)
                    == Some(request_agent)
                && record.pointer("/request").and_then(Value::as_u64)
                    == Some(request_sequence)
        })
        .and_then(|record| record.pointer("/tick").and_then(Value::as_u64))
        .unwrap_or_else(|| {
            panic!(
                "missing data response for agent {request_agent} request {request_sequence}: {trace:?}"
            )
        });
    DataWindow {
        request_tick,
        response_tick,
        request_sequence,
        route,
    }
}

pub(super) fn assert_parallel_two_worker_run(json: &Value) {
    assert_eq!(
        json.pointer("/parallel/scheduler/worker_limit")
            .and_then(Value::as_u64),
        Some(2),
        "the matrix must not be serialized by O3 debug mode: {json}"
    );
    assert_eq!(
        json.pointer("/parallel/scheduler/max_workers")
            .and_then(Value::as_u64),
        Some(2),
        "the matrix must exercise both scheduler workers: {json}"
    );
}

fn run_multicore_cpu1_scalar_load_handoff(
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
        "3000",
        "--stats-format",
        "json",
        "--execute",
        "--cores",
        "2",
        "--parallel-workers",
        "2",
        "--debug-flags",
        "Data,Fetch,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "64",
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        "0x80000210:16",
    ]);
    if let Some(tick) = switch_tick {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu1:timing")]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} CPU1 switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn multicore_cpu1_scalar_load_handoff_binary(name: &str) -> std::path::PathBuf {
    let data_start = 512_i32;
    let mut words = vec![
        csr_read(0xf14, 5), // csrr x5, mhartid
        0,                  // hart 0 branch patched below
        m5op(M5_SWITCH_CPU),
    ];
    let cpu1_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - cpu1_auipc_pc, 10, 0x0, 10, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 13, 14, 0x0, 15, 0x33),
        s_type(16, 12, 10, 0b010),
        s_type(20, 13, 10, 0b010),
        s_type(24, 14, 10, 0b010),
        s_type(28, 15, 10, 0b010),
    ]);
    append_host_stop(&mut words);

    let cpu0_path = words.len();
    for _ in 0..6 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    let cpu0_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 20, 0x17),
        i_type(data_start - cpu0_auipc_pc, 20, 0x0, 20, 0x13),
        i_type(4, 20, 0b010, 21, 0x03),
        b_type(0, 0, 0, 0x0),
    ]);
    words[1] = b_type(((cpu0_path - 1) * 4) as i32, 0, 5, 0x0);

    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 99, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
