use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1250;
const MAX_CONFIG_ROOT_LINES: usize = 1700;
const MAX_M5_HOST_ACTIONS_ROOT_LINES: usize = 5000;
const MAX_M5_HOST_ACTIONS_O3_ROOT_LINES: usize = 4500;
const MAX_M5_HOST_ACTIONS_O3_LSQ_ROOT_LINES: usize = 1400;
const MAX_M5_HOST_ACTIONS_O3_MODULE_LINES: usize = 1800;
const MAX_STATS_OUTPUT_CPU_LINES: usize = 1700;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_RISCV_SBI_SMOKE_LINES: usize = 1500;
const MAX_ARCHITECTURE_OVERVIEW_LINES: usize = 600;
const REQUIRED_MIGRATION_LEDGER_LINES: usize = 1200;
const MAX_ARCHITECTURE_README_LINES: usize = 80;
const MIGRATION_SCORE_BUCKETS: &[MigrationScoreBucket] = &[
    MigrationScoreBucket {
        name: "open",
        min: 0,
        max: 0,
    },
    MigrationScoreBucket {
        name: "scoped",
        min: 1,
        max: 19,
    },
    MigrationScoreBucket {
        name: "unit-slice",
        min: 20,
        max: 39,
    },
    MigrationScoreBucket {
        name: "single-axis",
        min: 40,
        max: 59,
    },
    MigrationScoreBucket {
        name: "representative",
        min: 60,
        max: 74,
    },
    MigrationScoreBucket {
        name: "matrix-gapped",
        min: 75,
        max: 89,
    },
    MigrationScoreBucket {
        name: "near-covered",
        min: 90,
        max: 99,
    },
    MigrationScoreBucket {
        name: "covered",
        min: 100,
        max: 100,
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MigrationScoreBucket {
    name: &'static str,
    min: u8,
    max: u8,
}

impl MigrationScoreBucket {
    fn contains(self, percent: u8) -> bool {
        percent >= self.min && percent <= self.max
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MigrationScore<'a> {
    percent: u8,
    bucket: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScoreCalculation {
    completed: usize,
    total: usize,
    raw_percent: u8,
    bucket: String,
}

#[test]
fn public_run_config_exports_include_riscv_se_input_source() {
    let name = std::any::type_name::<rem6::RiscvSeInputSource>();

    assert!(name.ends_with("RiscvSeInputSource"));
}

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
fn cli_riscv_se_smoke_modules_stay_within_size_limit() {
    let cli_run_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run");
    let mut oversized = Vec::new();

    for path in rust_source_files(&cli_run_dir) {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !file_name.starts_with("riscv_se_") {
            continue;
        }
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
        "RISC-V SE CLI smoke modules exceed {MAX_SOURCE_LINES} lines: {}",
        oversized.join(", ")
    );
}

#[test]
fn cli_riscv_sbi_smoke_modules_stay_within_size_limit() {
    let cli_run_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run");
    let mut oversized = Vec::new();

    for path in rust_source_files(&cli_run_dir) {
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let relative = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap();
        if !file_name.starts_with("riscv_sbi")
            && !relative.starts_with(Path::new("tests/cli_run/riscv_sbi"))
        {
            continue;
        }
        let lines = line_count(&path);
        if lines > MAX_RISCV_SBI_SMOKE_LINES {
            oversized.push(format!("{} has {lines} lines", relative.display()));
        }
    }

    assert!(
        oversized.is_empty(),
        "RISC-V SBI CLI smoke modules exceed {MAX_RISCV_SBI_SMOKE_LINES} lines: {}",
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
fn cli_config_root_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/config.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_CONFIG_ROOT_LINES,
        "src/config.rs should remain a facade over focused config parsers, but it has {lines} lines"
    );
}

#[test]
fn cli_stats_output_root_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stats_output.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/stats_output.rs should remain a facade over focused stats-output modules, but it has {lines} lines"
    );
}

#[test]
fn cli_stats_output_cpu_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stats_output/cpu.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_STATS_OUTPUT_CPU_LINES,
        "src/stats_output/cpu.rs should delegate detailed CPU stat families to focused modules, but it has {lines} lines"
    );
}

#[test]
fn cli_m5_host_actions_root_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_M5_HOST_ACTIONS_ROOT_LINES,
        "tests/cli_run/m5_host_actions.rs should delegate detailed host-action families to focused modules, but it has {lines} lines"
    );
}

#[test]
fn cli_m5_host_actions_o3_root_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions/o3.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_M5_HOST_ACTIONS_O3_ROOT_LINES,
        "tests/cli_run/m5_host_actions/o3.rs should delegate detailed O3 host-action families to focused modules, but it has {lines} lines"
    );
}

#[test]
fn cli_m5_host_actions_o3_lsq_root_stays_focused() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions/o3/lsq.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_M5_HOST_ACTIONS_O3_LSQ_ROOT_LINES,
        "tests/cli_run/m5_host_actions/o3/lsq.rs should delegate detailed LSQ families to focused modules, but it has {lines} lines"
    );
}

#[test]
fn cli_m5_host_actions_o3_modules_stay_focused() {
    let o3_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions/o3");
    let mut oversized = Vec::new();

    for path in rust_source_files(&o3_dir) {
        let lines = line_count(&path);
        if lines > MAX_M5_HOST_ACTIONS_O3_MODULE_LINES {
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
        "O3 host-action child modules exceed {MAX_M5_HOST_ACTIONS_O3_MODULE_LINES} lines: {}",
        oversized.join(", ")
    );
}

#[test]
fn architecture_docs_have_clear_boundaries() {
    let repo_root = repo_root();
    let readme = repo_root.join("docs/architecture/README.md");
    let architecture = repo_root.join("docs/architecture/rem6-architecture.md");
    let migration = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");

    assert!(
        readme.exists(),
        "architecture directory needs a content index"
    );
    assert!(
        architecture.exists(),
        "rem6 architecture needs one canonical doc"
    );
    assert!(
        migration.exists(),
        "gem5 to rem6 migration needs one canonical progress doc"
    );
    let architecture_docs = architecture_doc_paths(&repo_root);
    assert_eq!(
        architecture_docs,
        [
            "docs/architecture/README.md",
            "docs/architecture/gem5-to-rem6-migration.md",
            "docs/architecture/rem6-architecture.md",
        ],
        "docs/architecture should contain only the directory index, architecture overview, and migration ledger"
    );
    for retired in [
        "docs/architecture/gem5-test-migration.md",
        "docs/architecture/parallel-first-full-system-kernel.md",
    ] {
        assert!(
            !repo_root.join(retired).exists(),
            "{retired} should be folded into the canonical architecture or migration doc"
        );
    }

    let readme_contents = fs::read_to_string(&readme).unwrap();
    let readme_lines = readme_contents.lines().count();
    assert!(
        readme_lines <= MAX_ARCHITECTURE_README_LINES,
        "docs/architecture/README.md should stay a concise index, but it has {readme_lines} lines"
    );
    for required in [
        "# Architecture Documents",
        "## Content Index",
        "docs/architecture/rem6-architecture.md",
        "docs/architecture/gem5-to-rem6-migration.md",
        "stable architecture overview",
        "mutable migration ledger",
    ] {
        assert!(
            readme_contents.contains(required),
            "docs/architecture/README.md is missing required marker `{required}`"
        );
    }
    for forbidden in [
        "## Component Progress",
        "## Test Migration Ledger",
        "- [x]",
        "- [ ]",
    ] {
        assert!(
            !readme_contents.contains(forbidden),
            "docs/architecture/README.md should not duplicate migration ledger marker `{forbidden}`"
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
    assert_eq!(
        migration_lines, REQUIRED_MIGRATION_LEDGER_LINES,
        "gem5-to-rem6-migration.md must stay exactly {REQUIRED_MIGRATION_LEDGER_LINES} lines, but it has {migration_lines} lines"
    );
    for required in [
        "## Document Boundary",
        "## Scoring Rubric",
        "## Component Progress",
        "## Test Migration Ledger",
        "## Update Rules",
        "- [x]",
        "- [ ]",
        "%",
        "Row score",
        "Checklist source",
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
        "Row score",
        "Next evidence",
        "demandMshrHits",
        "conditional_branch_predictions",
        "conditional_branch_predicted_taken",
        "conditional_branch_mispredictions",
        "system.cpu.branchPred.condPredicted",
        "system.cpu.branchPred.condPredictedTaken",
        "system.cpu.branchPred.condIncorrect",
        "system.cpu.pipeline.inOrder.stallCycles",
        "system.cpu.pipeline.inOrder.fetchWaitCycles",
        "system.cpu.pipeline.inOrder.executeWaitCycles",
        "system.cpu.pipeline.inOrder.resourceBlocked",
        "system.cpu.pipeline.inOrder.orderingBlocked",
        "system.cpu.pipeline.inOrder.flushCycles",
        "system.cpu.pipeline.inOrder.branchPredictionFlushes",
        "system.cpu.pipeline.inOrder.branchPredictionFlushCycles",
        "system.cpu.pipeline.inOrder.branchSpeculationPredictions",
        "system.cpu.pipeline.inOrder.branchSpeculationRepairs",
        "system.cpu.pipeline.inOrder.branchSpeculationRemovedYoungers",
        "system.cpu.pipeline.inOrder.branchSpeculationMaxPending",
        "system.cpu.pipeline.inOrder.branchPredictionRedirects",
        "system.cpu.pipeline.inOrder.interruptRedirects",
        "system.cpu.pipeline.inOrder.interruptRedirectFlushes",
        "system.cpu.pipeline.inOrder.interruptRedirectFlushCycles",
        "system.cpu.pipeline.inOrder.trapRedirects",
        "system.cpu.pipeline.inOrder.trapRedirectFlushes",
        "system.cpu.pipeline.inOrder.trapRedirectFlushCycles",
        "system.cpu.pipeline.inOrder.stage.decode.interruptRedirectFlushed",
        "system.cpu.pipeline.inOrder.stage.decode.interruptRedirectFlushedCycles",
        "system.cpu.pipeline.inOrder.stage.decode.trapRedirectFlushed",
        "system.cpu.pipeline.inOrder.stage.decode.trapRedirectFlushedCycles",
        "sim.cpu0.pipeline.in_order.branch_prediction_redirects",
        "sim.cpu0.pipeline.in_order.interrupt_redirects",
        "sim.cpu0.pipeline.in_order.interrupt_redirect_flushes",
        "sim.cpu0.pipeline.in_order.interrupt_redirect_flush_cycles",
        "sim.cpu0.pipeline.in_order.stage.decode.interrupt_redirect_flushed",
        "sim.cpu0.pipeline.in_order.stage.decode.interrupt_redirect_flushed_cycles",
        "sim.cpu0.pipeline.in_order.trap_redirects",
        "sim.cpu0.pipeline.in_order.trap_redirect_flushes",
        "sim.cpu0.pipeline.in_order.trap_redirect_flush_cycles",
        "sim.cpu0.pipeline.in_order.stage.decode.trap_redirect_flushed",
        "sim.cpu0.pipeline.in_order.stage.decode.trap_redirect_flushed_cycles",
        "system.cpu.pipeline.inOrder.stage.fetch1.resourceBlocked",
        "system.cpu.pipeline.inOrder.stage.fetch1.resourceBlockedCycles",
        "system.cpu.pipeline.inOrder.stage.decode.branchPredictionFlushed",
        "system.cpu.pipeline.inOrder.stallCause.fetchWait.stage.fetch1.resourceBlocked",
        "system.cpu.pipeline.inOrder.flushCause.branchPrediction.stage.fetch1.flushed",
        "system.cpu.pipeline.inOrder.redirectCause.branchPrediction.stage.fetch1.flushed",
        "system.cpu0.branchPred.condPredicted",
        "system.cpu0.branchPred.condPredictedTaken",
        "system.cpu0.branchPred.condIncorrect",
        "system.cpu0.pipeline.inOrder.stallCycles",
        "system.cpu0.pipeline.inOrder.resourceBlocked",
        "system.cpu1.branchPred.condPredicted",
        "system.cpu1.branchPred.condPredictedTaken",
        "system.cpu1.branchPred.condIncorrect",
        "system.cpu1.pipeline.inOrder.stallCycles",
        "system.cpu1.pipeline.inOrder.resourceBlocked",
        "system.cpu.branchPred.BTBLookups",
        "system.cpu.branchPred.btb.lookups::total",
        "system.cpu.branchPred.btb.lookups::DirectCond",
        "system.cpu.branchPred.btb.lookups::DirectUncond",
        "system.cpu.branchPred.BTBHits",
        "system.cpu.branchPred.btb.misses::total",
        "system.cpu.branchPred.btb.misses::DirectCond",
        "system.cpu.branchPred.btb.misses::DirectUncond",
        "system.cpu.branchPred.BTBUpdates",
        "system.cpu.branchPred.btb.updates::total",
        "system.cpu.branchPred.btb.updates::DirectCond",
        "system.cpu.branchPred.btb.updates::DirectUncond",
        "system.cpu.branchPred.btb.evictions",
        "system.cpu.branchPred.BTBHitRatio",
        "system.cpu.branchPred.BTBMispredicted",
        "system.cpu.branchPred.btb.mispredict::total",
        "system.cpu.branchPred.predTakenBTBMiss",
        "system.cpu.branchPred.indirectMispredicted",
        "system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond",
        "system.cpu.branchPred.mispredictDueToBTBMiss_0::IndirectUncond",
        "system.cpu.branchPred.mispredictDueToBTBMiss_0::total",
        "system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond",
        "system.cpu.branchPred.mispredictDueToPredictor_0::IndirectUncond",
        "system.cpu.branchPred.mispredictDueToPredictor_0::total",
        "system.cpu.branchPred.committed_0::DirectCond",
        "system.cpu.branchPred.committed_0::IndirectUncond",
        "system.cpu.branchPred.committed_0::total",
        "system.cpu.branchPred.mispredicted_0::DirectCond",
        "system.cpu.branchPred.mispredicted_0::IndirectUncond",
        "system.cpu.branchPred.mispredicted_0::total",
        "system.cpu.branchPred.corrected_0::DirectCond",
        "system.cpu.branchPred.corrected_0::IndirectUncond",
        "system.cpu.branchPred.corrected_0::total",
        "system.cpu.branchPred.lookups_0::DirectCond",
        "system.cpu.branchPred.lookups_0::DirectUncond",
        "system.cpu.branchPred.lookups_0::IndirectUncond",
        "system.cpu.branchPred.lookups_0::total",
        "system.cpu.branchPred.targetWrong_0::DirectCond",
        "system.cpu.branchPred.targetWrong_0::IndirectUncond",
        "system.cpu.branchPred.targetWrong_0::total",
        "system.cpu.branchPred.targetProvider_0::BTB",
        "system.cpu.branchPred.targetProvider_0::Indirect",
        "system.cpu.branchPred.targetProvider_0::NoTarget",
        "system.cpu.branchPred.targetProvider_0::RAS",
        "system.cpu.branchPred.targetProvider_0::total",
        "system.cpu0.branchPred.BTBLookups",
        "system.cpu0.branchPred.btb.lookups::total",
        "system.cpu0.branchPred.btb.lookups::DirectCond",
        "system.cpu0.branchPred.btb.lookups::DirectUncond",
        "system.cpu0.branchPred.BTBHits",
        "system.cpu0.branchPred.btb.misses::total",
        "system.cpu0.branchPred.btb.misses::DirectCond",
        "system.cpu0.branchPred.btb.misses::DirectUncond",
        "system.cpu0.branchPred.BTBUpdates",
        "system.cpu0.branchPred.btb.updates::total",
        "system.cpu0.branchPred.btb.updates::DirectCond",
        "system.cpu0.branchPred.btb.updates::DirectUncond",
        "system.cpu0.branchPred.btb.evictions",
        "system.cpu0.branchPred.BTBHitRatio",
        "system.cpu0.branchPred.BTBMispredicted",
        "system.cpu0.branchPred.btb.mispredict::total",
        "system.cpu0.branchPred.predTakenBTBMiss",
        "system.cpu0.branchPred.indirectMispredicted",
        "system.cpu0.branchPred.mispredictDueToBTBMiss_0::DirectCond",
        "system.cpu0.branchPred.mispredictDueToBTBMiss_0::IndirectUncond",
        "system.cpu0.branchPred.mispredictDueToPredictor_0::DirectCond",
        "system.cpu0.branchPred.mispredictDueToPredictor_0::IndirectUncond",
        "system.cpu0.branchPred.committed_0::DirectCond",
        "system.cpu0.branchPred.committed_0::IndirectUncond",
        "system.cpu0.branchPred.mispredicted_0::DirectCond",
        "system.cpu0.branchPred.mispredicted_0::IndirectUncond",
        "system.cpu0.branchPred.corrected_0::DirectCond",
        "system.cpu0.branchPred.corrected_0::IndirectUncond",
        "system.cpu0.branchPred.corrected_0::total",
        "system.cpu0.branchPred.lookups_0::DirectCond",
        "system.cpu0.branchPred.lookups_0::DirectUncond",
        "system.cpu0.branchPred.lookups_0::IndirectUncond",
        "system.cpu0.branchPred.lookups_0::total",
        "system.cpu0.branchPred.targetWrong_0::DirectCond",
        "system.cpu0.branchPred.targetWrong_0::IndirectUncond",
        "system.cpu0.branchPred.targetWrong_0::total",
        "system.cpu0.branchPred.targetProvider_0::BTB",
        "system.cpu0.branchPred.targetProvider_0::Indirect",
        "system.cpu0.branchPred.targetProvider_0::NoTarget",
        "system.cpu0.branchPred.targetProvider_0::RAS",
        "system.cpu0.branchPred.targetProvider_0::total",
        "system.cpu1.branchPred.BTBLookups",
        "system.cpu1.branchPred.btb.lookups::total",
        "system.cpu1.branchPred.btb.lookups::DirectCond",
        "system.cpu1.branchPred.btb.lookups::DirectUncond",
        "system.cpu1.branchPred.BTBHits",
        "system.cpu1.branchPred.btb.misses::total",
        "system.cpu1.branchPred.btb.misses::DirectCond",
        "system.cpu1.branchPred.btb.misses::DirectUncond",
        "system.cpu1.branchPred.BTBUpdates",
        "system.cpu1.branchPred.btb.updates::total",
        "system.cpu1.branchPred.btb.updates::DirectCond",
        "system.cpu1.branchPred.btb.updates::DirectUncond",
        "system.cpu1.branchPred.btb.evictions",
        "system.cpu1.branchPred.BTBHitRatio",
        "system.cpu1.branchPred.BTBMispredicted",
        "system.cpu1.branchPred.btb.mispredict::total",
        "system.cpu1.branchPred.predTakenBTBMiss",
        "system.cpu1.branchPred.indirectMispredicted",
        "system.cpu1.branchPred.mispredictDueToBTBMiss_0::DirectCond",
        "system.cpu1.branchPred.mispredictDueToBTBMiss_0::IndirectUncond",
        "system.cpu1.branchPred.mispredictDueToPredictor_0::DirectCond",
        "system.cpu1.branchPred.mispredictDueToPredictor_0::IndirectUncond",
        "system.cpu1.branchPred.committed_0::DirectCond",
        "system.cpu1.branchPred.committed_0::IndirectUncond",
        "system.cpu1.branchPred.mispredicted_0::DirectCond",
        "system.cpu1.branchPred.mispredicted_0::IndirectUncond",
        "system.cpu1.branchPred.corrected_0::DirectCond",
        "system.cpu1.branchPred.corrected_0::IndirectUncond",
        "system.cpu1.branchPred.corrected_0::total",
        "system.cpu1.branchPred.lookups_0::DirectCond",
        "system.cpu1.branchPred.lookups_0::DirectUncond",
        "system.cpu1.branchPred.lookups_0::IndirectUncond",
        "system.cpu1.branchPred.lookups_0::total",
        "system.cpu1.branchPred.targetWrong_0::DirectCond",
        "system.cpu1.branchPred.targetWrong_0::IndirectUncond",
        "system.cpu1.branchPred.targetWrong_0::total",
        "system.cpu1.branchPred.targetProvider_0::BTB",
        "system.cpu1.branchPred.targetProvider_0::Indirect",
        "system.cpu1.branchPred.targetProvider_0::NoTarget",
        "system.cpu1.branchPred.targetProvider_0::RAS",
        "system.cpu1.branchPred.targetProvider_0::total",
        "sim.debug.branch_trace.records",
        "sim.debug.branch_trace.mispredictions",
        "sim.debug.branch_trace.flushed",
        "sim.debug.branch_trace.cpu.cpu0.records",
        "sim.debug.pipeline_trace.records",
        "sim.debug.pipeline_trace.branch_prediction_flushed",
        "sim.debug.pipeline_trace.flush_cause.branch_prediction.records",
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.records",
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.flushed",
        "sim.debug.pipeline_trace.redirect_cause.trap_redirect.branch_prediction_flushed",
        "sim.debug.pipeline_trace.stall_cause.fetch_wait.records",
        "sim.debug.pipeline_trace.stall_cause.data_wait.stall_cycles",
        "sim.debug.pipeline_trace.stall_cause.execute_wait.records",
        "sim.debug.pipeline_trace.stall_cause.execute_wait.stage.decode.ordering_blocked_cycles",
        "sim.debug.pipeline_trace.stage.commit.advanced",
        "sim.debug.pipeline_trace.stage.commit.retired",
        "sim.debug.pipeline_trace.stage.decode.ordering_blocked_cycles",
        "sim.debug.pipeline_trace.stage.fetch1.flushed",
        "sim.debug.pipeline_trace.stage.fetch1.branch_prediction_flushed_cycles",
        "sim.debug.pipeline_trace.stage.commit.trap_redirect_flushed_cycles",
        "sim.debug.pipeline_trace.stage.commit.resource_blocked",
        "/debug/pipeline_summary/stage/commit/resource_blocked_cycles",
        "/debug/pipeline_summary/stall_cause/fetch_wait/records",
        "/debug/pipeline_summary/stall_cause/fetch_wait/stage/commit/records",
        "/debug/pipeline_summary/stall_cause/data_wait/stage/commit/resource_blocked_cycles",
        "/debug/pipeline_summary/stall_cause/data_wait/stage/commit/records",
        "/debug/pipeline_summary/stall_cause/fetch_wait/stage/execute/resource_blocked_cycles",
        "/debug/pipeline_summary/stall_cause/execute_wait/stage/decode/records",
        "/debug/pipeline_summary/flush_cause/branch_prediction/stage/fetch1/records",
        "/debug/pipeline_summary/flush_cause/branch_prediction/stage/fetch1/flushed_cycles",
        "/debug/pipeline_summary/flush_cause/trap_redirect/stage/commit/records",
        "/debug/pipeline_summary/redirect_cause/trap_redirect/stage/commit/records",
        "/debug/pipeline_summary/redirect_cause/trap_redirect/stage/commit/flushed_cycles",
        "sim.cpu0.pipeline.in_order.execute_wait_cycles",
        "sim.cpu0.pipeline.in_order.stage.fetch1.advanced_cycles",
        "sim.cpu0.pipeline.in_order.stage.commit.retired_cycles",
        "/cores/0/in_order_pipeline/stage_advanced/fetch1",
        "/cores/0/in_order_pipeline/stage_retired_cycles/commit",
        "sim.cpu0.pipeline.in_order.flush_cause.branch_prediction.records",
        "sim.cpu0.pipeline.in_order.flush_cause.branch_prediction.stage.fetch1.records",
        "sim.cpu0.pipeline.in_order.flush_cause.branch_prediction.stage.fetch1.flushed_cycles",
        "/cores/0/in_order_pipeline/flush_cause/branch_prediction/records",
        "/cores/0/in_order_pipeline/flush_cause/branch_prediction/stage_records/fetch1",
        "sim.cpu0.pipeline.in_order.flush_cause.trap_redirect.stage.commit.flushed_cycles",
        "/cores/0/in_order_pipeline/flush_cause/branch_prediction/stage_flushed_cycles/fetch1",
        "sim.cpu0.pipeline.in_order.redirect_cause.branch_prediction.stage.fetch1.flushed_cycles",
        "sim.cpu0.pipeline.in_order.redirect_cause.trap_redirect.records",
        "sim.cpu0.pipeline.in_order.redirect_cause.trap_redirect.stage.commit.records",
        "sim.cpu0.pipeline.in_order.redirect_cause.trap_redirect.stage.commit.flushed_cycles",
        "/cores/0/in_order_pipeline/redirect_cause/trap_redirect/records",
        "/cores/0/in_order_pipeline/redirect_cause/trap_redirect/stage_records/commit",
        "/cores/0/in_order_pipeline/redirect_cause/trap_redirect/stage_flushed_cycles/commit",
        "sim.cpu0.pipeline.in_order.stall_cause.data_wait.records",
        "sim.cpu0.pipeline.in_order.stall_cause.data_wait.stage.commit.records",
        "sim.cpu0.pipeline.in_order.stall_cause.data_wait.stage.commit.resource_blocked",
        "sim.cpu0.pipeline.in_order.stall_cause.data_wait.stage.commit.resource_blocked_cycles",
        "sim.cpu0.pipeline.in_order.stall_cause.execute_wait.records",
        "sim.cpu0.pipeline.in_order.stall_cause.execute_wait.stage.execute.resource_blocked_cycles",
        "sim.cpu0.pipeline.in_order.stall_cause.execute_wait.stage.decode.records",
        "sim.cpu0.pipeline.in_order.stall_cause.execute_wait.stage.decode.ordering_blocked",
        "sim.cpu0.pipeline.in_order.stall_cause.execute_wait.stage.decode.ordering_blocked_cycles",
        "/cores/0/in_order_pipeline/stall_cause/data_wait/records",
        "/cores/0/in_order_pipeline/stall_cause/data_wait/stage_records/commit",
        "/cores/0/in_order_pipeline/stall_cause/data_wait/stage_resource_blocked_cycles/commit",
        "/cores/0/in_order_pipeline/stall_cause/execute_wait/records",
        "/cores/0/in_order_pipeline/stall_cause/execute_wait/stage_records/decode",
        "/cores/0/in_order_pipeline/stall_cause/execute_wait/stage_ordering_blocked_cycles/decode",
        "system.cpu.pipeline.inOrder.stage.fetch1.advancedCycles",
        "system.cpu.pipeline.inOrder.stage.commit.retiredCycles",
        "system.cpu.pipeline.inOrder.stallCause.dataWait.records",
        "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.records",
        "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.resourceBlocked",
        "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.resourceBlockedCycles",
        "system.cpu.pipeline.inOrder.flushCause.branchPrediction.records",
        "system.cpu.pipeline.inOrder.flushCause.branchPrediction.stage.fetch1.records",
        "system.cpu.pipeline.inOrder.redirectCause.trapRedirect.records",
        "system.cpu.pipeline.inOrder.redirectCause.trapRedirect.stage.commit.records",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.records",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.execute.resourceBlockedCycles",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.decode.records",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.decode.orderingBlocked",
        "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.decode.orderingBlockedCycles",
        "system.cpu.pipeline.inOrder.flushCause.branchPrediction.stage.fetch1.flushedCycles",
        "system.cpu.pipeline.inOrder.flushCause.interruptRedirect.stage.fetch1.flushedCycles",
        "system.cpu.pipeline.inOrder.redirectCause.interruptRedirect.stage.commit.flushedCycles",
        "system.cpu.pipeline.inOrder.redirectCause.trapRedirect.stage.commit.flushedCycles",
        "sim.host_actions.execution_mode_switch_quiescence.checker.checked_instructions",
        "sim.host_actions.execution_mode_switch_quiescence.checker.mismatches",
        "/debug/host_action_trace/0/state_transfer/quiescence_gate/checker/checked_instructions",
        "/debug/host_action_trace/0/state_transfer/quiescence_gate/captured_payload_bytes",
        "/debug/host_action_trace/1/execution_mode_authority/mode/detailed",
        "/debug/host_action_trace/1/execution_mode_authority/target/cpu0/mode/detailed",
        "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
        "sim.debug.host_action_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
        "--riscv-execution-mode detailed",
        "sim.host_actions.execution_mode_authority.mode.timing",
        "sim.host_actions.execution_mode_authority.mode.detailed",
        "sim.host_actions.execution_mode_authority.target.cpu0.mode.functional",
        "sim.host_actions.execution_mode_authority.target.cpu0.mode.timing",
        "sim.host_actions.execution_mode_authority.target.cpu0.mode.detailed",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.manifests",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.cleared_manifests",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.targets",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.mode.detailed",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
        "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
        "sim.host_actions.execution_mode_switch.mode.functional",
        "sim.host_actions.execution_mode_switch.target.cpu0.mode.functional",
        "sim.host_actions.execution_mode_switch.target.cpu0.mode.timing",
        "sim.host_actions.execution_mode_switch.target.cpu0.mode.detailed",
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.none",
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.functional",
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.timing",
        "sim.host_actions.execution_mode_switch.previous_mode.target.cpu0.detailed",
        "sim.host_actions.execution_mode_switch.quiescence.target.cpu0.validated",
        "sim.host_actions.execution_mode_switch.quiescence.captured_payload_bytes",
        "sim.debug.pipeline_trace.cpu.cpu0.records",
        "sim.debug.o3_trace.store_load_forwarding_suppressed",
        "sim.debug.o3_trace.store_load_forwarding_address_mismatches",
        "sim.debug.o3_trace.store_load_forwarding_byte_mismatches",
        "sim.debug.o3_trace.execution_mode.detailed",
        "/cores/1/o3_runtime/execution_mode",
        "/cores/1/o3_runtime/stats_reset_tick",
        "/cores/0/o3_runtime/checkpoint_restore/components",
        "sim.debug.o3_trace.execution_mode_authority.targets",
        "sim.debug.o3_trace.execution_mode_authority.mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.functional",
        "sim.debug.o3_trace.execution_mode_authority.target.cpu1.mode.timing",
        "sim.debug.o3_trace.cpu.cpu0.records",
        "sim.debug.o3_trace.cpu.cpu0.execution_mode.detailed",
        "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.mode.detailed",
        "sim.debug.o3_trace.cpu.cpu1.execution_mode_authority.target.cpu1.mode.detailed",
        "sim.debug.o3_trace.cpu.cpu1.event.tick_span",
        "/debug/o3_trace/0/execution_mode_authority/mode/detailed",
        "/debug/o3_trace/0/execution_mode_authority/target/cpu0/mode/detailed",
        "sim.debug.o3_trace.event.system_events",
        "sim.debug.o3_trace.event.iew_writeback_rate_ppm",
        "sim.debug.o3_trace.event.iew_dependency_producers",
        "sim.debug.o3_trace.event.iew_dependency_consumers",
        "sim.debug.o3_trace.event.iew_predicted_taken_incorrect",
        "sim.debug.o3_trace.event.iew_predicted_not_taken_incorrect",
        "sim.debug.o3_trace.event.iew_branch_mispredicts",
        "/debug/o3_trace/0/events/1/iew_dependency_producers",
        "/debug/o3_trace/0/events/4/iew_dependency_consumers",
        "/debug/o3_trace/0/event_summary/records",
        "/debug/o3_trace/0/event_summary/span_ticks",
        "/debug/o3_trace/0/event_summary/event_window/max_rob_occupancy/tick",
        "/debug/o3_trace/0/event_summary/event_window/max_lsq_occupancy/pc",
        "/debug/o3_trace/0/event_summary/event_window/max_rename_map_entries/sequence",
        "sim.cpu0.o3.event_summary.event_window.first.pc",
        "sim.cpu0.o3.event_summary.event_window.last.pc",
        "sim.cpu0.o3.event_summary.event_window.max_rob_occupancy.pc",
        "sim.cpu0.o3.event_summary.event_window.max_lsq_occupancy.pc",
        "sim.cpu0.o3.event_summary.event_window.max_rename_map_entries.pc",
        "/debug/o3_trace/0/event_summary/iew/producer_inst",
        "/debug/o3_trace/0/event_summary/iew/consumer_inst",
        "/debug/o3_trace/0/event_summary/iew/writeback_rate_ppm",
        "/debug/o3_trace/0/event_summary/iew/producer_consumer_fanout_ppm",
        "/debug/o3_trace/0/event_summary/iew/predicted_taken_incorrect",
        "/debug/o3_trace/0/event_summary/iew/predicted_not_taken_incorrect",
        "/debug/o3_trace/0/event_summary/iew/branch_mispredicts",
        "/debug/o3_trace/0/event_summary/iew/dependency/producer",
        "/debug/o3_trace/0/event_summary/iew/dependency/consumer",
        "sim.cpu0.o3.event_summary.iew.writeback_rate_ppm",
        "sim.cpu0.o3.event_summary.iew.producer_inst",
        "sim.cpu0.o3.event_summary.iew.consumer_inst",
        "sim.cpu0.o3.event_summary.iew.producer_consumer_fanout_ppm",
        "sim.cpu0.o3.event_summary.iew.dependency.producer",
        "sim.cpu0.o3.event_summary.iew.dependency.consumer",
        "sim.cpu0.o3.event_summary.iew.predicted_taken_incorrect",
        "sim.cpu0.o3.event_summary.iew.predicted_not_taken_incorrect",
        "/debug/o3_trace/0/event_summary/branch_event/branches",
        "/debug/o3_trace/0/event_summary/branch_event/taken",
        "/debug/o3_trace/0/event_summary/branch_event/not_taken",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_taken",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_not_taken",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_targets",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_matches",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_mismatches",
        "/debug/o3_trace/0/event_summary/branch_event/resolved_targets",
        "/debug/o3_trace/0/event_summary/branch_event/mispredictions",
        "/debug/o3_trace/0/event_summary/branch_event/squashes",
        "/debug/o3_trace/0/event_summary/branch_event/link_writes",
        "/debug/o3_trace/0/event_summary/branch_event/without_link_writes",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_targets",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_targets_with_link_writes",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_targets_without_link_writes",
        "/debug/o3_trace/0/event_summary/branch_event/kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/taken_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/taken_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/taken_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/not_taken_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_taken_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_taken_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_not_taken_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_not_taken_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_match_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_match_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_mismatch_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/predicted_target_mismatch_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/resolved_target_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/resolved_target_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/resolved_target_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/misprediction_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/misprediction_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/misprediction_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/link_write_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/without_link_write_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/squash_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/squash_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_event/squash_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_link_write_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_without_link_write_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_event/squashed_target_without_link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_repair/targetless_mismatches",
        "/debug/o3_trace/0/event_summary/branch_repair/wrong_targets",
        "/debug/o3_trace/0/event_summary/branch_repair/direction_only_mismatches",
        "/debug/o3_trace/0/event_summary/branch_repair/targetless_mismatch_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_repair/wrong_target_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_repair/direction_only_kind/direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_event.squashes",
        "sim.cpu0.o3.event_summary.branch_event.squashed_targets",
        "sim.cpu0.o3.event_summary.branch_event.squashed_target_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_repair.targetless_mismatches",
        "sim.cpu0.o3.event_summary.branch_repair.direction_only_mismatches",
        "sim.cpu0.o3.event_summary.branch_repair.targetless_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_repair.direction_only_kind.direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_target_mismatch/wrong_targets",
        "/debug/o3_trace/0/event_summary/branch_target_mismatch/wrong_target_squashed_target_link_write_kind/call_indirect",
        "/debug/o3_trace/0/event_summary/branch_target_mismatch/targetless_mismatch_kind/direct_conditional",
        "/debug/o3_trace/0/event_summary/branch_direction_mismatch/mismatches",
        "/debug/o3_trace/0/event_summary/branch_direction_mismatch/without_link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/branch_direction_mismatch/squashed_target_without_link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/event_summary/rob_allocations",
        "/debug/o3_trace/0/event_summary/rob/allocations",
        "/debug/o3_trace/0/event_summary/rob/commits",
        "/debug/o3_trace/0/event_summary/rob/max_occupancy",
        "/debug/o3_trace/0/event_summary/rename_writes",
        "/debug/o3_trace/0/event_summary/rename/writes",
        "/debug/o3_trace/0/event_summary/rename/map_entries",
        "sim.cpu0.o3.event_summary.rob_allocations",
        "sim.cpu0.o3.event_summary.rob_commits",
        "sim.cpu0.o3.event_summary.rename_writes",
        "/debug/o3_trace/0/event_summary/iq/issued_inst_type/mem_read",
        "/debug/o3_trace/0/event_summary/iq/issued_inst_type/mem_write",
        "/debug/o3_trace/0/event_summary/iq/issued_inst_type/int_mul",
        "/debug/o3_trace/0/event_summary/iq/issued_inst_type/int_div",
        "sim.debug.o3_trace.event.iq_insts_issued",
        "sim.debug.o3_trace.event.iq_issued_inst_type.int_mul",
        "/debug/o3_trace/0/event_summary/commit/branch_mispredicts",
        "/debug/o3_trace/0/event_summary/commit/committed_inst_type/mem_read",
        "/debug/o3_trace/0/event_summary/commit/committed_inst_type/mem_write",
        "/debug/o3_trace/0/event_summary/commit/committed_inst_type/int_div",
        "sim.debug.o3_trace.event.commit_branch_mispredicts",
        "sim.debug.o3_trace.event.commit_committed_inst_type.int_div",
        "/debug/o3_trace/0/event_summary/lsq_loads",
        "/debug/o3_trace/0/event_summary/lsq_operation_load",
        "/debug/o3_trace/0/event_summary/lsq_operation_store",
        "sim.cpu0.o3.event_summary.lsq_loads",
        "sim.cpu0.o3.event_summary.lsq_stores",
        "sim.cpu0.o3.event_summary.lsq_operation_load",
        "sim.cpu0.o3.event_summary.lsq_operation_store",
        "/debug/o3_trace/0/event_summary/store_load_forwarding_candidates",
        "/debug/o3_trace/0/event_summary/store_load_forwarding_matches",
        "/debug/o3_trace/0/event_summary/store_load_forwarding_suppressed",
        "/debug/o3_trace/0/event_summary/store_load_forwarding_address_mismatches",
        "/debug/o3_trace/0/event_summary/store_load_forwarding_byte_mismatches",
        "/debug/o3_trace/0/event_summary/lsq_data_latency/samples",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/forwarding_candidates",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/forwarding_matches",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/forwarding_suppressed",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/forwarding_address_mismatches",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/forwarding_byte_mismatches",
        "/debug/o3_trace/0/event_summary/lsq_operation/load/latency/ticks",
        "/debug/o3_trace/0/event_summary/lsq_operation/store/latency/ticks",
        "/debug/o3_trace/0/event_summary/lsq_operation/load_reserved/latency/samples",
        "/debug/o3_trace/0/event_summary/lsq_ordering/acquire",
        "/debug/o3_trace/0/event_summary/lsq_ordering/release",
        "/debug/o3_trace/0/event_summary/lsq_ordering/acquire_release",
        "sim.cpu0.o3.event_summary.store_load_forwarding_candidates",
        "sim.cpu0.o3.event_summary.store_load_forwarding_matches",
        "sim.cpu0.o3.event_summary.store_load_forwarding_suppressed",
        "sim.cpu0.o3.event_summary.store_load_forwarding_address_mismatches",
        "sim.cpu0.o3.event_summary.store_load_forwarding_byte_mismatches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_candidates",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_matches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_suppressed",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_address_mismatches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_byte_mismatches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.latency.ticks",
        "sim.cpu0.o3.event_summary.lsq_operation.store_conditional.latency.ticks",
        "sim.cpu0.o3.event_summary.lsq_operation.atomic.latency.avg_ticks",
        "sim.cpu0.o3.event_summary.lsq_ordering.acquire",
        "sim.cpu0.o3.event_summary.lsq_ordering.acquire_release",
        "sim.cpu0.o3.event_summary.branch_event.taken",
        "sim.cpu0.o3.event_summary.branch_event.not_taken",
        "sim.cpu0.o3.event_summary.branch_event.predicted_taken",
        "sim.cpu0.o3.event_summary.branch_event.predicted_not_taken",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_matches",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_mismatches",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_match_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.resolved_target_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.link_write_kind.call_indirect",
        "sim.cpu0.o3.event_summary.branch_event.without_link_write_kind.direct_unconditional",
        "/debug/o3_trace/0/event_summary/fu_latency_instructions",
        "/debug/o3_trace/0/event_summary/fu_latency_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_max_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_min_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_avg_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_mul/instructions",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_mul/max_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_mul/min_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_mul/avg_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_div/cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_div/max_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_div/min_cycles",
        "/debug/o3_trace/0/event_summary/fu_latency_class/integer_div/avg_cycles",
        "sim.cpu0.o3.event_summary.fu_latency.instructions",
        "sim.cpu0.o3.event_summary.fu_latency.cycles",
        "sim.cpu0.o3.event_summary.fu_latency.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency.avg_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.avg_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.avg_cycles",
        "sim.debug.o3_trace.event.store_load_forwarding_suppressed",
        "sim.debug.o3_trace.event.store_load_forwarding_address_mismatches",
        "sim.debug.o3_trace.event.store_load_forwarding_byte_mismatches",
        "sim.debug.o3_trace.event.branch_predicted_not_taken",
        "sim.debug.o3_trace.event.branch_predicted_targets",
        "sim.debug.o3_trace.event.branch_predicted_target_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_targets",
        "sim.debug.o3_trace.event.branch_targetless_mismatch_without_link_writes",
        "sim.debug.o3_trace.event.branch_targetless_mismatch_squashed_target_without_link_writes",
        "sim.debug.o3_trace.event.branch_wrong_target_squashed_targets",
        "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_without_link_writes",
        "sim.debug.o3_trace.event.branch_wrong_target_squashed_target_link_writes",
        "sim.debug.o3_trace.event.branch_wrong_target_link_writes",
        "sim.debug.o3_trace.event.branch_wrong_target_without_link_writes",
        "sim.debug.o3_trace.event.branch_repair_targetless_mismatches",
        "sim.debug.o3_trace.event.branch_repair_wrong_targets",
        "sim.debug.o3_trace.event.branch_repair_direction_only_mismatches",
        "sim.debug.o3_trace.event.branch_repair_targetless_mismatch_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_repair_wrong_target_kind.indirect_unconditional",
        "sim.debug.o3_trace.event.branch_repair_direction_only_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_wrong_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.iq.branch_insts_issued",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_suppressed",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.iew.branch_mispredicts",
        "sim.host_actions.stats_dump.cpu0.o3.commit.branch_mispredicts",
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.mem_read",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.float_misc",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional",
        "system.cpu.rob.writes",
        "system.cpu.rob.maxOccupancy",
        "system.cpu.rename.renamedOperands",
        "system.cpu.iew.instsToCommit.total",
        "system.cpu.iew.writebackCount.total",
        "system.cpu.iew.producerInst.total",
        "system.cpu.iew.consumerInst.total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "system.cpu.lsq0.maxOccupancy",
        "system.cpu.lsq0.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.storeLoadForwardingByteMismatches",
        "system.cpu.iq.issuedInstType.MemRead",
        "system.cpu.iq.issuedInstType.MemWrite",
        "system.cpu.iq.issuedInstType.IntMult",
        "system.cpu.iq.issuedInstType.IntDiv",
        "system.cpu.iq.issuedInstType.FloatAdd",
        "system.cpu.iq.issuedInstType.FloatCmp",
        "system.cpu.iq.issuedInstType.FloatMisc",
        "system.cpu.iq.issuedInstType.FloatMult",
        "system.cpu.iq.issuedInstType.FloatMultAcc",
        "system.cpu.iq.issuedInstType.FloatDiv",
        "system.cpu.iq.issuedInstType.FloatSqrt",
        "system.cpu.iq.issuedInstType.SimdMult",
        "system.cpu.iq.issuedInstType.SimdDiv",
        "system.cpu.iq.issuedInstType.SimdFloatAdd",
        "system.cpu.iq.issuedInstType.SimdFloatCmp",
        "system.cpu.iq.issuedInstType.SimdFloatMisc",
        "system.cpu.iq.issuedInstType.SimdFloatMult",
        "system.cpu.iq.issuedInstType.SimdFloatMultAcc",
        "system.cpu.iq.issuedInstType.SimdFloatDiv",
        "system.cpu.iq.issuedInstType.SimdFloatSqrt",
        "system.cpu.commit.committedInstType.MemRead",
        "system.cpu.commit.committedInstType.MemWrite",
        "system.cpu.commit.committedInstType.IntMult",
        "system.cpu.commit.committedInstType.IntDiv",
        "system.cpu.commit.committedInstType.FloatAdd",
        "system.cpu.commit.committedInstType.FloatCmp",
        "system.cpu.commit.committedInstType.FloatMisc",
        "system.cpu.commit.committedInstType.FloatMult",
        "system.cpu.commit.committedInstType.FloatMultAcc",
        "system.cpu.commit.committedInstType.FloatDiv",
        "system.cpu.commit.committedInstType.FloatSqrt",
        "system.cpu.commit.committedInstType.SimdMult",
        "system.cpu.commit.committedInstType.SimdDiv",
        "system.cpu.commit.committedInstType.SimdFloatAdd",
        "system.cpu.commit.committedInstType.SimdFloatCmp",
        "system.cpu.commit.committedInstType.SimdFloatMisc",
        "system.cpu.commit.committedInstType.SimdFloatMult",
        "system.cpu.commit.committedInstType.SimdFloatMultAcc",
        "system.cpu.commit.committedInstType.SimdFloatDiv",
        "system.cpu.commit.committedInstType.SimdFloatSqrt",
        "system.cpu1.iq.issuedInstType.MemRead",
        "system.cpu1.iq.issuedInstType.MemWrite",
        "system.cpu1.iq.issuedInstType.IntMult",
        "system.cpu1.iq.issuedInstType.IntDiv",
        "system.cpu1.commit.committedInstType.MemRead",
        "system.cpu1.commit.committedInstType.MemWrite",
        "system.cpu1.commit.committedInstType.IntMult",
        "system.cpu1.commit.committedInstType.IntDiv",
        "/cores/0/o3_runtime/iq/issued_inst_type/int_mul",
        "/cores/0/o3_runtime/iew/writeback_count",
        "/cores/0/o3_runtime/commit/committed_inst_type/vector_float_misc",
        "/cores/0/o3_runtime/event_window/max_rob_occupancy/tick",
        "/cores/0/o3_runtime/rob/allocations",
        "/cores/0/o3_runtime/rename/writes",
        "/cores/0/o3_runtime/snapshot/rob/count",
        "/cores/0/o3_runtime/snapshot/rename_map/entries",
        "/cores/0/o3_runtime/lsq/operation/load/latency/ticks",
        "/cores/0/o3_runtime/lsq/operation/load/forwarding_matches",
        "/cores/0/o3_runtime/lsq/ordering/acquire_release",
        "/debug/o3_trace/0/lsq/data_latency/samples",
        "/debug/o3_trace/0/lsq/operation/store_conditional/count",
        "/debug/o3_trace/0/lsq/ordering/acquire_release",
        "/debug/o3_trace/0/commit/committed_inst_type/int_mul",
        "/debug/o3_trace/0/commit/committed_inst_type/vector_float_misc",
        "/debug/o3_trace/0/iq/issued_inst_type/int_mul",
        "/debug/o3_trace/0/iew/writeback_count",
        "/debug/o3_trace/0/rob/allocations",
        "/debug/o3_trace/0/rename/writes",
        "/debug/o3_trace/0/fu_latency_class/float_mul/instructions",
        "/debug/o3_trace/0/fu_latency_class/vector_float_div/cycles",
        "/debug/o3_trace/0/fu_latency_class/vector_integer_mul/instructions",
        "/debug/o3_trace/0/checkpoint_restore/execution_mode_authority/mode/detailed",
        "/debug/o3_trace/0/checkpoint_restore/execution_mode_authority/target/cpu0/mode/detailed",
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.mode.detailed",
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.functional",
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.timing",
        "sim.debug.o3_trace.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
        "sim.debug.o3_trace.cpu.cpu0.checkpoint_restore.execution_mode_authority.target.cpu1.mode.detailed",
        "sim.debug.o3_trace.cpu.cpu1.checkpoint_restore.execution_mode_authority.target.cpu0.mode.detailed",
        "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        "/cores/0/o3_runtime/branch_event/branches",
        "/cores/0/o3_runtime/branch_event/taken",
        "/cores/0/o3_runtime/branch_event/not_taken",
        "/cores/0/o3_runtime/branch_event/predicted_taken",
        "/cores/0/o3_runtime/branch_event/predicted_not_taken",
        "/cores/0/o3_runtime/branch_event/predicted_targets",
        "/cores/0/o3_runtime/branch_event/predicted_target_matches",
        "/cores/0/o3_runtime/branch_event/predicted_target_mismatches",
        "/cores/0/o3_runtime/branch_event/resolved_targets",
        "/cores/0/o3_runtime/branch_event/mispredictions",
        "/cores/0/o3_runtime/branch_event/kind/return",
        "/cores/0/o3_runtime/branch_event/not_taken_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/predicted_taken_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/predicted_not_taken_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/predicted_target_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/predicted_target_match_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/predicted_target_mismatch_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/misprediction_kind/call_indirect",
        "/cores/0/o3_runtime/branch_event/misprediction_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/link_writes",
        "/cores/0/o3_runtime/branch_event/without_link_writes",
        "/cores/0/o3_runtime/branch_event/without_link_write_kind/call_indirect",
        "/cores/0/o3_runtime/branch_event/without_link_write_kind/direct_conditional",
        "/cores/0/o3_runtime/branch_event/squashes",
        "/cores/0/o3_runtime/branch_event/squashed_targets",
        "/cores/0/o3_runtime/branch_event/squashed_targets_with_link_writes",
        "/cores/0/o3_runtime/branch_event/squashed_targets_without_link_writes",
        "/cores/0/o3_runtime/branch_event/squashed_target_kind/call_indirect",
        "/cores/0/o3_runtime/branch_event/squashed_target_kind/direct_unconditional",
        "/cores/0/o3_runtime/branch_event/squashed_target_link_write_kind/call_indirect",
        "/cores/0/o3_runtime/branch_event/squashed_target_without_link_write_kind/return",
        "/debug/o3_trace/0/branch_event/branches",
        "/debug/o3_trace/0/branch_event/not_taken_kind/direct_conditional",
        "/debug/o3_trace/0/branch_event/without_link_write_kind/direct_conditional",
        "/debug/o3_trace/0/branch_event/mispredictions",
        "/debug/o3_trace/0/branch_event/misprediction_kind/call_indirect",
        "/debug/o3_trace/0/branch_event/misprediction_kind/direct_unconditional",
        "/debug/o3_trace/0/branch_event/squashed_target_kind/call_indirect",
        "/debug/o3_trace/0/branch_event/squashed_target_kind/direct_unconditional",
        "/debug/o3_trace/0/branch_event/squashed_target_link_write_kind/call_indirect",
        "/debug/o3_trace/0/branch_repair/wrong_targets",
        "/debug/o3_trace/0/branch_repair/direction_only_kind/direct_unconditional",
        "/debug/o3_trace/0/branch_repair/targetless_mismatch_kind/direct_conditional",
        "/debug/o3_trace/0/branch_direction_mismatch/mismatches",
        "/debug/o3_trace/0/branch_direction_mismatch/link_write_kind/call_indirect",
        "/debug/o3_trace/0/branch_direction_mismatch/without_link_write_kind/direct_conditional",
        "/debug/o3_trace/0/branch_direction_mismatch/squashed_target_without_link_write_kind/direct_unconditional",
        "/debug/o3_trace/0/branch_target_mismatch/wrong_targets",
        "/debug/o3_trace/0/branch_target_mismatch/wrong_target_squashed_target_link_write_kind/call_indirect",
        "/debug/o3_trace/0/branch_target_mismatch/wrong_target_squashed_target_without_link_write_kind/indirect_unconditional",
        "/debug/o3_trace/0/branch_target_mismatch/targetless_mismatch_squashed_target_without_link_write_kind/direct_conditional",
        "sim.cpu0.o3.branch_event.branches",
        "sim.cpu0.o3.branch_event.taken",
        "sim.cpu0.o3.branch_event.not_taken",
        "sim.cpu0.o3.branch_event.predicted_taken",
        "sim.cpu0.o3.branch_event.predicted_not_taken",
        "sim.cpu0.o3.branch_event.predicted_targets",
        "sim.cpu0.o3.branch_event.predicted_target_matches",
        "sim.cpu0.o3.branch_event.predicted_target_mismatches",
        "sim.cpu0.o3.branch_event.resolved_targets",
        "sim.cpu0.o3.branch_event.mispredictions",
        "sim.cpu0.o3.branch_event.kind.return",
        "sim.cpu0.o3.branch_event.not_taken_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.predicted_taken_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.predicted_not_taken_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.predicted_target_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.predicted_target_match_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.predicted_target_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.misprediction_kind.call_indirect",
        "sim.cpu0.o3.branch_event.misprediction_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.link_writes",
        "sim.cpu0.o3.branch_event.without_link_writes",
        "sim.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
        "sim.cpu0.o3.branch_event.without_link_write_kind.direct_conditional",
        "sim.cpu0.o3.branch_event.squashes",
        "sim.cpu0.o3.branch_event.squashed_targets",
        "sim.cpu0.o3.branch_event.squashed_targets_with_link_writes",
        "sim.cpu0.o3.branch_event.squashed_targets_without_link_writes",
        "sim.cpu0.o3.branch_event.squashed_target_kind.call_indirect",
        "sim.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
        "sim.cpu0.o3.branch_event.squashed_target_link_write_kind.call_indirect",
        "sim.cpu0.o3.branch_event.squashed_target_without_link_write_kind.return",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.branches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.not_taken_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_direct",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.call_direct",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_link_write_kind.call_direct",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
        "sim.cpu0.o3.branch_repair_targetless_mismatches",
        "sim.cpu0.o3.branch_repair_direction_only_mismatches",
        "sim.cpu0.o3.branch_repair_wrong_targets",
        "system.cpu.iew.branchRepair.targetlessMismatch",
        "system.cpu.iew.branchRepair.directionOnly",
        "system.cpu.iew.branchRepair.wrongTarget",
        "system.cpu.iew.branchRepair.total",
        "system.cpu.iew.predictedTakenIncorrect",
        "system.cpu.iew.predictedNotTakenIncorrect",
        "system.cpu.iew.branchMispredicts",
        "system.cpu.commit.branchMispredicts",
        "system.cpu.ftq.squashes",
        "system.cpu.ftq.squashes_0::CallDirect",
        "system.cpu.ftq.squashes_0::CallIndirect",
        "system.cpu.ftq.squashes_0::DirectCond",
        "system.cpu.ftq.squashes_0::DirectUncond",
        "system.cpu.ftq.squashes_0::total",
        "system.cpu.ftq.squashedTargets",
        "system.cpu.ftq.squashedTargets_0::CallDirect",
        "system.cpu.ftq.squashedTargets_0::CallIndirect",
        "system.cpu.ftq.squashedTargets_0::DirectUncond",
        "system.cpu.ftq.squashedTargets_0::total",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallDirect",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::total",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
        "system.cpu.ftq.squashedTargetsWithLinkWrites",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites",
        "system.cpu.iew.branchRepair_0::TargetlessMismatch",
        "system.cpu.iew.branchRepair_0::DirectionOnly",
        "system.cpu.iew.branchRepair_0::WrongTarget",
        "system.cpu.iew.branchRepair_0::total",
        "sim.cpu0.o3.iew.branch_mispredicts",
        "sim.cpu0.o3.commit.branch_mispredicts",
        "sim.cpu0.o3.iew.writeback_rate_ppm",
        "sim.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "system.cpu.iq.instsIssued",
        "system.cpu.iq.memInstsIssued",
        "system.cpu.lsq0.forwLoads",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.load_forwarding_suppressed",
        "sim.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
        "sim.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
        "/cores/0/o3_runtime/lsq_operation_load_forwarding_candidates",
        "/cores/0/o3_runtime/lsq_operation_load_forwarding_matches",
        "/cores/0/o3_runtime/lsq_operation_load_forwarding_suppressed",
        "/cores/0/o3_runtime/lsq_operation_load_forwarding_address_mismatches",
        "/cores/0/o3_runtime/lsq_operation_load_forwarding_byte_mismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
        "system.cpu.lsq0.dataResponse.samples",
        "system.cpu.lsq0.dataResponse.totalLatency",
        "system.cpu.lsq0.dataResponse.maxLatency",
        "system.cpu.lsq0.dataResponse.minLatency",
        "system.cpu.lsq0.dataResponse.avgLatency",
        "system.cpu.lsq0.operation.load",
        "system.cpu.lsq0.operation.storeConditional",
        "system.cpu.lsq0.operation.total",
        "system.cpu.lsq0.ordering.acquireRelease",
        "system.cpu.lsq0.ordering.total",
        "system.cpu.lsq0.dataResponse.load.totalLatency",
        "system.cpu.lsq0.dataResponse.storeConditional.totalLatency",
        "system.cpu.lsq0.dataResponse.atomic.avgLatency",
        "system.cpu.lsq0.dataResponse.floatLoad.totalLatency",
        "system.cpu.lsq0.dataResponse.floatStore.totalLatency",
        "system.cpu.lsq0.dataResponse.vectorLoad.totalLatency",
        "system.cpu.lsq0.dataResponse.vectorStore.totalLatency",
        "sim.cpu0.o3.iq.branch_insts_issued",
        "system.cpu.iq.branchInstsIssued",
        "sim.cpu0.o3.instructions",
        "sim.cpu0.o3.rob_allocations",
        "sim.cpu0.o3.lsq_loads",
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "/cores/0/o3_runtime/instructions",
        "/cores/0/o3_runtime/fu_integer_mul_instructions",
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        "system.cpu.commit.committedInstType_0::MemRead",
        "system.cpu.commit.committedInstType_0::IntMult",
        "system.cpu.commit.committedInstType_0::FloatMisc",
        "sim.debug.o3_trace.event.branch_direction_mismatches",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_targets",
        "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_writes",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_writes",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_link_writes",
        "sim.debug.o3_trace.event.branch_direction_mismatch_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_kind.direct_unconditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_kind.indirect_unconditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_link_write_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_kind.direct_unconditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_write_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_without_link_write_kind.direct_unconditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_write_kind.direct_conditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_without_link_write_kind.direct_unconditional",
        "sim.debug.o3_trace.event.branch_direction_mismatch_squashed_target_link_write_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_resolved_targets",
        "sim.debug.o3_trace.event.branch_squashed_targets",
        "sim.debug.o3_trace.event.branch_squashed_target_link_writes",
        "sim.debug.o3_trace.event.branch_squashed_target_without_link_writes",
        "sim.debug.o3_trace.event.branch_squashed_target_link_write_kind.call_indirect",
        "sim.debug.o3_trace.event.branch_squashed_target_without_link_write_kind.indirect_unconditional",
        "sim.debug.host_action_trace.records",
        "/fabric/wait_for_edge_count",
        "wait_for_edge_kind_windows",
        "sim.memory.fabric.wait_for.edges",
        "sim.memory.fabric.wait_for.kind.queue.edges",
        "system.l2.overallMshrHits",
        "system.l3.overallMshrMissRate",
        "demandMshrMisses",
        "demandMshrMissRate",
        "pfUnused",
        "pfHitInCache",
        "pfHitInMSHR",
        "pfHitInWB",
        "pfLate",
        "pfUsefulSpanPage",
        "pfUsefulButMiss",
        "accuracy_ppm",
        "coverage_ppm",
    ] {
        assert!(
            contents.contains(required),
            "gem5-to-rem6-migration.md is missing required anchor `{required}`"
        );
    }
}

#[test]
fn gem5_migration_sections_are_auditable() {
    let repo_root = repo_root();
    let path = repo_root.join("docs/architecture/gem5-to-rem6-migration.md");
    let contents = fs::read_to_string(&path).unwrap();

    for section_heading in ["## Component Progress", "## External Adapter Migration"] {
        let section = markdown_section(&contents, section_heading)
            .unwrap_or_else(|| panic!("missing section `{section_heading}`"));
        let subsections = markdown_subsections(section);
        assert!(
            !subsections.is_empty(),
            "{section_heading} should contain auditable component subsections"
        );

        for (heading, body) in subsections {
            let score = migration_heading_score(heading).unwrap_or_else(|| {
                panic!("`{heading}` should end with a numeric percent and migration bucket")
            });
            let calculation = score_calculation(body)
                .unwrap_or_else(|| panic!("`{heading}` is missing a parseable score calculation"));
            let completed = count_checkbox_items(body, "- [x]");
            let total = completed + count_checkbox_items(body, "- [ ]");
            let raw_percent = rounded_percent(completed, total);
            let bucket = migration_score_bucket(&calculation.bucket).unwrap_or_else(|| {
                panic!(
                    "`{heading}` uses unknown bucket `{}` in score calculation",
                    calculation.bucket
                )
            });

            assert_eq!(
                score.bucket,
                calculation.bucket.as_str(),
                "`{heading}` header bucket should match its score calculation"
            );
            assert_eq!(
                calculation.completed, completed,
                "`{heading}` score calculation should match completed checklist items"
            );
            assert_eq!(
                calculation.total, total,
                "`{heading}` score calculation should match total checklist items"
            );
            assert_eq!(
                calculation.raw_percent, raw_percent,
                "`{heading}` score calculation should match rounded checklist percentage"
            );
            assert_eq!(
                score.percent,
                raw_percent.min(bucket.max),
                "`{heading}` header percent should be the raw percentage after the bucket cap"
            );
            assert!(
                bucket.contains(score.percent),
                "`{heading}` header percent should fit bucket `{}`",
                bucket.name
            );
            assert!(
                total > 0,
                "`{heading}` should have at least one checklist item"
            );
            assert!(
                migration_score_bucket(score.bucket).is_some(),
                "`{heading}` should end with a numeric percent and migration bucket"
            );
            assert!(
                body.contains("% raw") && body.contains("bucket cap"),
                "`{heading}` should state raw percentage and bucket cap"
            );
            for required in ["**Migrated:**", "**Not migrated:**", "**Next evidence:**"] {
                assert!(
                    body.contains(required),
                    "`{heading}` is missing required field `{required}`"
                );
            }
        }
    }

    let table = markdown_section(&contents, "## Test Migration Ledger")
        .expect("missing test migration ledger");
    let mut row_count = 0;
    for cells in markdown_table_rows(table) {
        if cells.first().is_none_or(|cell| !cell.starts_with('`')) {
            continue;
        }
        row_count += 1;
        assert_eq!(
            cells.len(),
            5,
            "test migration ledger rows should have five cells"
        );
        let score = migration_score(cells[2]).unwrap_or_else(|| {
            panic!(
                "test migration ledger row `{}` has invalid row score",
                cells[0]
            )
        });
        let bucket = migration_score_bucket(score.bucket).unwrap_or_else(|| {
            panic!(
                "test migration ledger row `{}` uses unknown bucket `{}`",
                cells[0], score.bucket
            )
        });
        assert!(
            bucket.contains(score.percent),
            "test migration ledger row `{}` percent should fit bucket `{}`",
            cells[0],
            bucket.name
        );
        if cells[0] == "`tests/gem5/stats`" {
            assert_eq!(
                cells[2], "65% representative",
                "`tests/gem5/stats` score should not change without explicit score evidence"
            );
        }
    }
    assert!(
        row_count > 0,
        "test migration ledger should contain scored gem5 test-anchor rows"
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

fn architecture_doc_paths(repo_root: &Path) -> Vec<String> {
    let architecture_dir = repo_root.join("docs/architecture");
    let mut paths = Vec::new();
    for entry in fs::read_dir(&architecture_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(
                path.strip_prefix(repo_root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    paths.sort();
    paths
}

fn markdown_section<'a>(contents: &'a str, heading: &str) -> Option<&'a str> {
    let start = contents.find(heading)?;
    let after_heading = start + heading.len();
    let after = &contents[after_heading..];
    let end = after.find("\n## ").unwrap_or(after.len());
    Some(after[..end].trim())
}

fn markdown_subsections(section: &str) -> Vec<(&str, &str)> {
    let mut headings = Vec::new();
    let mut offset = 0;
    while offset < section.len() {
        let rest = &section[offset..];
        let line_end = rest
            .find('\n')
            .map(|relative| offset + relative)
            .unwrap_or(section.len());
        let line = &section[offset..line_end];
        if let Some(heading) = line.strip_prefix("### ") {
            headings.push((offset, heading.trim()));
        }
        offset = if line_end < section.len() {
            line_end + 1
        } else {
            section.len()
        };
    }

    let mut subsections = Vec::new();
    for (index, (start, heading)) in headings.iter().enumerate() {
        let body_start = section[*start..]
            .find('\n')
            .map(|relative| *start + relative + 1)
            .unwrap_or(section.len());
        let body_end = headings
            .get(index + 1)
            .map(|(next_start, _)| *next_start)
            .unwrap_or(section.len());
        subsections.push((*heading, &section[body_start..body_end]));
    }
    subsections
}

fn markdown_table_rows(section: &str) -> Vec<Vec<&str>> {
    section
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with('|') || !line.ends_with('|') {
                return None;
            }
            Some(
                line.trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

fn migration_heading_score(heading: &str) -> Option<MigrationScore<'_>> {
    let (_, score) = heading.rsplit_once(" - ")?;
    migration_score(score)
}

fn migration_score(score: &str) -> Option<MigrationScore<'_>> {
    let (percent, bucket) = score.trim().split_once("% ")?;
    let percent = percent.trim().parse::<u8>().ok()?;
    let bucket = bucket.trim();
    (percent <= 100 && migration_score_bucket(bucket).is_some())
        .then_some(MigrationScore { percent, bucket })
}

fn score_calculation(body: &str) -> Option<ScoreCalculation> {
    let prefix = "**Score calculation:**";
    let start = body.find(prefix)?;
    let paragraph = body[start + prefix.len()..]
        .trim_start()
        .split("\n\n")
        .next()?;
    let paragraph = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
    let (fraction, after_fraction) =
        paragraph.split_once(" items have executable evidence, or ")?;
    let (completed, total) = fraction.split_once(" of ")?;
    let (raw_percent, after_percent) = after_fraction.split_once("% raw")?;
    let (_, after_bucket) = after_percent.split_once("The bucket cap is ")?;
    let bucket_end = after_bucket
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
        .unwrap_or(after_bucket.len());
    let bucket = &after_bucket[..bucket_end];

    Some(ScoreCalculation {
        completed: completed.trim().parse().ok()?,
        total: total.trim().parse().ok()?,
        raw_percent: raw_percent.trim().parse().ok()?,
        bucket: bucket.to_string(),
    })
}

fn count_checkbox_items(body: &str, marker: &str) -> usize {
    body.lines()
        .filter(|line| line.trim_start().starts_with(marker))
        .count()
}

fn rounded_percent(completed: usize, total: usize) -> u8 {
    assert!(
        total > 0,
        "migration score calculation needs a nonzero item count"
    );
    (((completed * 100) + (total / 2)) / total) as u8
}

fn migration_score_bucket(name: &str) -> Option<MigrationScoreBucket> {
    MIGRATION_SCORE_BUCKETS
        .iter()
        .copied()
        .find(|bucket| bucket.name == name)
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
