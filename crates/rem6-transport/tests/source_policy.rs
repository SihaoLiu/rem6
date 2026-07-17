use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn transport_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused transport modules, but it has {lines} lines"
    );
}

#[test]
fn memory_trace_exposes_a_poison_aware_snapshot() {
    let trace =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/trace.rs")).unwrap();

    assert!(trace.contains("pub fn try_snapshot("));
    assert!(trace.contains("MemoryTraceSnapshotError"));
}

#[test]
fn route_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let route_rs = crate_dir.join("src/route.rs");

    assert!(
        route_rs.exists(),
        "memory route contracts belong in src/route.rs"
    );
    assert!(
        !lib_rs.contains("pub struct MemoryRoute {"),
        "src/lib.rs should re-export memory route state from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum TransportError {"),
        "src/lib.rs should re-export transport errors from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum TopologyRouteError {"),
        "src/lib.rs should re-export topology route errors from a focused module"
    );
}

#[test]
fn message_buffer_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let message_buffer_rs = crate_dir.join("src/message_buffer.rs");

    assert!(
        message_buffer_rs.exists(),
        "transport message buffer contracts belong in src/message_buffer.rs"
    );
    assert!(
        !lib_rs.contains("pub struct TransportMessageBuffer<"),
        "src/lib.rs should re-export transport message buffers from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum TransportMessageBufferError {"),
        "src/lib.rs should re-export transport message buffer errors from a focused module"
    );
}

#[test]
fn fabric_qos_activity_contracts_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let qos_activity_rs = crate_dir.join("src/qos_activity.rs");
    let qos_activity = fs::read_to_string(&qos_activity_rs).unwrap();

    assert!(
        qos_activity_rs.exists(),
        "fabric QoS activity contracts belong in src/qos_activity.rs"
    );
    assert!(
        !lib_rs.contains("pub struct FabricQosGrantActivity {"),
        "src/lib.rs should re-export fabric QoS grant activity from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct FabricQosSuppressedRequest {"),
        "src/lib.rs should re-export suppressed fabric QoS requests from a focused module"
    );
    assert!(
        !lib_rs.contains("pub enum FabricQosSuppressionReason {"),
        "src/lib.rs should re-export fabric QoS suppression reasons from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct SharedFabricQosState {"),
        "src/lib.rs should re-export shared fabric QoS state from a focused module"
    );
    assert!(
        qos_activity.contains("pub enum FabricQosGrantDirection {"),
        "fabric QoS grant direction belongs in src/qos_activity.rs"
    );
    assert!(
        !lib_rs.contains("pub enum FabricQosGrantDirection {"),
        "src/lib.rs should re-export fabric QoS grant direction from a focused module"
    );
}

#[test]
fn response_qos_scheduling_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let response_qos_rs = crate_dir.join("src/response_qos.rs");

    assert!(
        response_qos_rs.exists(),
        "parallel response QoS scheduling belongs in src/response_qos.rs"
    );
    assert!(
        !lib_rs.contains("struct PendingResponseQosBatch {"),
        "src/lib.rs should delegate response QoS batch state to a focused module"
    );
    assert!(
        !lib_rs.contains("fn enqueue_parallel_response_qos_hop("),
        "src/lib.rs should delegate response QoS scheduling to a focused module"
    );
}

#[test]
fn transport_source_files_stay_within_size_limit() {
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
