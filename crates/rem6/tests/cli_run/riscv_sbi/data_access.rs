use super::*;

#[test]
fn rem6_run_riscv_sbi_grants_supervisor_data_access() {
    let mut words = vec![
        u_type(0, 5, 0x17),
        i_type(0, 5, 0x0, 5, 0x13),
        i_type(0, 5, 0x3, 6, 0x03),
        i_type(0x5a, 0, 0x0, 7, 0x13),
        s_type(8, 7, 5, 0x3),
        i_type(8, 5, 0x3, 8, 0x03),
        i_type(0, 0, 0x0, 0, 0x13),
        0x0010_0073,
    ];
    let data_offset = (words.len() * 4) as i32;
    words[1] = i_type(data_offset, 5, 0x0, 5, 0x13);
    let mut program = riscv64_program(&words);
    program.extend_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &program);
    let path = temp_binary("riscv-sbi-supervisor-data-access", &elf);
    let dump = format!("0x{:x}:16", RISCV_SBI_ENTRY + data_offset as u64);

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
            "--riscv-sbi",
            "--dump-memory",
            &dump,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/simulation/status")
            .and_then(serde_json::Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x6")
            .and_then(serde_json::Value::as_str),
        Some("0x1122334455667788")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x8")
            .and_then(serde_json::Value::as_str),
        Some("0x5a")
    );
    assert_eq!(
        json.pointer("/memory/0/hex")
            .and_then(serde_json::Value::as_str),
        Some("88776655443322115a00000000000000")
    );
}
