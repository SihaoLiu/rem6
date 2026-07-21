use super::*;

const LOAD_PCS: [&str; 5] = [
    "0x80000008",
    "0x8000000c",
    "0x80000010",
    "0x80000014",
    "0x80000018",
];
const FIVE_LOAD_MEMORY: &str = concat!(
    "0100000000000000020000000000000003000000000000000400000000000000",
    "0500000000000000010000000000000002000000000000000300000000000000",
    "04000000000000000500000000000000"
);

fn five_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 10, 0x17),
        i_type(128, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b011, 5, 0x03),
        i_type(8, 10, 0b011, 6, 0x03),
        i_type(16, 10, 0b011, 7, 0x03),
        i_type(24, 10, 0b011, 8, 0x03),
        i_type(32, 10, 0b011, 9, 0x03),
        i_type(1, 9, 0x0, 11, 0x13),
        s_type(40, 5, 10, 0b011),
        s_type(48, 6, 10, 0b011),
        s_type(56, 7, 10, 0b011),
        s_type(64, 8, 10, 0b011),
        s_type(72, 9, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 128 {
        words.push(0);
    }
    words.extend([1, 0, 2, 0, 3, 0, 4, 0, 5, 0]);
    words.extend([0; 10]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn five_load_json(path: &Path, max_tick: u64) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
            "O3,Data,Fetch,Memory",
            "--riscv-execution-mode",
            "detailed",
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--riscv-o3-scalar-live-window-depth",
            "8",
            "--memory-system",
            "direct",
            "--memory-route-delay",
            "80",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
            "--dump-memory",
            "0x800000a0:16",
            "--dump-memory",
            "0x800000b0:16",
            "--dump-memory",
            "0x800000c0:16",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn dumped_memory_hex(json: &Value) -> String {
    json.pointer("/memory")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|entry| entry.pointer("/hex").and_then(Value::as_str).unwrap())
        .collect()
}

#[test]
fn rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row() {
    let path = five_load_binary("o3-live-eight-five-load-boundary");
    let completed = five_load_json(&path, 4_000);
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(dumped_memory_hex(&completed), FIVE_LOAD_MEMORY);
    for (register, value) in [
        ("x5", "0x1"),
        ("x6", "0x2"),
        ("x7", "0x3"),
        ("x8", "0x4"),
        ("x9", "0x5"),
        ("x11", "0x6"),
    ] {
        assert_eq!(
            completed
                .pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value)
        );
    }
    let first_response = LOAD_PCS[..4]
        .iter()
        .map(|pc| event_u64(event_at_pc(&completed, pc), "lsq_data_response_tick"))
        .min()
        .unwrap();
    let resident = five_load_json(&path, first_response - 1);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rob.len(), 4);
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert!(rob
        .iter()
        .all(|entry| { entry.pointer("/pc").and_then(Value::as_str) != Some(LOAD_PCS[4]) }));
    assert!(event_u64(event_at_pc(&completed, LOAD_PCS[4]), "issue_tick") >= first_response);
}
