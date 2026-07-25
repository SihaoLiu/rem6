use std::process::Command;

use serde_json::Value;

use super::*;

pub(super) const FP_ADD_PC: &str = "0x80000044";
pub(super) const VECTOR_RESULT_PC: &str = "0x80000048";
pub(super) const SECOND_FP_PC: &str = "0x8000004c";
pub(super) const DIV_PC: &str = "0x80000040";
pub(super) const LOAD_HEAD_PC: &str = "0x8000003c";
const DATA_ADDRESS: u64 = 0x8000_00c0;
const DATA_OFFSET: usize = 0xc0;
const EXPECTED_RESULTS: &str = "00004040090000000000c040";

pub(super) fn run_mixed_compute_json(
    issue_width: usize,
    memory_system: &str,
    switch_mode: &str,
    extra_args: &[&str],
) -> Value {
    let path = mixed_compute_binary(&format!(
        "o3-persistent-iq-mixed-compute-{memory_system}-width-{issue_width}"
    ));
    let issue_width = issue_width.to_string();
    let dump_memory = format!("0x{DATA_ADDRESS:x}:12");
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "2000",
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "1",
        "--riscv-o3-scalar-live-window-depth",
        "5",
        "--riscv-o3-issue-width",
        issue_width.as_str(),
        "--riscv-o3-writeback-width",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        switch_mode,
        "--dump-memory",
        dump_memory.as_str(),
    ]);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid mixed-compute stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/host_actions/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    json
}

pub(super) fn assert_exact_architectural_results(json: &Value) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(EXPECTED_RESULTS),
        "mixed-compute architectural results: {json}",
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x3")
            .and_then(Value::as_str),
        Some("0xc"),
        "mixed-compute DIV result: {json}",
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x15")
            .and_then(Value::as_str),
        Some("0x2a"),
        "mixed-compute batching load result: {json}",
    );
}

fn mixed_compute_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0x3f80_0000, 8, 0x37), // 1.0f bits
        fp_r_type(0x78, 0, 8, 0, 1),  // fmv.w.x f1, x8
        u_type(0x4000_0000, 8, 0x37), // 2.0f bits
        fp_r_type(0x78, 0, 8, 0, 2),  // fmv.w.x f2, x8
        u_type(0x4040_0000, 8, 0x37), // 3.0f bits
        fp_r_type(0x78, 0, 8, 0, 3),  // fmv.w.x f3, x8
        i_type(9, 0, 0, 9, 0x13),
        i_type(1, 0, 0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vmv_s_x_type(9, 3),
        i_type(84, 0, 0, 1, 0x13),
        i_type(7, 0, 0, 2, 0x13),
    ];
    let auipc_pc = i32::try_from(words.len() * 4).unwrap();
    words.extend([
        u_type(0, 12, 0x17),
        i_type(
            i32::try_from(DATA_OFFSET).unwrap() - auipc_pc,
            12,
            0,
            12,
            0x13,
        ),
        m5op(M5_SWITCH_CPU),
        i_type(12, 12, 0b010, 15, 0x03), // lw x15, 12(x12)
        r_type(1, 2, 1, 0x4, 3, 0x33),   // div x3, x1, x2
        fp_add_s(4, 1, 2),
        vmv_x_s_type(3, 11),
        fp_mul_s(5, 2, 3),
        fp_r_type(0x70, 0, 4, 0, 13),
        s_type(0, 13, 12, 0b010),
        s_type(4, 11, 12, 0b010),
        fp_r_type(0x70, 0, 5, 0, 14),
        s_type(8, 14, 12, 0b010),
        i_type(0, 0, 0, 10, 0x13),
        i_type(0, 0, 0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_host_stop(&mut words);
    assert!(words.len() * 4 <= DATA_OFFSET);
    while words.len() * 4 < DATA_OFFSET {
        words.push(0);
    }
    words.extend([0, 0, 0, 42]); // result words followed by the load input
    let program = riscv64_program(&words);
    temp_binary(name, &riscv64_elf(0x8000_0000, 0x8000_0000, &program))
}

fn vmv_s_x_type(rs1: u8, vd: u8) -> u32 {
    vector_arith_type(0b010000, 0b110, 0, rs1, vd)
}

fn vmv_x_s_type(vs2: u8, rd: u8) -> u32 {
    vector_arith_type(0b010000, 0b010, vs2, 0, rd)
}

fn fp_add_s(rd: u8, rs1: u8, rs2: u8) -> u32 {
    fp_r_type(0x00, rs2, rs1, 0, rd)
}

fn fp_mul_s(rd: u8, rs1: u8, rs2: u8) -> u32 {
    fp_r_type(0x08, rs2, rs1, 0, rd)
}
