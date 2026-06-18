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
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 60\nexecute = true\nstats_format = \"json\"\ndebug_flags = [\"Exec\"]\n",
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
