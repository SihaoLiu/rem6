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
fn virtio_9p_protocol_constants_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let protocol_rs = fs::read_to_string(crate_dir.join("src/fs9p_protocol.rs")).unwrap();

    for symbol in [
        "VIRTIO_9P_TVERSION",
        "VIRTIO_9P_RLERROR",
        "VIRTIO_9P_EBADF",
        "VIRTIO_9P_QTDIR",
        "VIRTIO_9P_SETATTR_MODE",
    ] {
        assert!(
            !device_rs.contains(&format!("pub const {symbol}:")),
            "9P wire constant {symbol} belongs in src/fs9p_protocol.rs"
        );
        assert!(
            protocol_rs.contains(&format!("pub const {symbol}:")),
            "src/fs9p_protocol.rs should define 9P wire constant {symbol}"
        );
    }
}

#[test]
fn virtio_9p_operation_handlers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let ops_rs = crate_dir.join("src/fs9p/ops.rs");

    assert!(
        ops_rs.exists(),
        "9P operation handlers belong in src/fs9p/ops.rs"
    );
    let ops_source = fs9p_ops_source(crate_dir);
    for symbol in [
        "fn handle_xattrwalk(",
        "fn handle_xattrcreate(",
        "fn handle_readdir(",
        "fn handle_fsync(",
        "fn handle_lock(",
        "fn handle_getlock(",
    ] {
        assert!(
            !device_rs.contains(symbol),
            "{symbol} should live outside src/fs9p.rs"
        );
        assert!(
            ops_source.contains(symbol),
            "{symbol} should live under src/fs9p/ops"
        );
    }
}

#[test]
fn virtio_9p_namespace_mutation_handlers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let ops_rs = crate_dir.join("src/fs9p/ops.rs");

    assert!(
        ops_rs.exists(),
        "9P namespace mutation handlers belong in src/fs9p/ops.rs"
    );
    let ops_source = fs9p_ops_source(crate_dir);
    for symbol in [
        "fn handle_mkdir(",
        "fn handle_link(",
        "fn handle_renameat(",
        "fn handle_rename(",
        "fn handle_unlinkat(",
    ] {
        assert!(
            !device_rs.contains(symbol),
            "{symbol} should live outside src/fs9p.rs"
        );
        assert!(
            ops_source.contains(symbol),
            "{symbol} should live under src/fs9p/ops"
        );
    }
}

#[test]
fn virtio_9p_io_lifecycle_handlers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let ops_rs = crate_dir.join("src/fs9p/ops.rs");

    assert!(
        ops_rs.exists(),
        "9P I/O lifecycle handlers belong in src/fs9p/ops.rs"
    );
    let ops_source = fs9p_ops_source(crate_dir);
    for symbol in [
        "fn handle_read(",
        "fn handle_write(",
        "fn handle_clunk(",
        "fn handle_remove(",
    ] {
        assert!(
            !device_rs.contains(symbol),
            "{symbol} should live outside src/fs9p.rs"
        );
        assert!(
            ops_source.contains(symbol),
            "{symbol} should live under src/fs9p/ops"
        );
    }
}

#[test]
fn virtio_9p_path_metadata_handlers_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let device_rs = fs::read_to_string(crate_dir.join("src/fs9p.rs")).unwrap();
    let ops_rs = crate_dir.join("src/fs9p/ops.rs");

    assert!(
        ops_rs.exists(),
        "9P path and metadata handlers belong in src/fs9p/ops.rs"
    );
    let ops_source = fs9p_ops_source(crate_dir);
    for symbol in [
        "fn handle_attach(",
        "fn handle_statfs(",
        "fn handle_walk(",
        "fn handle_lopen(",
        "fn handle_open(",
        "fn handle_lcreate(",
        "fn handle_create(",
        "fn handle_symlink(",
        "fn handle_mknod(",
        "fn handle_readlink(",
        "fn handle_getattr(",
        "fn handle_setattr(",
        "fn handle_stat(",
        "fn handle_wstat(",
    ] {
        assert!(
            !device_rs.contains(symbol),
            "{symbol} should live outside src/fs9p.rs"
        );
        assert!(
            ops_source.contains(symbol),
            "{symbol} should live under src/fs9p/ops"
        );
    }
}

#[test]
fn virtio_9p_operation_handlers_live_in_family_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ops_root = fs::read_to_string(crate_dir.join("src/fs9p/ops.rs")).unwrap();

    assert!(
        !ops_root.contains("fn handle_"),
        "src/fs9p/ops.rs should delegate 9P handlers to operation-family modules"
    );

    for (module, symbols) in [
        (
            "path",
            &[
                "fn handle_attach(",
                "fn handle_statfs(",
                "fn handle_walk(",
                "fn handle_lopen(",
                "fn handle_open(",
                "fn handle_lcreate(",
                "fn handle_create(",
                "fn handle_symlink(",
                "fn handle_mknod(",
                "fn handle_readlink(",
                "fn handle_getattr(",
                "fn handle_setattr(",
                "fn handle_stat(",
                "fn handle_wstat(",
            ][..],
        ),
        (
            "xattr",
            &["fn handle_xattrwalk(", "fn handle_xattrcreate("][..],
        ),
        (
            "io",
            &[
                "fn handle_read(",
                "fn handle_write(",
                "fn handle_clunk(",
                "fn handle_remove(",
                "fn handle_readdir(",
                "fn handle_fsync(",
            ][..],
        ),
        ("lock", &["fn handle_lock(", "fn handle_getlock("][..]),
        (
            "namespace",
            &[
                "fn handle_mkdir(",
                "fn handle_link(",
                "fn handle_renameat(",
                "fn handle_rename(",
                "fn handle_unlinkat(",
            ][..],
        ),
    ] {
        let path = crate_dir.join(format!("src/fs9p/ops/{module}.rs"));
        assert!(path.exists(), "9P {module} handlers belong in {path:?}");
        let source = fs::read_to_string(path).unwrap();
        let module_declaration = format!("mod {module};");
        assert!(
            ops_root
                .lines()
                .any(|line| line.trim() == module_declaration),
            "src/fs9p/ops.rs should declare the {module} operation module"
        );
        assert!(
            !ops_root
                .lines()
                .any(|line| line.trim().ends_with(&module_declaration)
                    && line.trim() != module_declaration),
            "9P {module} operation modules should stay private to the 9P device"
        );
        assert!(
            !source.contains("pub(crate) fn handle_"),
            "9P {module} handlers should not be visible across the VirtIO crate"
        );
        assert!(
            !source.contains("pub fn handle_"),
            "9P {module} handlers should not be public API"
        );
        for symbol in symbols {
            assert!(
                source.contains(&format!("pub(in crate::fs9p) {symbol}")),
                "{symbol} should live in the 9P {module} operation module with fs9p-scoped visibility"
            );
        }
    }
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

fn fs9p_ops_source(crate_dir: &Path) -> String {
    let mut source = String::new();
    for path in [
        "src/fs9p/ops.rs",
        "src/fs9p/ops/io.rs",
        "src/fs9p/ops/lock.rs",
        "src/fs9p/ops/namespace.rs",
        "src/fs9p/ops/path.rs",
        "src/fs9p/ops/xattr.rs",
    ] {
        let path = crate_dir.join(path);
        if path.exists() {
            source.push_str(&fs::read_to_string(path).unwrap());
            source.push('\n');
        }
    }
    source
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
