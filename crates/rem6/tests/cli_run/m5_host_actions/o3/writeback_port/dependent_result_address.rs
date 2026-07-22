use super::result_classes::unique_result_temp_binary;
use super::result_support::{
    assert_register, assert_resource_counter, data_trace, event_str, json_u64, memory_dump_hex,
    memory_result_event_at_pc,
};
use super::*;

#[path = "dependent_result_address/boundaries.rs"]
mod boundaries;

#[path = "dependent_result_address/two_pending.rs"]
mod two_pending;

const HEAD_PC: &str = "0x80000030";
const DEPENDENT_PC: &str = "0x80000034";
const SCALAR_PC: &str = "0x80000038";
const FAN_IN_PC: &str = "0x8000003c";
const WITNESS_PC: &str = "0x80000040";
const DATA_START: u64 = 0x8000_0100;
const POINTER: u64 = DATA_START + 64;
const WITNESS: u64 = DATA_START + 96;
const TARGET_ZERO_VALUE: u64 = 0x1111;
const TARGET_EIGHT_VALUE: u64 = 0x2222;
const SWAP_VALUE: u64 = 0x44;
const HEAD_GUARD: u64 = 0xaaaa_bbbb_cccc_dddd;
const WITNESS_GUARD: u64 = 0x5555_6666_7777_8888;
const OLD_REGISTERS: [(&str, u64); 4] = [("x5", 0x15), ("x6", 0x16), ("x7", 0x17), ("x8", 0x18)];

static OUTPUT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependentAddressHead {
    ScalarLoad,
    AtomicSwap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DependentAddressRow {
    head: DependentAddressHead,
    memory_system: &'static str,
    issue_width: usize,
    offset: i32,
    route_delay: u64,
    max_tick: u64,
}

const DEPENDENT_ADDRESS_ROWS: [DependentAddressRow; 4] = [
    DependentAddressRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        route_delay: 9,
        max_tick: 800,
    },
    DependentAddressRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        route_delay: 9,
        max_tick: 2_000,
    },
    DependentAddressRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        route_delay: 9,
        max_tick: 800,
    },
    DependentAddressRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        route_delay: 9,
        max_tick: 2_000,
    },
];

#[test]
fn rem6_run_o3_dependent_result_address_matrix_direct() {
    run_matrix("direct");
}

#[test]
fn rem6_run_o3_dependent_result_address_matrix_cache_fabric_dram() {
    run_matrix("cache-fabric-dram");
}

#[test]
fn rem6_run_timing_suppresses_o3_dependent_result_address() {
    let row = DEPENDENT_ADDRESS_ROWS[0];
    let fixture = DependentAddressFixture::new(row);
    let detailed = fixture.run(row.max_tick, "detailed", &[]);
    let timing = fixture.run(row.max_tick, "timing", &[]);
    assert_eq!(
        timing.pointer("/cores/0/registers"),
        detailed.pointer("/cores/0/registers")
    );
    assert_eq!(timing.pointer("/memory"), detailed.pointer("/memory"));
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let aliases = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing stats")
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
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(aliases.is_empty(), "timing O3 aliases: {aliases:?}");
}

fn run_matrix(memory_system: &str) {
    for row in DEPENDENT_ADDRESS_ROWS
        .into_iter()
        .filter(|row| row.memory_system == memory_system)
    {
        let fixture = DependentAddressFixture::new(row);
        let completed = fixture.run(row.max_tick, "detailed", &[]);
        let resident = assert_resident(&fixture, &completed);
        assert_completed(&fixture, &completed, &resident);
    }
}

struct DependentAddressFixture {
    row: DependentAddressRow,
    binary: std::path::PathBuf,
}

impl DependentAddressFixture {
    fn new(row: DependentAddressRow) -> Self {
        Self {
            row,
            binary: dependent_address_binary(row.head, row.offset),
        }
    }

    fn command(&self, max_tick: u64, switch_mode: &str) -> std::process::Command {
        let mut command = dependent_address_command(
            &self.binary,
            self.row.memory_system,
            self.row.issue_width,
            self.row.route_delay,
            max_tick,
            switch_mode,
        );
        add_memory_dumps(&mut command);
        command
    }

    fn run(&self, max_tick: u64, switch_mode: &str, extra_args: &[&str]) -> Value {
        let mut command = self.command(max_tick, switch_mode);
        command.args(extra_args);
        let json = run_json(
            command,
            &format!("{:?} {}", self.row.head, self.row.memory_system),
        );
        if max_tick == self.row.max_tick {
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_by_host"),
                "dependent-address run stopped at tick {:?}, pc {:?}",
                json.pointer("/simulation/final_tick"),
                json.pointer("/cores/0/pc"),
            );
        } else {
            assert_eq!(json_u64(&json, "/simulation/final_tick"), max_tick);
            assert_eq!(
                json.pointer("/simulation/status").and_then(Value::as_str),
                Some("stopped_at_tick_limit")
            );
        }
        json
    }
}

fn assert_resident(fixture: &DependentAddressFixture, completed: &Value) -> Value {
    let response_tick = event_u64(
        memory_result_event_at_pc(completed, HEAD_PC),
        "lsq_data_response_tick",
    );
    let resident = fixture.run(response_tick.saturating_sub(1), "detailed", &[]);
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"),
        4
    );
    assert_eq!(
        json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
        if fixture.row.head == DependentAddressHead::AtomicSwap {
            3
        } else {
            2
        }
    );
    let dependent_sequence = event_u64(rob_entry_at_pc(&resident, DEPENDENT_PC), "sequence");
    let pending_lsq = resident
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/sequence").and_then(Value::as_u64) == Some(dependent_sequence)
            })
        })
        .unwrap_or_else(|| panic!("pending LSQ row {dependent_sequence}: {resident}"));
    assert!(pending_lsq.pointer("/address").is_some_and(Value::is_null));
    assert_eq!(data_requests_sent(&resident).len(), 1);
    let head_destination = live_integer_mapping(&resident, 5);
    let dependent_destination = live_integer_mapping(&resident, 6);
    assert_ne!(head_destination, dependent_destination);
    for (register, value) in OLD_REGISTERS {
        assert_register(&resident, register, &format!("0x{value:x}"));
    }
    resident
}

fn assert_completed(fixture: &DependentAddressFixture, json: &Value, resident: &Value) {
    let row = fixture.row;
    let head = memory_result_event_at_pc(json, HEAD_PC);
    let dependent = memory_result_event_at_pc(json, DEPENDENT_PC);
    let scalar = event_at_pc(json, SCALAR_PC);
    let fan_in = event_at_pc(json, FAN_IN_PC);
    let witness = event_at_pc(json, WITNESS_PC);
    assert_eq!(
        event_u64(dependent, "issue_tick"),
        event_u64(head, "writeback_tick")
    );
    assert_eq!(
        event_u64(scalar, "issue_tick"),
        event_u64(dependent, "issue_tick") + u64::from(row.issue_width == 1)
    );
    let dependent_address = POINTER.wrapping_add_signed(i64::from(row.offset));
    let dependent_address_text = format!("0x{dependent_address:x}");
    assert_eq!(
        dependent
            .pointer("/lsq_load_address")
            .and_then(Value::as_str),
        Some(dependent_address_text.as_str())
    );
    let dependent_records = data_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "kind") == "load"
                && event_str(record, "address") == dependent_address_text
        })
        .collect::<Vec<_>>();
    assert_eq!(
        dependent_records.len(),
        1,
        "dependent data trace: {:?}",
        data_trace(json)
    );
    let requests = data_requests_sent(json);
    assert_eq!(
        requests.len(),
        3,
        "head, dependent, and witness requests: {requests:?}"
    );
    assert!(event_u64(requests[1], "tick") >= event_u64(dependent, "issue_tick"));
    assert!(event_u64(fan_in, "issue_tick") >= event_u64(dependent, "writeback_tick"));
    let sequences = [head, dependent, scalar, fan_in].map(|event| event_u64(event, "sequence"));
    assert!(sequences.windows(2).all(|window| window[0] < window[1]));
    let commits = [head, dependent, scalar, fan_in].map(|event| event_u64(event, "commit_tick"));
    assert!(
        commits.windows(2).all(|window| window[0] <= window[1]),
        "commits: {commits:?}"
    );
    assert!(event_u64(witness, "commit_tick") >= commits[3]);

    let dependent_value = if row.offset == 0 {
        TARGET_ZERO_VALUE
    } else {
        TARGET_EIGHT_VALUE
    };
    let scalar_value = POINTER + 8;
    let fan_in_value = dependent_value + scalar_value;
    for (register, value) in [
        ("x5", POINTER),
        ("x6", dependent_value),
        ("x7", scalar_value),
        ("x8", fan_in_value),
    ] {
        assert_register(json, register, &format!("0x{value:x}"));
    }
    assert_eq!(
        memory_dump_hex(json, DATA_START),
        Some(
            hex_u64_pair(
                if row.head == DependentAddressHead::AtomicSwap {
                    SWAP_VALUE
                } else {
                    POINTER
                },
                HEAD_GUARD
            )
            .as_str()
        )
    );
    assert_eq!(
        memory_dump_hex(json, POINTER),
        Some(hex_u64_pair(TARGET_ZERO_VALUE, TARGET_EIGHT_VALUE).as_str())
    );
    assert_eq!(
        memory_dump_hex(json, WITNESS),
        Some(hex_u64_pair(fan_in_value, WITNESS_GUARD).as_str())
    );
    assert_route_activity(json, row.memory_system);
    let issue = json
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing issue counters: {json}"));
    assert!(json_u64(issue, "/dependency_blocked_row_cycles") > 0);
    if row.issue_width == 1 {
        let resident_issue = resident
            .pointer("/cores/0/o3_runtime/issue")
            .expect("resident issue counters");
        assert_eq!(
            json_u64(issue, "/resource_blocked_row_cycles")
                - json_u64(resident_issue, "/resource_blocked_row_cycles"),
            1,
            "the dependent memory row wins one width-one issue slot"
        );
    }
}

fn live_integer_mapping(json: &Value, architectural: u64) -> u64 {
    json.pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64)
                        == Some(architectural)
            })
        })
        .and_then(|entry| entry.pointer("/physical").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing live x{architectural} mapping: {json}"))
}

fn assert_route_activity(json: &Value, memory_system: &str) {
    assert!(json_u64(json, "/memory_resources/transport/data/activity") > 0);
    if memory_system == "cache-fabric-dram" {
        for pointer in [
            "/memory_resources/cache/data/activity",
            "/memory_resources/fabric/activity",
            "/memory_resources/dram/activity",
        ] {
            assert!(json_u64(json, pointer) > 0, "{pointer}: {json}");
        }
    } else {
        for suffix in ["cache.data.activity", "fabric.activity", "dram.activity"] {
            assert_resource_counter(json, suffix, 0);
        }
    }
}

fn dependent_address_command(
    path: &std::path::Path,
    memory_system: &str,
    issue_width: usize,
    route_delay: u64,
    max_tick: u64,
    switch_mode: &str,
) -> std::process::Command {
    let config = WritebackRunConfig::detailed_json(memory_system, 2, route_delay, max_tick)
        .with_switch_mode(switch_mode);
    let mut command = writeback_command(path, config);
    command.args([
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--riscv-branch-lookahead",
        "3",
        "--riscv-o3-issue-width",
        &issue_width.to_string(),
        "--host-event-delay",
        "1",
    ]);
    command
}

fn add_memory_dumps(command: &mut std::process::Command) {
    for address in [DATA_START, POINTER, WITNESS] {
        command.args(["--dump-memory", &format!("0x{address:x}:16")]);
    }
}

fn run_json(mut command: std::process::Command, label: &str) -> Value {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let output =
        crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30));
    assert!(
        output.status.success(),
        "{label} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{label} invalid stdout JSON: {error}"))
}

fn data_requests_sent(json: &Value) -> Vec<&Value> {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("memory trace")
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
        })
        .collect()
}

fn wait_for_boundary(mut command: std::process::Command) -> std::process::Output {
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30))
}

fn unique_output(label: &str) -> std::path::PathBuf {
    let id = OUTPUT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "rem6-o3-dependent-address-{}-{}-{id}.json",
        label.replace(' ', "-"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

fn lsq_entries(json: &Value) -> &[Value] {
    json.pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("LSQ entries")
}

fn addressless_sequences(json: &Value) -> Vec<u64> {
    lsq_entries(json)
        .iter()
        .filter(|entry| entry.pointer("/address").is_some_and(Value::is_null))
        .map(|entry| event_u64(entry, "sequence"))
        .collect()
}

fn lsq_address(json: &Value, sequence: u64) -> Option<u64> {
    lsq_entries(json)
        .iter()
        .find(|entry| event_u64(entry, "sequence") == sequence)
        .and_then(|entry| entry.pointer("/address").and_then(Value::as_str))
        .map(|address| u64::from_str_radix(address.trim_start_matches("0x"), 16).unwrap())
}

fn rob_entries(json: &Value) -> &[Value] {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .expect("ROB entries")
}

fn rob_has_sequence(json: &Value, sequence: u64) -> bool {
    rob_entries(json)
        .iter()
        .any(|entry| entry.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
}

fn rename_has_integer(json: &Value, architectural: u64) -> bool {
    json.pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64)
                        == Some(architectural)
            })
        })
}

fn load_count(json: &Value, address: u64) -> usize {
    let address = format!("0x{address:x}");
    data_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "kind") == "load" && event_str(record, "address") == address
        })
        .count()
}

fn completed_load_bytes(json: &Value, address: u64) -> u64 {
    let address = format!("0x{address:x}");
    data_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "kind") == "load" && event_str(record, "address") == address
        })
        .map(|record| event_u64(record, "size"))
        .sum()
}

fn dependent_address_binary(head: DependentAddressHead, offset: i32) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 9, 0x17),
        i_type(DATA_START as i32 - 0x8000_0000_u64 as i32, 9, 0, 9, 0x13),
        i_type(OLD_REGISTERS[0].1 as i32, 0, 0, 5, 0x13),
        i_type(OLD_REGISTERS[1].1 as i32, 0, 0, 6, 0x13),
        i_type(OLD_REGISTERS[2].1 as i32, 0, 0, 7, 0x13),
        i_type(OLD_REGISTERS[3].1 as i32, 0, 0, 8, 0x13),
        i_type(SWAP_VALUE as i32, 0, 0, 11, 0x13),
    ];
    while words.len() < 11 {
        words.push(i_type(0, 0, 0, 0, 0x13));
    }
    words.push(m5op(M5_SWITCH_CPU));
    words.extend([
        match head {
            DependentAddressHead::ScalarLoad => i_type(0, 9, 0b011, 5, 0x03),
            DependentAddressHead::AtomicSwap => atomic_type(0x01, false, false, 11, 9, 0b011, 5),
        },
        i_type(offset, 5, 0b011, 6, 0x03),
        i_type(8, 5, 0, 7, 0x13),
        r_type(0, 7, 6, 0, 8, 0x33),
        s_type((WITNESS - DATA_START) as i32, 8, 9, 0b011),
    ]);
    words.extend(std::iter::repeat_n(i_type(0, 0, 0, 0, 0x13), 12));
    append_host_stop(&mut words);
    while words.len() * 4 < 0x100 {
        words.push(0);
    }
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&initial_memory(POINTER));
    unique_result_temp_binary(
        &format!("o3-dependent-result-address-{}-{offset}", head.label()),
        &riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
}

fn initial_memory(pointer: u64) -> Vec<u8> {
    let mut data = vec![0; 112];
    write_u64(&mut data, 0, pointer);
    write_u64(&mut data, 8, HEAD_GUARD);
    write_u64(&mut data, 64, TARGET_ZERO_VALUE);
    write_u64(&mut data, 72, TARGET_EIGHT_VALUE);
    write_u64(&mut data, 96, 0x9999);
    write_u64(&mut data, 104, WITNESS_GUARD);
    data
}

fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn hex_u64_pair(first: u64, second: u64) -> String {
    [first, second]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl DependentAddressHead {
    fn label(self) -> &'static str {
        match self {
            Self::ScalarLoad => "scalar-load",
            Self::AtomicSwap => "atomic-swap",
        }
    }
}
