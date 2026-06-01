use std::fs;
use std::path::{Path, PathBuf};

use rem6_workload::WorkloadError;

const MAX_FACADE_LINES: usize = 1300;
const MAX_ERROR_DISPLAY_ROOT_LINES: usize = 1300;
const MAX_REPLAY_VERIFY_ROOT_LINES: usize = 1300;
const MAX_RESULT_ROOT_LINES: usize = 1300;
const MAX_WAIT_FOR_DIAGNOSTICS_LINES: usize = 1750;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_RESULT_ERROR_BYTES: usize = 128;

#[test]
fn workload_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade, but it has {lines} lines"
    );
}

#[test]
fn workload_result_root_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/result.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_RESULT_ROOT_LINES,
        "src/result.rs should delegate focused result facets, but it has {lines} lines"
    );
}

#[test]
fn workload_error_display_root_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/error/display.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_ERROR_DISPLAY_ROOT_LINES,
        "src/error/display.rs should delegate focused display groups, but it has {lines} lines"
    );
}

#[test]
fn workload_replay_verify_root_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/replay_verify.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_REPLAY_VERIFY_ROOT_LINES,
        "src/replay_verify.rs should delegate focused verifiers, but it has {lines} lines"
    );
}

#[test]
fn workload_wait_for_diagnostics_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/result/wait_for_diagnostics.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_WAIT_FOR_DIAGNOSTICS_LINES,
        "src/result/wait_for_diagnostics.rs should delegate focused raw-evidence audits, but it has {lines} lines"
    );
}

#[test]
fn wait_for_edge_kind_helpers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let diagnostics_rs =
        fs::read_to_string(crate_dir.join("src/result/wait_for_diagnostics.rs")).unwrap();
    let edge_kind_rs = crate_dir.join("src/result/wait_for_edge_kind_windows.rs");
    let edge_kind_rs_text = fs::read_to_string(&edge_kind_rs).unwrap_or_default();

    assert!(
        edge_kind_rs.exists(),
        "wait-for edge-kind helpers belong in src/result/wait_for_edge_kind_windows.rs"
    );
    for anchor in [
        "fn collect_wait_for_edge_kind_windows",
        "fn merge_wait_for_edge_kind_counts",
        "fn validate_wait_for_edge_kind_window_merge_summary",
        "fn merge_wait_for_edge_kind_counts_from_windows",
    ] {
        assert!(
            edge_kind_rs_text.contains(anchor),
            "src/result/wait_for_edge_kind_windows.rs should own {anchor}"
        );
    }
    assert!(
        !diagnostics_rs.contains("fn collect_wait_for_edge_kind_windows"),
        "src/result/wait_for_diagnostics.rs should delegate edge-kind window collection"
    );
    assert!(
        !diagnostics_rs.contains("fn merge_wait_for_edge_kind_counts"),
        "src/result/wait_for_diagnostics.rs should delegate edge-kind count merging"
    );
}

#[test]
fn batch_timeline_worker_tick_helpers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let timeline_rs = fs::read_to_string(crate_dir.join("src/result/batch_timeline.rs")).unwrap();
    let worker_ticks_rs = crate_dir.join("src/result/batch_timeline/worker_ticks.rs");
    let worker_ticks_rs_text = fs::read_to_string(&worker_ticks_rs).unwrap_or_default();

    assert!(
        worker_ticks_rs.exists(),
        "batch timeline worker tick helpers belong in src/result/batch_timeline/worker_ticks.rs"
    );
    for anchor in [
        "fn planned_batch_worker_slot_tick_summaries",
        "fn collect_batch_worker_slot_tick_summaries",
        "fn collect_strongest_batch_worker_count_tick_summaries",
        "fn batch_worker_tick_streak_at_or_above",
    ] {
        assert!(
            worker_ticks_rs_text.contains(anchor),
            "src/result/batch_timeline/worker_ticks.rs should own {anchor}"
        );
    }
    assert!(
        !timeline_rs.contains("fn planned_batch_worker_slot_tick_summaries"),
        "src/result/batch_timeline.rs should delegate planned worker slot summaries"
    );
    assert!(
        !timeline_rs.contains("fn collect_strongest_batch_worker_count_tick_summaries"),
        "src/result/batch_timeline.rs should delegate strongest worker count tick summaries"
    );
}

#[test]
fn progress_livelock_helpers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let progress_rs = fs::read_to_string(crate_dir.join("src/result/progress.rs")).unwrap();
    let livelock_rs = crate_dir.join("src/result/progress/livelock_diagnostics.rs");
    let livelock_rs_text = fs::read_to_string(&livelock_rs).unwrap_or_default();

    assert!(
        livelock_rs.exists(),
        "progress livelock helpers belong in src/result/progress/livelock_diagnostics.rs"
    );
    for anchor in [
        "fn collect_livelock_diagnostic_subjects",
        "fn livelock_diagnostic_tick_window",
        "fn collect_livelock_diagnostic_transition_kind_summaries",
        "fn collect_livelock_diagnostic_transition_kind_window_summaries",
    ] {
        assert!(
            livelock_rs_text.contains(anchor),
            "src/result/progress/livelock_diagnostics.rs should own {anchor}"
        );
    }
    assert!(
        !progress_rs.contains("fn collect_livelock_diagnostic_subjects"),
        "src/result/progress.rs should delegate livelock subject summaries"
    );
    assert!(
        !progress_rs.contains("fn collect_livelock_diagnostic_transition_kind_window_summaries"),
        "src/result/progress.rs should delegate livelock transition-kind window summaries"
    );
}

#[test]
fn workload_source_files_stay_within_size_limit() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut oversized = Vec::new();

    for path in rust_source_files(&src_dir) {
        let lines = line_count(&path);
        if lines > MAX_SOURCE_LINES {
            oversized.push(format!(
                "{} has {lines} lines",
                path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap()
                    .display()
            ));
        }
    }

    assert!(
        oversized.is_empty(),
        "source files exceed {MAX_SOURCE_LINES} lines: {}",
        oversized.join(", ")
    );
}

#[test]
fn workload_error_stays_within_result_error_size_budget() {
    assert!(std::mem::size_of::<WorkloadError>() <= MAX_RESULT_ERROR_BYTES);
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}
