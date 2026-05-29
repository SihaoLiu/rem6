use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn interrupt_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused interrupt modules, but it has {lines} lines"
    );
}

#[test]
fn interrupt_errors_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(
        error_rs.exists(),
        "interrupt error reporting belongs in src/error.rs"
    );
    assert!(
        !lib_rs.contains("pub enum InterruptError {"),
        "src/lib.rs should re-export interrupt errors from a focused module"
    );
}

#[test]
fn interrupt_routes_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let route_rs = crate_dir.join("src/route.rs");

    assert!(
        route_rs.exists(),
        "interrupt route identifiers and route records belong in src/route.rs"
    );
    assert!(
        !lib_rs.contains("pub struct InterruptLineId("),
        "src/lib.rs should re-export interrupt line identifiers from a focused route module"
    );
    assert!(
        !lib_rs.contains("pub struct InterruptRoute {"),
        "src/lib.rs should re-export interrupt routes from a focused route module"
    );
}

#[test]
fn interrupt_events_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let event_rs = crate_dir.join("src/event.rs");

    assert!(
        event_rs.exists(),
        "interrupt delivery, event, pending, and claim records belong in src/event.rs"
    );
    assert!(
        !lib_rs.contains("pub enum InterruptEventKind {"),
        "src/lib.rs should re-export interrupt event kinds from a focused event module"
    );
    assert!(
        !lib_rs.contains("pub struct InterruptDelivery {"),
        "src/lib.rs should re-export interrupt deliveries from a focused event module"
    );
    assert!(
        !lib_rs.contains("pub struct PendingInterrupt {"),
        "src/lib.rs should re-export pending interrupt records from a focused event module"
    );
}

#[test]
fn interrupt_snapshots_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let snapshot_rs = crate_dir.join("src/snapshot.rs");

    assert!(
        snapshot_rs.exists(),
        "interrupt controller snapshots belong in src/snapshot.rs"
    );
    assert!(
        !lib_rs.contains("pub struct InterruptSnapshot {"),
        "src/lib.rs should re-export interrupt snapshots from a focused snapshot module"
    );
}

#[test]
fn interrupt_source_files_stay_within_size_limit() {
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
