use std::{fs, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_exec_debug_flag_emits_real_instruction_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "13831200",
            },
            ExpectedExecTraceRecord {
                tick: 6,
                pc: "0x80000008",
                bytes: "73000000",
            },
        ],
    );
}

#[test]
fn rem6_run_fetch_debug_flag_emits_real_fetch_issue_trace() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "60",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "Exec,Fetch",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![
            Value::String("Exec".to_string()),
            Value::String("Fetch".to_string())
        ])
    );
    assert_fetch_trace(
        &json,
        &[
            ExpectedFetchTraceRecord {
                tick: 0,
                pc: "0x80000000",
                sequence: 0,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 2,
                pc: "0x80000004",
                sequence: 1,
                size: 4,
            },
            ExpectedFetchTraceRecord {
                tick: 4,
                pc: "0x80000008",
                sequence: 2,
                size: 4,
            },
        ],
    );
    assert_eq!(
        json.pointer("/debug/exec_trace")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3)
    );
}

#[test]
fn rem6_run_fetch_debug_flag_keeps_fetches_across_riscv_se_stream_reset() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        0x0070_0293,                   // addi x5, x0, 7
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-fetch-riscv-se-reset", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--debug-flags",
            "Fetch",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Fetch".to_string())])
    );
    assert_fetch_pcs(
        &json,
        &[
            "0x80000000",
            "0x80000004",
            "0x80000008",
            "0x8000000c",
            "0x80000010",
            "0x80000014",
        ],
    );
}

#[test]
fn rem6_run_data_debug_flag_emits_real_data_access_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-data", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "Data",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Data".to_string())])
    );
    let trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("debug data trace array");
    assert_eq!(trace.len(), 2);
    assert_eq!(trace[0].get("kind").and_then(Value::as_str), Some("store"));
    assert_eq!(trace[1].get("kind").and_then(Value::as_str), Some("load"));
    for record in trace {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("address").and_then(Value::as_str),
            Some("0x80000040")
        );
        assert_eq!(record.get("size").and_then(Value::as_u64), Some(8));
        assert!(record.get("tick").and_then(Value::as_u64).is_some());
    }
}

#[test]
fn rem6_run_syscall_debug_flag_emits_real_riscv_se_syscall_trace() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-syscall", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--debug-flags",
            "Syscall",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Syscall".to_string())])
    );
    let trace = json
        .pointer("/debug/syscall_trace")
        .and_then(Value::as_array)
        .expect("debug syscall trace array");
    assert_eq!(trace.len(), 2);

    assert_eq!(trace[0].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[0].get("pc").and_then(Value::as_str),
        Some("0x80000004")
    );
    assert_eq!(trace[0].get("number").and_then(Value::as_u64), Some(172));
    assert_eq!(
        trace[0].pointer("/outcome/kind").and_then(Value::as_str),
        Some("return")
    );
    assert_eq!(
        trace[0].pointer("/outcome/value").and_then(Value::as_u64),
        Some(100)
    );

    assert_eq!(trace[1].get("cpu").and_then(Value::as_u64), Some(0));
    assert_eq!(
        trace[1].get("pc").and_then(Value::as_str),
        Some("0x80000010")
    );
    assert_eq!(trace[1].get("number").and_then(Value::as_u64), Some(93));
    assert_eq!(
        trace[1].pointer("/outcome/kind").and_then(Value::as_str),
        Some("exit")
    );
    assert_eq!(
        trace[1].pointer("/outcome/code").and_then(Value::as_i64),
        Some(0)
    );
}

#[test]
fn rem6_run_power_debug_flag_emits_activity_power_trace() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-power", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--debug-flags",
            "Power",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Power".to_string())])
    );
    let trace = json
        .pointer("/debug/power_trace")
        .and_then(Value::as_array)
        .expect("debug power trace array");
    for target in [
        "cpu0.core",
        "cpu.instruction_cache",
        "cpu.data_cache",
        "memory.transport",
        "memory.dram",
    ] {
        let record = trace
            .iter()
            .find(|record| record.get("target").and_then(Value::as_str) == Some(target))
            .unwrap_or_else(|| panic!("missing power trace target {target}: {trace:?}"));
        assert_eq!(record.get("state").and_then(Value::as_str), Some("on"));
        assert!(
            record
                .get("residency_ticks")
                .and_then(Value::as_u64)
                .is_some_and(|ticks| ticks > 0),
            "missing residency ticks for {target}: {record:?}"
        );
        assert!(
            record
                .get("dynamic_watts")
                .and_then(Value::as_f64)
                .is_some_and(|watts| watts > 0.0),
            "missing dynamic watts for {target}: {record:?}"
        );
    }
}

#[test]
fn rem6_run_loads_debug_flags_from_toml_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("debug-flags-config");
    let binary = workspace.join("kernel.elf");
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 60\nexecute = true\nmemory_system = \"direct\"\nstats_format = \"json\"\ndebug_flags = [\"Exec\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = stdout_json(output.stdout);
    assert_exec_trace(
        &json,
        &[
            ExpectedExecTraceRecord {
                tick: 2,
                pc: "0x80000000",
                bytes: "93027000",
            },
            ExpectedExecTraceRecord {
                tick: 4,
                pc: "0x80000004",
                bytes: "73000000",
            },
        ],
    );
}

#[test]
fn rem6_run_rejects_unknown_debug_flag() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-unknown", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,NoSuchFlag",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported debug flag NoSuchFlag"));
}

#[test]
fn rem6_run_rejects_empty_debug_flag_entries() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-empty", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--execute",
            "--debug-flags",
            "Exec,",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("empty debug flag entry"));
}

#[test]
fn rem6_run_rejects_debug_flags_without_execution() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-no-execute", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "json",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --execute"));
}

#[test]
fn rem6_run_rejects_exec_debug_flags_with_text_stats() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("debug-flags-text-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "text",
            "--execute",
            "--debug-flags",
            "Exec",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--debug-flags requires --stats-format json"));
}

struct ExpectedExecTraceRecord {
    tick: u64,
    pc: &'static str,
    bytes: &'static str,
}

struct ExpectedFetchTraceRecord {
    tick: u64,
    pc: &'static str,
    sequence: u64,
    size: u64,
}

fn stdout_json(stdout: Vec<u8>) -> Value {
    serde_json::from_slice(&stdout)
        .unwrap_or_else(|error| panic!("invalid JSON stdout: {error}; stdout={:?}", stdout))
}

fn assert_exec_trace(json: &Value, expected: &[ExpectedExecTraceRecord]) {
    assert_eq!(
        json.pointer("/debug/flags").and_then(Value::as_array),
        Some(&vec![Value::String("Exec".to_string())])
    );
    let trace = json
        .pointer("/debug/exec_trace")
        .and_then(Value::as_array)
        .expect("debug exec trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("bytes").and_then(Value::as_str),
            Some(expected.bytes)
        );
        assert_eq!(record.get("retired").and_then(Value::as_bool), Some(true));
    }
}

fn assert_fetch_trace(json: &Value, expected: &[ExpectedFetchTraceRecord]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    assert_eq!(trace.len(), expected.len());
    for (record, expected) in trace.iter().zip(expected) {
        assert_eq!(record.get("cpu").and_then(Value::as_u64), Some(0));
        assert_eq!(
            record.get("tick").and_then(Value::as_u64),
            Some(expected.tick)
        );
        assert_eq!(record.get("pc").and_then(Value::as_str), Some(expected.pc));
        assert_eq!(
            record.get("sequence").and_then(Value::as_u64),
            Some(expected.sequence)
        );
        assert_eq!(
            record.get("size").and_then(Value::as_u64),
            Some(expected.size)
        );
    }
}

fn assert_fetch_pcs(json: &Value, expected: &[&str]) {
    let trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("debug fetch trace array");
    let pcs = trace
        .iter()
        .map(|record| record.get("pc").and_then(Value::as_str).unwrap_or(""))
        .collect::<Vec<_>>();
    assert_eq!(pcs, expected);
}
