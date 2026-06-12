use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_ARCHITECTURE_OVERVIEW_LINES: usize = 600;
const MAX_MIGRATION_LEDGER_LINES: usize = 1200;

#[test]
fn cli_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused CLI modules, but it has {lines} lines"
    );
}

#[test]
fn cli_source_files_stay_within_size_limit() {
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
fn cli_runtime_inputs_live_in_focused_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();

    assert!(
        crate_dir.join("src/config.rs").exists(),
        "CLI argument and runtime configuration parsing belongs in src/config.rs"
    );
    assert!(
        crate_dir.join("src/guest_memory.rs").exists(),
        "ELF and load-blob guest memory construction belongs in src/guest_memory.rs"
    );
    assert!(
        !lib_rs.contains("pub struct Rem6RunConfig {"),
        "src/lib.rs should re-export run configuration from a focused module"
    );
    assert!(
        !lib_rs.contains("fn read_load_blobs("),
        "src/lib.rs should delegate load-blob file reading to guest memory code"
    );
    assert!(
        !lib_rs.contains("fn build_cli_memory_store("),
        "src/lib.rs should delegate guest memory store construction to guest memory code"
    );
}

#[test]
fn architecture_docs_have_clear_boundaries() {
    let repo_root = repo_root();
    let architecture = repo_root.join("docs/architecture/rem6-architecture.md");
    let migration = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");

    assert!(
        architecture.exists(),
        "rem6 architecture needs one canonical doc"
    );
    assert!(
        migration.exists(),
        "gem5 to rem6 migration needs one canonical progress doc"
    );
    for retired in [
        "docs/architecture/gem5-to-rem6-alignment.md",
        "docs/architecture/gem5-test-migration.md",
        "docs/architecture/parallel-first-full-system-kernel.md",
    ] {
        assert!(
            !repo_root.join(retired).exists(),
            "{retired} should be folded into the canonical architecture or migration doc"
        );
    }

    let architecture_contents = fs::read_to_string(&architecture).unwrap();
    let architecture_lines = architecture_contents.lines().count();
    assert!(
        architecture_lines <= MAX_ARCHITECTURE_OVERVIEW_LINES,
        "rem6-architecture.md should stay a concise architecture overview, but it has {architecture_lines} lines"
    );
    for required in [
        "## gem5 Pain Points",
        "## rem6 Architecture",
        "## Runtime Invariants",
        "## Workspace Responsibilities",
        "## Evidence Policy",
    ] {
        assert!(
            architecture_contents.contains(required),
            "rem6-architecture.md is missing required section `{required}`"
        );
    }
    assert!(
        !architecture_contents.contains("## Migration Ledger")
            && !architecture_contents.contains("## Component Progress"),
        "architecture doc should not duplicate migration progress ledgers"
    );

    let migration_contents = fs::read_to_string(&migration).unwrap();
    let migration_lines = migration_contents.lines().count();
    assert!(
        migration_lines <= MAX_MIGRATION_LEDGER_LINES,
        "gem5-to-rem6-migration.md should stay a concise ledger, but it has {migration_lines} lines"
    );
    for required in [
        "## Scoring Rubric",
        "## Component Progress",
        "## Test Migration Ledger",
        "## Update Rules",
        "- [x]",
        "- [ ]",
        "%",
        "single-axis",
    ] {
        assert!(
            migration_contents.contains(required),
            "gem5-to-rem6-migration.md is missing required marker `{required}`"
        );
    }
}

#[test]
fn gem5_migration_doc_tracks_core_test_anchors() {
    let repo_root = repo_root();
    let path = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");
    let contents = fs::read_to_string(&path).unwrap();

    for required in [
        "## Scoring Rubric",
        "## Test Migration Ledger",
        "tests/gem5/asmtest",
        "tests/gem5/cpu_tests",
        "tests/gem5/chi_tlm_tests",
        "tests/gem5/dram_lowp",
        "tests/gem5/fdp_tests",
        "tests/gem5/fs",
        "tests/gem5/gem5_resources",
        "tests/gem5/se_mode",
        "tests/gem5/riscv_boot_tests",
        "tests/gem5/kvm_fork_tests",
        "tests/gem5/kvm_switch_tests",
        "tests/gem5/memory",
        "tests/gem5/m5threads_test_atomic",
        "tests/gem5/parsec_benchmarks",
        "tests/gem5/processor_switch_tests",
        "tests/gem5/py_port",
        "tests/gem5/readfile_tests",
        "tests/gem5/replacement_policies",
        "tests/gem5/regression_tests",
        "tests/gem5/traffic_gen",
        "tests/gem5/stats",
        "tests/gem5/checkpoint_tests",
        "tests/gem5/gpu",
        "tests/gem5/stdlib",
        "tests/pyunit",
        "tests/test-progs",
        "rem6 owner",
        "Score",
        "Next evidence",
    ] {
        assert!(
            contents.contains(required),
            "gem5-to-rem6-migration.md is missing required anchor `{required}`"
        );
    }
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
