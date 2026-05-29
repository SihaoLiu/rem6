use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn gpu_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused GPU modules, but it has {lines} lines"
    );
}

#[test]
fn gpu_command_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let command_rs = crate_dir.join("src/command.rs");

    assert!(
        command_rs.exists(),
        "GPU command contracts belong in src/command.rs"
    );
    assert!(
        !lib_rs.contains("pub struct GpuKernelLaunch {"),
        "src/lib.rs should re-export kernel launch state from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct GpuComputeConfig {"),
        "src/lib.rs should re-export compute config from a focused module"
    );
}

#[test]
fn gpu_errors_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(
        error_rs.exists(),
        "GPU error reporting belongs in src/error.rs"
    );
    assert!(
        !lib_rs.contains("pub enum GpuError {"),
        "src/lib.rs should re-export GPU errors from a focused module"
    );
}

#[test]
fn gpu_trace_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let trace_rs = crate_dir.join("src/trace.rs");

    assert!(
        trace_rs.exists(),
        "GPU trace contracts belong in src/trace.rs"
    );
    assert!(
        !lib_rs.contains("pub struct GpuTraceEvent {"),
        "src/lib.rs should re-export GPU trace events from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum GpuTraceKind {"),
        "src/lib.rs should re-export GPU trace kinds from a focused module"
    );
}

#[test]
fn gpu_snapshot_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let snapshot_rs = crate_dir.join("src/snapshot.rs");

    assert!(
        snapshot_rs.exists(),
        "GPU snapshot contracts belong in src/snapshot.rs"
    );
    assert!(
        !lib_rs.contains("pub struct GpuDeviceSnapshot {"),
        "src/lib.rs should re-export GPU device snapshots from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct GpuSlotSnapshot {"),
        "src/lib.rs should re-export GPU slot snapshots from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct GpuQueuedWorkgroupSnapshot {"),
        "src/lib.rs should re-export GPU queued workgroup snapshots from a focused module"
    );
}

#[test]
fn gpu_source_files_stay_within_size_limit() {
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
