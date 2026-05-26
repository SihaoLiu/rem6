use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_REPLAY_VERIFY_ROOT_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

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
fn workload_replay_verify_root_stays_within_module_budget() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/replay_verify.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_REPLAY_VERIFY_ROOT_LINES,
        "src/replay_verify.rs should delegate focused verifiers, but it has {lines} lines"
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
