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
