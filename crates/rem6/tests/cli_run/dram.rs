use std::process::Command;

use serde_json::Value;

use crate::support::{assert_stat, b_type, riscv64_elf, riscv64_program, temp_binary};

#[test]
fn rem6_run_applies_fine_granularity_dram_refresh_to_jedec_profile() {
    let program = riscv64_program(&[
        b_type(0, 0, 0, 0x0), // beq x0, x0, self
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-refresh-granularity-4x", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "2600",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "ddr4-2400-8gb",
            "--dram-refresh-granularity",
            "4x",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/dram/profile/timing/refresh_granularity")
            .and_then(Value::as_str),
        Some("4x")
    );
    assert_eq!(
        json_u64(&json, "/dram/profile/timing/refresh_interval"),
        2340
    );
    assert_eq!(
        json_u64(&json, "/dram/profile/timing/refresh_recovery"),
        105
    );
    assert!(json_u64(&json, "/dram/refreshes") > 0);
    assert_eq!(
        json_u64(&json, "/dram/refresh_ticks"),
        json_u64(&json, "/dram/refreshes") * 105
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_granularity.four_x",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_granularity.one_x",
        "Count",
        0,
        "constant",
    );
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 at {pointer}: {json}"))
}
