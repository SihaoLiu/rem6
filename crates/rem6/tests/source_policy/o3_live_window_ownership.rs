use std::fs;
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn contents(relative: &str) -> String {
    fs::read_to_string(crate_root().join(relative)).unwrap()
}

fn line_count(relative: &str) -> usize {
    contents(relative).lines().count()
}

#[test]
fn o3_live_window_files_keep_focused_ownership_and_caps() {
    for (relative, maximum) in [
        ("src/config.rs", 1699),
        ("tests/cli_run/validation.rs", 3799),
        ("tests/cli_run/validation/o3_depths.rs", 900),
        ("tests/cli_run/m5_host_actions/o3/live_window_depth.rs", 900),
        (
            "tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs",
            350,
        ),
        (
            "tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs",
            700,
        ),
        (
            "tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs",
            500,
        ),
    ] {
        let lines = line_count(relative);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, max {maximum}"
        );
    }
    let timing = contents("src/config/riscv_timing.rs");
    for anchor in [
        "struct RiscvO3WindowDepths",
        "fn resolve_riscv_o3_window_depths",
        "MAX_RISCV_O3_SCALAR_MEMORY_DEPTH",
        "MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH",
    ] {
        assert!(timing.contains(anchor), "missing timing owner {anchor}");
    }
    let root = contents("src/config.rs");
    assert!(!root.contains("const MAX_RISCV_O3_SCALAR"));
    assert!(!root.contains("BelowMemoryDepth {"));
    let validation = contents("tests/cli_run/validation.rs");
    assert!(validation.contains("mod o3_depths;"));
    assert!(!validation.contains("fn rem6_run_accepts_max_riscv_o3_scalar_memory_depth"));
}

#[test]
fn o3_live_window_ledger_anchors_name_real_cli_tests() {
    let tests = [
        "tests/cli_run/m5_host_actions/o3/live_window_depth.rs",
        "tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs",
        "tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs",
    ]
    .into_iter()
    .map(contents)
    .collect::<Vec<_>>()
    .join("\n");
    for anchor in [
        "rem6_run_o3_deep_scalar_window_matrix",
        "rem6_run_o3_deep_scalar_window_text_stats",
        "rem6_run_o3_deep_scalar_window_dump_stats",
        "rem6_run_o3_scalar_live_depth_eight_combines_four_memory_and_four_scalar_rows",
        "rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row",
        "rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained",
        "rem6_run_host_switch_preserves_deep_scalar_window_timing",
        "rem6_run_timing_suppresses_deep_scalar_window_surfaces",
    ] {
        assert!(tests.contains(anchor), "missing compiled CLI test {anchor}");
    }
}
