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
    assert!(
        output.stderr.is_empty(),
        "{memory_system} {execution_mode} lookahead={branch_lookahead} succeeded with stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid control-window JSON: {error}"))
}

pub(super) fn finish_control_window_binary(
    name: &str,
    mut words: Vec<u32>,
    data_start: usize,
    data_words: [u32; 4],
) -> std::path::PathBuf {
    let word_bytes = std::mem::size_of::<u32>();
    let code_bytes = words.len().checked_mul(word_bytes).unwrap_or_else(|| {
        panic!(
            "control-window binary `{name}` code byte length overflow for {} words",
            words.len()
        )
    });
    assert!(
        code_bytes <= data_start,
        "control-window binary `{name}` has {code_bytes} code bytes, exceeding data start {data_start:#x}"
    );
    assert_eq!(
        data_start % word_bytes,
        0,
        "control-window binary `{name}` data start {data_start:#x} must be word-aligned"
    );
    words.resize(data_start / word_bytes, 0);
    words.extend(data_words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
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

pub(super) fn assert_no_fetch_pc(json: &Value, pc: &str) {
    assert!(
        json.pointer("/debug/fetch_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records
                .iter()
                .all(|record| { record.pointer("/pc").and_then(Value::as_str) != Some(pc) })),
        "unexpected fetch at {pc}: {json}"
    );
}

pub(super) fn assert_stopped_by_host(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
}

pub(super) fn assert_branch_kind_and_link(event: &Value, kind: &str, link_write: bool) {
    assert_eq!(
        event.pointer("/branch_kind").and_then(Value::as_str),
        Some(kind),
        "unexpected branch kind: {event}"
    );
    assert_eq!(
        event
            .pointer("/branch_link_register_write")
            .and_then(Value::as_bool),
        Some(link_write),
        "unexpected branch link-write flag: {event}"
    );
}

pub(super) fn assert_ordered_commits<const N: usize>(events: [&Value; N]) {
    assert!(events
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
}

pub(super) fn assert_register_absent_or_zero(json: &Value, register: &str) {
    let registers = json
        .pointer("/cores/0/registers")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing register object: {json}"));
    match registers.get(register) {
        None => {}
        Some(value) if value.as_str() == Some("0x0") => {}
        Some(value) => {
            panic!("expected {register} to be absent or explicitly zero, got {value}: {json}")
        }
    }
}

pub(super) fn assert_integer_rename_maps_to_row_destination(
    json: &Value,
    row_pc: &str,
    register: u64,
) {
    let row = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(row_pc))
        })
        .unwrap_or_else(|| panic!("missing resident integer row {row_pc}: {json}"));
    let destination = row
        .pointer("/destination")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("integer row should own a destination: {row}"));
    let rename_entry = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64) == Some(register)
            })
        })
        .unwrap_or_else(|| panic!("missing live rename for x{register}: {json}"));
    assert_eq!(
        rename_entry.pointer("/physical").and_then(Value::as_u64),
        Some(destination),
        "x{register} should map to the destination owned by {row_pc}"
    );
}

pub(super) fn assert_pointer_u64_gt(json: &Value, pointer: &str, minimum: u64) {
    assert!(
        json.pointer(pointer)
            .and_then(Value::as_u64)
            .is_some_and(|value| value > minimum),
        "expected {pointer} > {minimum}: {json}"
    );
}

pub(super) fn assert_hierarchy_activity(json: &Value) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_pointer_u64_gt(json, pointer, 0);
    }
}

pub(super) fn assert_direct_memory_activity(json: &Value) {
    assert_pointer_u64_gt(json, "/memory_resources/transport/data/activity", 0);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "direct memory route should bypass hierarchy resource {pointer}: {json}"
        );
    }
}

pub(super) fn assert_final_execution_mode(json: &Value, expected_mode: &str) {
    let execution_modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final execution mode: {json}"));
    assert_eq!(execution_modes.len(), 1);
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some(expected_mode)
    );
}

pub(super) fn assert_drained_control_runtime(json: &Value) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

pub(super) fn assert_no_o3_stats(json: &Value) {
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing control-window stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                    "system.cpu.fetch.",
                    "system.cpu.bac.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode leaked control-window O3 stats: {unexpected:?}"
    );
}
