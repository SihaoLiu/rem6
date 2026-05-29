use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn uart_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused UART modules, but it has {lines} lines"
    );
}

#[test]
fn uart_events_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let event_rs = crate_dir.join("src/event.rs");

    assert!(
        event_rs.exists(),
        "UART byte records belong in src/event.rs"
    );
    assert!(
        !lib_rs.contains("pub struct UartTxByte {"),
        "src/lib.rs should re-export UART TX byte records from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct UartRxByte {"),
        "src/lib.rs should re-export UART RX byte records from a focused module"
    );
}

#[test]
fn uart_errors_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(
        error_rs.exists(),
        "UART error reporting belongs in src/error.rs"
    );
    assert!(
        !lib_rs.contains("pub enum UartError {"),
        "src/lib.rs should re-export UART errors from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum Pl011Error {"),
        "src/lib.rs should re-export PL011 errors from a focused module"
    );
}

#[test]
fn uart_snapshots_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let snapshot_rs = crate_dir.join("src/snapshot.rs");

    assert!(
        snapshot_rs.exists(),
        "UART snapshot records belong in src/snapshot.rs"
    );
    assert!(
        !lib_rs.contains("pub struct UartSnapshot {"),
        "src/lib.rs should re-export UART snapshots from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct Pl011UartSnapshot {"),
        "src/lib.rs should re-export PL011 snapshots from a focused module"
    );
}

#[test]
fn uart_source_files_stay_within_size_limit() {
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
