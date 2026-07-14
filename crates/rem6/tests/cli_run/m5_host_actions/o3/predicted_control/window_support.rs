use super::*;

pub(super) fn control_window_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    data_address: &str,
    dump_bytes: u64,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-branch-lookahead",
        &branch_lookahead.to_string(),
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        &format!("{data_address}:{dump_bytes}"),
    ]);
    command
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_control_window_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    data_address: &str,
    dump_bytes: u64,
    extra_args: &[&str],
) -> Value {
    let mut command = control_window_command(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        data_address,
        dump_bytes,
    );
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode} lookahead={branch_lookahead}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid control-window JSON: {error}"))
}

pub(super) fn resident_rob_pcs(json: &Value) -> Vec<&str> {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing resident control-window ROB: {json}"))
        .iter()
        .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
        .collect()
}

pub(super) fn assert_no_data_address(json: &Value, address: &str) {
    for pointer in ["/debug/data_trace", "/debug/memory_trace"] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_array)
                .is_some_and(|records| records.iter().all(|record| {
                    record.pointer("/address").and_then(Value::as_str) != Some(address)
                })),
            "unexpected data access at {address}: {json}"
        );
    }
}
