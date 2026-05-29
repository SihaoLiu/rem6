use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn coherence_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused coherence modules, but it has {lines} lines"
    );
}

#[test]
fn coherence_error_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(
        error_rs.exists(),
        "coherence harness error code belongs in src/error.rs"
    );
    assert!(
        !lib_rs.contains("pub enum HarnessError {"),
        "src/lib.rs should re-export harness error types from a focused module"
    );
}

#[test]
fn coherence_response_records_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let response_rs = crate_dir.join("src/response.rs");

    assert!(
        response_rs.exists(),
        "coherence submit results and CPU response records belong in src/response.rs"
    );
    assert!(
        !lib_rs.contains("pub struct SubmitResult {"),
        "src/lib.rs should re-export submit results from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct CpuResponseRecord {"),
        "src/lib.rs should re-export CPU response records from a focused module"
    );
}

#[test]
fn coherence_partitioned_config_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let partitioned_config_rs = crate_dir.join("src/partitioned_config.rs");

    assert!(
        partitioned_config_rs.exists(),
        "partitioned coherence route and memory config belongs in src/partitioned_config.rs"
    );
    assert!(
        !lib_rs.contains("pub struct PartitionedCacheAgentConfig {"),
        "src/lib.rs should re-export partitioned cache config from a focused module"
    );
    assert!(
        !lib_rs.contains("pub struct PartitionedMemoryConfig {"),
        "src/lib.rs should re-export partitioned memory config from a focused module"
    );
}

#[test]
fn coherence_source_files_stay_within_size_limit() {
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
