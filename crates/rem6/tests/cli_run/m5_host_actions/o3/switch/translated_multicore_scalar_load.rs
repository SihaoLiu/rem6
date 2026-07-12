use super::*;

const VIRTUAL_PAGE: u64 = 0x4000;
const PHYSICAL_PAGE: u64 = 0x8000_0000;
const DATA_OFFSET: u64 = 0x200;
const CPU0_DATA_ADDRESS: &str = "0x80000204";
const CPU1_DATA_ADDRESS: &str = "0x80000200";
const CPU1_RESULTS_ADDRESS: &str = "0x80000210";
const CPU1_RESULTS: &str = "2a000000050000001000000015000000";

#[test]
fn rem6_run_host_switch_transfers_multicore_cpu1_cached_translated_scalar_load_cache_fabric_dram() {
    let path = translated_multicore_scalar_load_binary(
        "host-switch-multicore-cpu1-cached-translated-scalar-load-cache-fabric-dram",
    );
    let baseline = run_translated_multicore_scalar_load(&path, None);
    super::multicore_scalar_load::assert_parallel_two_worker_run(&baseline);

    let cpu0_window = cached_load_window(&baseline, 0, CPU0_DATA_ADDRESS);
    let cpu1_window = cached_load_window(&baseline, 1, CPU1_DATA_ADDRESS);
    let overlap_start = cpu0_window.request_tick.max(cpu1_window.request_tick);
    let overlap_end = cpu0_window.response_tick.min(cpu1_window.response_tick);
    assert!(
        overlap_start.saturating_add(1) < overlap_end,
        "translated peer windows must overlap: cpu0={cpu0_window:?}, cpu1={cpu1_window:?}"
    );
    let switch_tick = overlap_start + (overlap_end - overlap_start) / 2;

    let json = run_translated_multicore_scalar_load(&path, Some(switch_tick));

    super::multicore_scalar_load::assert_parallel_two_worker_run(&json);
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
        "the translated CPU0 peer load must complete independently: {json}"
    );
    for (register, value) in [
        ("x11", "0x2a"),
        ("x12", "0x2a"),
        ("x13", "0x5"),
        ("x14", "0x10"),
        ("x15", "0x15"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/1/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "CPU1 translated handoff must preserve {register}: {json}"
        );
    }

    let timing_switch = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            assert!(
                switches.iter().all(|switch| {
                    switch.pointer("/target").and_then(Value::as_str) == Some("cpu1")
                }),
                "translated switches must remain CPU1-scoped: {switches:?}"
            );
            switches.iter().find(|switch| {
                switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing CPU1 translated timing switch: {json}"));
    let action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("CPU1 translated switch tick");
    let cpu0_window = cached_load_window(&json, 0, CPU0_DATA_ADDRESS);
    let cpu1_window = cached_load_window(&json, 1, CPU1_DATA_ADDRESS);
    for (cpu, window) in [("cpu0", cpu0_window), ("cpu1", cpu1_window)] {
        assert!(
            window.request_tick < action_tick && action_tick < window.response_tick,
            "{cpu} translated load must remain transport-owned across tick {action_tick}: {window:?}"
        );
    }

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing CPU1 translated state transfer: {timing_switch}"));
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
        .unwrap_or_else(|| panic!("missing CPU1 translated transfer components: {transfer}"));
    assert!(components.iter().any(|component| {
        component.pointer("/component").and_then(Value::as_str) == Some("cpu1")
    }));
    assert!(
        components.iter().all(|component| {
            component.pointer("/component").and_then(Value::as_str) != Some("cpu0")
        }),
        "CPU1 translated handoff must not capture peer CPU0: {components:?}"
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu1");
    for (pointer, expected) in [
        ("/snapshot_rob_entries", 4),
        ("/snapshot_lsq_entries", 1),
        ("/stats_max_rob_occupancy", 4),
        ("/stats_max_lsq_occupancy", 1),
    ] {
        assert_eq!(
            runtime.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "CPU1 translated runtime field {pointer}: {runtime}"
        );
    }
    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu1");
    for (pointer, expected) in [
        ("/outstanding_requests", 1),
        ("/resident_rows", 1),
        ("/younger_rows", 3),
        ("/first_fetch_request_agent", 1),
        ("/first_data_request_agent", 1),
        ("/first_partition", 1),
        ("/first_bytes", 4),
    ] {
        assert_eq!(
            handoff.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "CPU1 translated handoff field {pointer}: {handoff}"
        );
    }
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
    assert!(
        handoff
            .pointer("/first_issue_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick <= cpu1_window.request_tick && tick < action_tick),
        "CPU1 translated issue must precede the host switch: {handoff}"
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(CPU1_DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_target/kind")
            .and_then(Value::as_str),
        Some("memory")
    );
    assert_eq!(
        handoff
            .pointer("/first_target/source_partition")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        handoff
            .pointer("/first_target/route")
            .and_then(Value::as_u64),
        Some(cpu1_window.route)
    );

    assert_json_stat_at_least(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu1.components",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(
        &json,
        "sim.host_actions.execution_mode_switch_state_transfer.target.cpu0.components",
    );
    super::scalar_load::assert_memory_resources(&json, "cache-fabric-dram");
}

fn cached_load_window(
    json: &Value,
    request_agent: u64,
    address: &str,
) -> super::multicore_scalar_load::DataWindow {
    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing translated multicore Data trace: {json}"));
    let mut completed_load_ticks = data_trace
        .iter()
        .filter(|record| {
            record.pointer("/cpu").and_then(Value::as_u64) == Some(request_agent)
                && record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some(address)
                && record.pointer("/size").and_then(Value::as_u64) == Some(4)
        })
        .map(|record| {
            record
                .pointer("/tick")
                .and_then(Value::as_u64)
                .expect("translated load completion tick")
        })
        .collect::<Vec<_>>();
    completed_load_ticks.sort_unstable();
    assert_eq!(
        completed_load_ticks.len(),
        2,
        "agent {request_agent} must complete exactly the warm and cached loads at {address}: {data_trace:?}"
    );
    let cached_response_tick = completed_load_ticks[1];
    let windows = super::multicore_scalar_load::data_windows(json, request_agent);
    let matching = windows
        .iter()
        .filter(|window| window.response_tick == cached_response_tick)
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "cached translated load for agent {request_agent} at {address} must map to one transport window: response_tick={cached_response_tick}, windows={windows:?}"
    );
    matching[0]
}

fn run_translated_multicore_scalar_load(path: &Path, switch_tick: Option<u64>) -> Value {
    let phase = if switch_tick.is_some() {
        "switch"
    } else {
        "baseline"
    };
    let workspace = temp_workspace(&format!(
        "translated-multicore-cpu1-cache-fabric-dram-{phase}"
    ));
    let config = workspace.join("run.toml");
    let host_switch = switch_tick
        .map(|tick| format!("host_execution_mode_switches = [\"{tick}:cpu1:timing\"]\n"))
        .unwrap_or_default();
    std::fs::write(
        &config,
        format!(
            r#"[run]
isa = "riscv"
binary = "{}"
max_tick = 4000
stats_format = "json"
execute = true
cores = 2
parallel_workers = 2
memory_system = "cache-fabric-dram"
memory_route_delay = 64
m5_switch_cpu_mode = "detailed"
riscv_o3_scalar_memory_depth = 4
debug_flags = ["Data", "Fetch", "Memory", "HostAction"]
memory_dumps = ["0x80000210:16"]
{host_switch}[run.riscv_data_translation]
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
"#,
            path.display()
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "translated multicore CPU1 switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn translated_multicore_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        csr_read(0xf14, 5),
        0,
        m5op(M5_SWITCH_CPU),
        u_type(VIRTUAL_PAGE as i32, 10, 0x37),
        i_type(DATA_OFFSET as i32, 10, 0b010, 11, 0x03),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(DATA_OFFSET as i32, 10, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        r_type(0, 13, 14, 0x0, 15, 0x33),
        s_type((DATA_OFFSET + 16) as i32, 12, 10, 0b010),
        s_type((DATA_OFFSET + 20) as i32, 13, 10, 0b010),
        s_type((DATA_OFFSET + 24) as i32, 14, 10, 0b010),
        s_type((DATA_OFFSET + 28) as i32, 15, 10, 0b010),
    ];
    append_host_stop(&mut words);

    let cpu0_path = words.len();
    words.extend([
        u_type(VIRTUAL_PAGE as i32, 20, 0x37),
        i_type((DATA_OFFSET + 4) as i32, 20, 0b010, 22, 0x03),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type((DATA_OFFSET + 4) as i32, 20, 0b010, 21, 0x03),
        b_type(0, 0, 0, 0x0),
    ]);
    words[1] = b_type(((cpu0_path - 1) * 4) as i32, 0, 5, 0x0);

    while words.len() * 4 < DATA_OFFSET as usize {
        words.push(0);
    }
    words.extend([42, 99, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(PHYSICAL_PAGE, PHYSICAL_PAGE, &program);
    temp_binary(name, &elf)
}
