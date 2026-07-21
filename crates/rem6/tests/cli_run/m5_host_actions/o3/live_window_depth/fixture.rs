use super::*;

pub(super) const LOAD_PC: &str = "0x8000001c";
pub(super) const ROW_PCS: [&str; 7] = [
    "0x80000020",
    "0x80000024",
    "0x80000028",
    "0x8000002c",
    "0x80000030",
    "0x80000034",
    "0x80000038",
];
pub(super) const FINAL_MEMORY: &str = "09000000000000002a00000000000000";

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x33
}

pub(super) fn scalar_live_window_binary(name: &str, dump_stats: bool) -> std::path::PathBuf {
    let mut words = vec![
        i_type(2, 0, 0x0, 1, 0x13),
        i_type(3, 0, 0x0, 2, 0x13),
        i_type(4, 0, 0x0, 3, 0x13),
        i_type(5, 0, 0x0, 4, 0x13),
        i_type(7, 0, 0x0, 13, 0x13),
        u_type(0, 10, 0x17),
        i_type(76, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b011, 5, 0x03),
        r_type(0x01, 2, 1, 0x0, 6),
        r_type(0x01, 4, 3, 0x0, 7),
        i_type(1, 6, 0x0, 8, 0x13),
        r_type(0x00, 7, 6, 0x0, 9),
        i_type(1, 13, 0x0, 14, 0x13),
        r_type(0x00, 9, 8, 0x0, 16),
        r_type(0x00, 5, 16, 0x0, 17),
        s_type(8, 17, 10, 0b011),
    ];
    if dump_stats {
        words.extend([
            i_type(0, 0, 0x0, 10, 0x13),
            i_type(0, 0, 0x0, 11, 0x13),
            m5op(M5_DUMP_STATS),
        ]);
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < 96 {
        words.push(0);
    }
    words.extend([9, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn scalar_live_window_command(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
    mode: &str,
    stats_format: &str,
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
        stats_format,
        "--execute",
        "--riscv-execution-mode",
        mode,
        "--riscv-o3-scalar-memory-depth",
        "1",
        "--riscv-o3-scalar-live-window-depth",
        &live_depth.to_string(),
        "--riscv-o3-issue-width",
        &issue_width.to_string(),
        "--riscv-o3-writeback-width",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "80",
        "--dump-memory",
        "0x80000060:16",
    ]);
    if stats_format == "json" {
        command.args(["--debug-flags", "O3,Data,Fetch,Memory,HostAction"]);
    }
    command
}

pub(super) fn scalar_live_window_json(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
) -> Value {
    scalar_live_window_json_with_mode_and_args(
        path,
        memory_system,
        live_depth,
        issue_width,
        max_tick,
        "detailed",
        &[],
    )
}

pub(super) fn scalar_live_window_json_with_mode_and_args(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
    mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = scalar_live_window_command(
        path,
        memory_system,
        live_depth,
        issue_width,
        max_tick,
        mode,
        "json",
    );
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
