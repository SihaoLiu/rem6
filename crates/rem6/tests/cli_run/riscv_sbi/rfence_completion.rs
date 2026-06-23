use std::process::Command;

use serde_json::Value;

use crate::support::*;

use super::{
    load_hsm_extension, load_rfence_extension, RISCV_SBI_ENTRY, SBI_HSM_HART_START,
    SBI_RFENCE_REMOTE_SFENCE_VMA,
};

#[test]
fn rem6_run_riscv_sbi_remote_sfence_vma_records_completion() {
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
    let secondary_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0x44, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let hsm_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.extend([
        i_type(1 << 1, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        u_type(0x4000, 12, 0x37),
        u_type(0x1000, 13, 0x37),
        load_rfence_extension(17)[0],
        load_rfence_extension(17)[1],
        i_type(SBI_RFENCE_REMOTE_SFENCE_VMA, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let rfence_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    for _ in 0..8 {
        words.push(i_type(0, 0, 0x0, 0, 0x13));
    }
    words.extend([i_type(0x35, 0, 0x0, 5, 0x13), 0x0010_0073]);
    let failure_index = words.len();
    words.extend([i_type(0x7e, 0, 0x0, 5, 0x13), 0x0010_0073]);
    words[hsm_error_branch_index] = b_type(
        ((failure_index - hsm_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );
    words[rfence_error_branch_index] = b_type(
        ((failure_index - rfence_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );

    let secondary_index = words.len();
    words.extend([i_type(1, 0, 0x0, 5, 0x13), j_type(0, 0)]);
    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-remote-sfence-vma-completion", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "640",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--riscv-sbi",
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
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/cores").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("breakpoint")
    );
    assert!(stdout.contains("\"x5\":\"0x35\""));
    let rfences = json
        .pointer("/riscv_sbi_rfences")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rfences.len(), 1);
    assert_eq!(
        rfences[0].get("source_cpu").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(rfences[0].get("function").and_then(Value::as_u64), Some(1));
    assert_eq!(
        rfences[0].get("hart_mask").and_then(Value::as_str),
        Some("0x2")
    );
    assert_eq!(
        rfences[0].get("hart_mask_base").and_then(Value::as_str),
        Some("0x0")
    );
    assert_eq!(
        rfences[0].get("start_addr").and_then(Value::as_str),
        Some("0x4000")
    );
    assert_eq!(
        rfences[0].get("size").and_then(Value::as_str),
        Some("0x1000")
    );
    assert!(rfences[0].get("address_space").unwrap().is_null());
    assert_eq!(
        rfences[0].get("targets").and_then(Value::as_array),
        Some(&vec![Value::from(1)])
    );

    let completions = json
        .pointer("/riscv_sbi_rfence_completions")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(completions.len(), 1);
    let completion = &completions[0];
    assert_eq!(
        completion.get("source_cpu").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        completion.get("target_hart").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(completion.get("function").and_then(Value::as_u64), Some(1));
    assert_eq!(
        completion.get("start_addr").and_then(Value::as_str),
        Some("0x4000")
    );
    assert_eq!(
        completion.get("size").and_then(Value::as_str),
        Some("0x1000")
    );
    assert!(completion.get("address_space").unwrap().is_null());
    assert!(completion
        .get("completed_tick")
        .and_then(Value::as_u64)
        .is_some());
    assert!(completion.get("flushed_entries").unwrap().is_null());
    assert_stat(
        &stdout,
        "sim.riscv.sbi.rfence.completions",
        "Count",
        1,
        "constant",
    );
}
