use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_FOCUSED_DEVICE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn virtio_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused VirtIO modules, but it has {lines} lines"
    );
}

#[test]
fn virtio_errors_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(
        error_rs.exists(),
        "VirtIO error reporting belongs in src/error.rs"
    );
    assert!(
        !lib_rs.contains("pub enum VirtioError {"),
        "src/lib.rs should re-export VirtIO errors from a focused module"
    );
}

#[test]
fn virtio_queue_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let queue_rs = crate_dir.join("src/queue.rs");

    assert!(
        queue_rs.exists(),
        "VirtIO queue contracts belong in src/queue.rs"
    );
    assert!(
        !lib_rs.contains("pub struct VirtioQueueIndex("),
        "src/lib.rs should re-export queue indexes from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct VirtioQueueSpec {"),
        "src/lib.rs should re-export queue specs from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct VirtioQueueNotification {"),
        "src/lib.rs should re-export queue notifications from a focused module"
    );
}

#[test]
fn virtio_9p_device_source_remains_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fs9p.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FOCUSED_DEVICE_LINES,
        "src/fs9p.rs should delegate 9P parsing and namespace internals to focused modules, but it has {lines} lines"
    );
}

#[test]
fn virtio_9p_protocol_parsing_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let protocol_rs = crate_dir.join("src/fs9p_protocol.rs");

    assert!(
        protocol_rs.exists(),
        "9P payload parsing belongs in src/fs9p_protocol.rs"
    );
    assert!(
        !device_rs.contains("struct Virtio9pPayloadReader"),
        "src/fs9p.rs should delegate 9P payload reading to src/fs9p_protocol.rs"
    );
    assert!(
        !device_rs.contains("fn parse_"),
        "src/fs9p.rs should delegate typed 9P request parsing to src/fs9p_protocol.rs"
    );
}

#[test]
fn virtio_source_files_stay_within_size_limit() {
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
fn virtio_9p_device_tests_delegate_protocol_helpers() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_test = fs::read_to_string(crate_dir.join("tests/fs9p_device.rs")).unwrap();
    let support = crate_dir.join("tests/support/fs9p.rs");

    assert!(
        support.exists(),
        "9P integration test protocol helpers belong in tests/support/fs9p.rs"
    );
    assert!(
        !device_test.contains("\nfn p9_"),
        "tests/fs9p_device.rs should use shared 9P protocol helper constructors from tests/support/fs9p.rs"
    );
    assert!(
        !device_test.contains("\nfn decoded_request"),
        "tests/fs9p_device.rs should use the shared request decoder from tests/support/fs9p.rs"
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
