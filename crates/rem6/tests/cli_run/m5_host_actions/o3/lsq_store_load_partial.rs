use std::collections::BTreeSet;

use super::lsq_store_load::{data_memory_request_count, event_at_pc, event_u64};
use super::*;

const OLDER_STORE_PC: &str = "0x80000010";
const YOUNGER_LOAD_PC: &str = "0x80000014";
const DATA_ADDRESS: &str = "0x80000100";
const LOAD_RESULT_ADDRESS: &str = "0x80000108";
const DEPENDENT_RESULT_ADDRESS: &str = "0x80000110";
const STORED_WORD_HEX: &str = "00807f00";

#[derive(Clone, Copy, Debug)]
struct ContainedLoadCase {
    name: &'static str,
    offset: i32,
    funct3: u32,
    size: u64,
    expected: u64,
}

const CONTAINED_LOAD_CASES: [ContainedLoadCase; 4] = [
    ContainedLoadCase {
        name: "lb-offset-1",
        offset: 1,
        funct3: 0b000,
        size: 1,
        expected: 0xffff_ffff_ffff_ff80,
    },
    ContainedLoadCase {
        name: "lbu-offset-1",
        offset: 1,
        funct3: 0b100,
        size: 1,
        expected: 0x80,
    },
    ContainedLoadCase {
        name: "lh-offset-0",
        offset: 0,
        funct3: 0b001,
        size: 2,
        expected: 0xffff_ffff_ffff_8000,
    },
    ContainedLoadCase {
        name: "lhu-offset-2",
        offset: 2,
        funct3: 0b101,
        size: 2,
        expected: 0x7f,
    },
];

#[derive(Clone, Copy, Debug)]
struct PartialStoreLoadCase {
    name: &'static str,
    store_offset: i32,
    store_funct3: u32,
    store_size: u64,
    load_offset: i32,
    load_funct3: u32,
    load_size: u64,
    forwarded_bytes: u64,
    expected: u64,
    expected_data_hex: &'static str,
}

const PARTIAL_STORE_LOAD_CASES: [PartialStoreLoadCase; 4] = [
    PartialStoreLoadCase {
        name: "byte-to-word",
        store_offset: 1,
        store_funct3: 0b000,
        store_size: 1,
        load_offset: 0,
        load_funct3: 0b010,
        load_size: 4,
        forwarded_bytes: 1,
        expected: 0xffff_ffff_8033_5a11,
        expected_data_hex: "115a338055667700",
    },
    PartialStoreLoadCase {
        name: "half-to-word",
        store_offset: 2,
        store_funct3: 0b001,
        store_size: 2,
        load_offset: 0,
        load_funct3: 0b010,
        load_size: 4,
        forwarded_bytes: 2,
        expected: 0x065a_2211,
        expected_data_hex: "11225a0655667700",
    },
    PartialStoreLoadCase {
        name: "word-to-doubleword",
        store_offset: 0,
        store_funct3: 0b010,
        store_size: 4,
        load_offset: 0,
        load_funct3: 0b011,
        load_size: 8,
        forwarded_bytes: 4,
        expected: 0x0077_6655_0000_065a,
        expected_data_hex: "5a06000055667700",
    },
    PartialStoreLoadCase {
        name: "byte-to-word-unsigned",
        store_offset: 1,
        store_funct3: 0b000,
        store_size: 1,
        load_offset: 0,
        load_funct3: 0b110,
        load_size: 4,
        forwarded_bytes: 1,
        expected: 0x8033_5a11,
        expected_data_hex: "115a338055667700",
    },
];

#[test]
fn rem6_run_o3_detailed_contained_store_load_forwarding_direct_matrix() {
    for case in CONTAINED_LOAD_CASES {
        let path = contained_store_load_binary(
            &format!("o3-contained-store-load-direct-{}", case.name),
            case.offset,
            case.funct3,
        );
        let json = contained_store_load_json(&path, "direct", 1100, None, 4);

        assert_eq!(
            json.pointer("/simulation/memory_system")
                .and_then(Value::as_str),
            Some("direct")
        );
        assert_contained_forwarding(&json, case);
    }
}

#[test]
fn rem6_run_o3_detailed_contained_store_load_forwarding_cache_fabric_dram_matrix() {
    for case in CONTAINED_LOAD_CASES {
        let path = contained_store_load_binary(
            &format!("o3-contained-store-load-hierarchy-{}", case.name),
            case.offset,
            case.funct3,
        );
        let json = contained_store_load_json(&path, "cache-fabric-dram", 1800, None, 4);

        assert_eq!(
            json.pointer("/simulation/memory_system")
                .and_then(Value::as_str),
            Some("cache-fabric-dram")
        );
        assert_contained_forwarding(&json, case);
        assert_hierarchy_activity(&json);
    }
}

#[test]
fn rem6_run_o3_timing_contained_store_load_uses_transport_without_o3_window() {
    let case = CONTAINED_LOAD_CASES[0];
    let path =
        contained_store_load_binary("o3-contained-store-load-timing", case.offset, case.funct3);
    let json = contained_store_load_json(&path, "direct", 1100, Some("timing"), 4);

    assert_final_architecture(&json, case.expected);
    assert_eq!(data_memory_request_count(&json), 4);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_data_trace(&json, case, 4);
}

#[test]
fn rem6_run_o3_detailed_partial_store_load_merge_direct_matrix() {
    for case in PARTIAL_STORE_LOAD_CASES {
        let path =
            partial_store_load_binary(&format!("o3-partial-store-load-direct-{}", case.name), case);
        let json = contained_store_load_json(&path, "direct", 1100, None, 8);

        assert_eq!(
            json.pointer("/simulation/memory_system")
                .and_then(Value::as_str),
            Some("direct")
        );
        assert_partial_forwarding(&json, case);
    }
}

#[test]
fn rem6_run_o3_detailed_partial_store_load_merge_cache_fabric_dram_matrix() {
    for case in [PARTIAL_STORE_LOAD_CASES[1], PARTIAL_STORE_LOAD_CASES[2]] {
        let path = partial_store_load_binary(
            &format!("o3-partial-store-load-hierarchy-{}", case.name),
            case,
        );
        let json = contained_store_load_json(&path, "cache-fabric-dram", 1800, None, 8);

        assert_eq!(
            json.pointer("/simulation/memory_system")
                .and_then(Value::as_str),
            Some("cache-fabric-dram")
        );
        assert_partial_forwarding(&json, case);
        assert_hierarchy_activity(&json);
    }
}

#[test]
fn rem6_run_o3_timing_partial_store_load_uses_transport_without_o3_merge() {
    let case = PARTIAL_STORE_LOAD_CASES[0];
    let path = partial_store_load_binary("o3-partial-store-load-timing", case);
    let json = contained_store_load_json(&path, "direct", 1100, Some("timing"), 8);

    assert_partial_final_architecture(&json, case);
    assert_eq!(data_memory_request_count(&json), 4);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_partial_data_trace(&json, case);
}

fn assert_contained_forwarding(json: &Value, case: ContainedLoadCase) {
    assert_final_architecture(json, case.expected);
    let older = event_at_pc(json, OLDER_STORE_PC);
    let younger = event_at_pc(json, YOUNGER_LOAD_PC);
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "contained load should issue before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(younger, "lsq_data_response_tick")
            < event_u64(older, "lsq_data_response_tick"),
        "contained load should complete locally before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "contained pair must retire in program order: older={older}, younger={younger}"
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_candidate")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_match")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_partial")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(event_u64(younger, "store_load_forwarding_bytes"), case.size);
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    for (path, value) in [
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_candidates", 1),
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_matches", 1),
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_suppressed", 0),
        (
            "sim.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
            0,
        ),
    ] {
        assert_json_stat(json, path, "Count", value, "monotonic");
    }
    assert_eq!(data_memory_request_count(json), 3);
    assert_data_trace(json, case, 4);
}

fn assert_partial_forwarding(json: &Value, case: PartialStoreLoadCase) {
    assert_partial_final_architecture(json, case);
    let older = event_at_pc(json, OLDER_STORE_PC);
    let younger = event_at_pc(json, YOUNGER_LOAD_PC);
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "partial load should issue before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "partial pair must retire in program order: older={older}, younger={younger}"
    );
    for field in [
        "store_load_forwarding_candidate",
        "store_load_forwarding_match",
        "store_load_forwarding_partial",
    ] {
        let pointer = format!("/{field}");
        assert_eq!(
            younger.pointer(&pointer).and_then(Value::as_bool),
            Some(true),
            "partial forwarding event field {field}: {younger}"
        );
    }
    assert_eq!(
        event_u64(younger, "store_load_forwarding_bytes"),
        case.forwarded_bytes
    );
    for field in [
        "store_load_forwarding_suppressed",
        "store_load_forwarding_address_mismatch",
        "store_load_forwarding_byte_mismatch",
    ] {
        let pointer = format!("/{field}");
        assert_eq!(
            younger.pointer(&pointer).and_then(Value::as_bool),
            Some(false),
            "partial forwarding must not report {field}: {younger}"
        );
    }
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    for (path, value) in [
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_candidates", 1),
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_matches", 1),
        ("sim.cpu0.o3.lsq_store_to_load_forwarding_suppressed", 0),
        (
            "sim.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
            0,
        ),
    ] {
        assert_json_stat(json, path, "Count", value, "monotonic");
    }
    assert_eq!(data_memory_request_count(json), 4);
    assert_partial_data_trace(json, case);
}

fn assert_final_architecture(json: &Value, expected: u64) {
    let dependent = expected.wrapping_add(1);
    let expected_register = format!("{expected:#x}");
    let dependent_register = format!("{dependent:#x}");
    let expected_memory = little_endian_hex(expected);
    let dependent_memory = little_endian_hex(dependent);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(STORED_WORD_HEX)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(expected_memory.as_str())
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some(dependent_memory.as_str())
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some(expected_register.as_str())
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        Some(dependent_register.as_str())
    );
}

fn assert_partial_final_architecture(json: &Value, case: PartialStoreLoadCase) {
    let dependent = case.expected.wrapping_add(1);
    let expected_register = format!("{:#x}", case.expected);
    let dependent_register = format!("{dependent:#x}");
    let expected_memory = little_endian_hex(case.expected);
    let dependent_memory = little_endian_hex(dependent);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.expected_data_hex)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(expected_memory.as_str())
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some(dependent_memory.as_str())
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some(expected_register.as_str())
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        Some(dependent_register.as_str())
    );
}

fn assert_data_trace(json: &Value, case: ContainedLoadCase, expected_len: usize) {
    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("contained store-load run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), expected_len);
    let observed = data_trace
        .iter()
        .map(|record| {
            (
                record
                    .pointer("/kind")
                    .and_then(Value::as_str)
                    .expect("data trace kind")
                    .to_owned(),
                record
                    .pointer("/address")
                    .and_then(Value::as_str)
                    .expect("data trace address")
                    .to_owned(),
                record
                    .pointer("/size")
                    .and_then(Value::as_u64)
                    .expect("data trace size"),
            )
        })
        .collect::<BTreeSet<_>>();
    let load_address = format!("0x{:x}", 0x8000_0100_u64 + case.offset as u64);
    assert_eq!(
        observed,
        BTreeSet::from([
            ("load".to_owned(), load_address, case.size),
            ("store".to_owned(), DATA_ADDRESS.to_owned(), 4),
            ("store".to_owned(), LOAD_RESULT_ADDRESS.to_owned(), 8),
            ("store".to_owned(), DEPENDENT_RESULT_ADDRESS.to_owned(), 8,),
        ])
    );
}

fn assert_partial_data_trace(json: &Value, case: PartialStoreLoadCase) {
    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("partial store-load run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), 4);
    let observed = data_trace
        .iter()
        .map(|record| {
            (
                record
                    .pointer("/kind")
                    .and_then(Value::as_str)
                    .expect("data trace kind")
                    .to_owned(),
                record
                    .pointer("/address")
                    .and_then(Value::as_str)
                    .expect("data trace address")
                    .to_owned(),
                record
                    .pointer("/size")
                    .and_then(Value::as_u64)
                    .expect("data trace size"),
            )
        })
        .collect::<BTreeSet<_>>();
    let store_address = format!("0x{:x}", 0x8000_0100_u64 + case.store_offset as u64);
    let load_address = format!("0x{:x}", 0x8000_0100_u64 + case.load_offset as u64);
    assert_eq!(
        observed,
        BTreeSet::from([
            ("load".to_owned(), load_address, case.load_size),
            ("store".to_owned(), store_address, case.store_size),
            ("store".to_owned(), LOAD_RESULT_ADDRESS.to_owned(), 8),
            ("store".to_owned(), DEPENDENT_RESULT_ADDRESS.to_owned(), 8),
        ])
    );
}

fn assert_hierarchy_activity(json: &Value) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed contained forwarding should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
    }
}

fn contained_store_load_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
    data_dump_bytes: u64,
) -> Value {
    let data_dump = format!("0x80000100:{data_dump_bytes}");
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
        "O3,Data,Memory",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        &data_dump,
        "--dump-memory",
        "0x80000108:8",
        "--dump-memory",
        "0x80000110:8",
    ]);
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn contained_store_load_binary(
    name: &str,
    load_offset: i32,
    load_funct3: u32,
) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        u_type(0x007f_8000, 11, 0x37),
        s_type(0, 11, 10, 0b010),
        i_type(load_offset, 10, load_funct3, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(8, 12, 10, 0b011),
        s_type(16, 13, 10, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0x63, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn partial_store_load_binary(name: &str, case: PartialStoreLoadCase) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x65a, 0, 0x0, 11, 0x13),
        s_type(case.store_offset, 11, 10, case.store_funct3),
        i_type(case.load_offset, 10, case.load_funct3, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(8, 12, 10, 0b011),
        s_type(16, 13, 10, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x8033_2211, 0x0077_6655, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn little_endian_hex(value: u64) -> String {
    value
        .to_le_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}
