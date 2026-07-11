use std::fs;
use std::path::{Path, PathBuf};

const MAX_FACADE_LINES: usize = 1250;
const MAX_CONFIG_ROOT_LINES: usize = 1700;
const MAX_HOST_ACTIONS_ROOT_LINES: usize = 1200;
const MAX_HOST_ACTIONS_O3_STATS_DUMP_ALIASES_LINES: usize = 800;
const MAX_M5_HOST_ACTIONS_ROOT_LINES: usize = 5000;
const MAX_M5_HOST_ACTIONS_O3_ROOT_LINES: usize = 4500;
const MAX_M5_HOST_ACTIONS_O3_LSQ_ROOT_LINES: usize = 1400;
const MAX_M5_HOST_ACTIONS_O3_BRANCH_LINES: usize = 1600;
const MAX_M5_HOST_ACTIONS_O3_MODULE_LINES: usize = 1800;
const MAX_M5_HOST_ACTIONS_O3_RUNTIME_LINES: usize = 1600;
const MAX_STATS_OUTPUT_CPU_LINES: usize = 1700;
const MAX_O3_RUNTIME_STATS_LINES: usize = 1700;
const MAX_REM6_CPU_O3_RUNTIME_ROOT_LINES: usize = 1700;
const MAX_REM6_SYSTEM_O3_RUNTIME_STATS_MODULE_LINES: usize = 1800;
const MAX_STATS_COMPAT_ROOT_LINES: usize = 17_500;
const MAX_STATS_COMPAT_MODULE_LINES: usize = 1800;
const MAX_STATS_COMPAT_IN_ORDER_BRANCH_PREDICTION_MATRIX_LINES: usize = 1100;
const MAX_SOURCE_POLICY_DRIVER_LINES: usize = 1500;
const MAX_SOURCE_LINES: usize = 1800;
const MAX_RISCV_SBI_SMOKE_LINES: usize = 1500;
const MAX_ARCHITECTURE_OVERVIEW_LINES: usize = 600;
const REQUIRED_MIGRATION_LEDGER_LINES: usize = 1200;
const MAX_ARCHITECTURE_README_LINES: usize = 80;
const CORE_TEST_ANCHORS: &str = include_str!("source_policy/core_test_anchors.txt");
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
fn source_policy_driver_keeps_anchor_data_out_of_root() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/source_policy.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_SOURCE_POLICY_DRIVER_LINES,
        "tests/source_policy.rs should keep anchor data in focused modules, but it has {lines} lines"
    );
}

#[test]
fn rem6_cpu_o3_runtime_root_keeps_headroom() {
    for (relative_path, maximum) in [
        (
            "../rem6-cpu/src/o3_runtime.rs",
            MAX_REM6_CPU_O3_RUNTIME_ROOT_LINES,
        ),
        (
            "../rem6-cpu/src/o3_runtime_snapshot_entries.rs",
            MAX_SOURCE_LINES,
        ),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative_path} should keep headroom for executable O3 evidence, but it has {lines} lines"
        );
    }
}

#[test]
fn rem6_system_o3_runtime_stats_modules_stay_focused() {
    let stats_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../rem6-system/src/riscv_o3_runtime_stats");
    let mut oversized = Vec::new();

    for path in rust_source_files(&stats_dir) {
        let lines = line_count(&path);
        if lines > MAX_REM6_SYSTEM_O3_RUNTIME_STATS_MODULE_LINES {
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
        "rem6-system O3 runtime stats modules exceed {MAX_REM6_SYSTEM_O3_RUNTIME_STATS_MODULE_LINES} lines: {}",
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
fn cli_host_action_o3_stats_dump_aliases_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/host_actions.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let module_path = crate_dir.join("src/host_actions/o3_stats_dump_aliases.rs");

    assert!(
        root.contains("mod o3_stats_dump_aliases;"),
        "src/host_actions.rs must declare the focused O3 stats-dump alias module"
    );
    assert!(
        module_path.exists(),
        "O3 stats-dump alias translation belongs in src/host_actions/o3_stats_dump_aliases.rs"
    );
    let module = fs::read_to_string(&module_path).unwrap();

    let root_lines = line_count(&root_path);
    assert!(
        root_lines <= MAX_HOST_ACTIONS_ROOT_LINES,
        "src/host_actions.rs should delegate O3 stats-dump alias translation, but it has {root_lines} lines"
    );
    let module_lines = line_count(&module_path);
    assert!(
        module_lines <= MAX_HOST_ACTIONS_O3_STATS_DUMP_ALIASES_LINES,
        "src/host_actions/o3_stats_dump_aliases.rs exceeds {MAX_HOST_ACTIONS_O3_STATS_DUMP_ALIASES_LINES} lines: {module_lines}"
    );

    for anchor in [
        "fn append_o3_stats_dump_rate_alias_samples",
        "fn append_o3_stats_dump_branch_mismatch_alias_samples",
        "fn append_o3_stats_dump_lsq_data_response_alias_samples",
        "fn stats_dump_sample_is_active",
    ] {
        assert!(
            module.contains(anchor),
            "src/host_actions/o3_stats_dump_aliases.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "src/host_actions.rs still owns `{anchor}`"
        );
    }
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
fn cli_stats_output_o3_runtime_stays_focused() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/stats_output/o3_runtime.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_O3_RUNTIME_STATS_LINES,
        "src/stats_output/o3_runtime.rs should delegate O3 stat families to focused modules, but it has {lines} lines"
    );
}

#[test]
fn stats_compat_root_keeps_current_ratchet() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/stats_compat.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_STATS_COMPAT_ROOT_LINES,
        "tests/cli_run/stats_compat.rs exceeds the current decomposition ratchet: {lines} lines"
    );
}

#[test]
fn stats_compat_in_order_branch_prediction_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/stats_compat.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert!(
        root.contains("include!(\"stats_compat/in_order_branch_prediction_matrix.rs\");"),
        "tests/cli_run/stats_compat.rs must include the focused in-order branch prediction matrix"
    );

    let module_path =
        crate_dir.join("tests/cli_run/stats_compat/in_order_branch_prediction_matrix.rs");
    assert!(
        module_path.exists(),
        "the in-order branch prediction compatibility matrix belongs in tests/cli_run/stats_compat/in_order_branch_prediction_matrix.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let module_lines = module.lines().count();
    assert!(
        module_lines <= MAX_STATS_COMPAT_IN_ORDER_BRANCH_PREDICTION_MATRIX_LINES,
        "tests/cli_run/stats_compat/in_order_branch_prediction_matrix.rs exceeds {MAX_STATS_COMPAT_IN_ORDER_BRANCH_PREDICTION_MATRIX_LINES} lines: {module_lines}"
    );

    for anchor in [
        "fn rem6_run_stats_emit_conditional_branch_predicted_taken_from_execution",
        "fn rem6_run_text_stats_emit_gem5_branch_prediction_aliases",
    ] {
        assert!(
            module.contains(anchor),
            "tests/cli_run/stats_compat/in_order_branch_prediction_matrix.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "tests/cli_run/stats_compat.rs still owns `{anchor}`"
        );
    }

    for family in [
        "in-order-conditional-branch-predicted-taken",
        "in-order-branch-prediction-aliases",
    ] {
        assert!(
            module.contains(family),
            "tests/cli_run/stats_compat/in_order_branch_prediction_matrix.rs is missing matrix binary family `{family}`"
        );
        assert!(
            !root.contains(family),
            "tests/cli_run/stats_compat.rs still owns CPU branch binary family `{family}`"
        );
    }
}

#[test]
fn stats_compat_dram_aliases_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/stats_compat.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert!(
        root.contains("include!(\"stats_compat/dram.rs\");"),
        "tests/cli_run/stats_compat.rs must include the focused DRAM test file"
    );

    let module_path = crate_dir.join("tests/cli_run/stats_compat/dram.rs");
    assert!(
        module_path.exists(),
        "DRAM stats compatibility tests belong in tests/cli_run/stats_compat/dram.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let module_lines = module.lines().count();
    assert!(
        module_lines <= MAX_STATS_COMPAT_MODULE_LINES,
        "tests/cli_run/stats_compat/dram.rs exceeds {MAX_STATS_COMPAT_MODULE_LINES} lines: {module_lines}"
    );

    for anchor in [
        "fn rem6_run_text_stats_emit_gem5_mem_ctrl_bandwidth_aliases",
        "fn rem6_run_json_stats_omit_text_only_gem5_dram_interface_page_hit_rate_alias",
    ] {
        assert!(
            module.contains(anchor),
            "tests/cli_run/stats_compat/dram.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "tests/cli_run/stats_compat.rs still owns `{anchor}`"
        );
    }

    for family in [
        "gem5_mem_ctrl",
        "gem5_nvm_interface",
        "gem5_dram_interface",
        "gem5-mem-ctrl",
        "gem5-nvm-interface",
        "gem5-dram-interface",
    ] {
        assert!(
            module.contains(family),
            "tests/cli_run/stats_compat/dram.rs is missing DRAM family `{family}`"
        );
        assert!(
            !root.contains(family),
            "tests/cli_run/stats_compat.rs still owns DRAM family `{family}`"
        );
    }
}

#[test]
fn stats_compat_cache_aliases_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/stats_compat.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert!(
        root.contains("include!(\"stats_compat/cache.rs\");"),
        "tests/cli_run/stats_compat.rs must include the focused cache test file"
    );

    let module_path = crate_dir.join("tests/cli_run/stats_compat/cache.rs");
    assert!(
        module_path.exists(),
        "cache stats compatibility tests belong in tests/cli_run/stats_compat/cache.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let module_lines = module.lines().count();
    assert!(
        module_lines <= MAX_STATS_COMPAT_MODULE_LINES,
        "tests/cli_run/stats_compat/cache.rs exceeds {MAX_STATS_COMPAT_MODULE_LINES} lines: {module_lines}"
    );

    for anchor in [
        "fn rem6_run_text_stats_omit_ambiguous_gem5_l1_cache_aliases_for_multicore",
        "fn rem6_run_json_stats_emit_gem5_l1_icache_prefetcher_pf_useful_alias",
    ] {
        assert!(
            module.contains(anchor),
            "tests/cli_run/stats_compat/cache.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "tests/cli_run/stats_compat.rs still owns `{anchor}`"
        );
    }

    let root_test_names = root
        .lines()
        .filter(|line| line.contains("fn rem6_run_"))
        .collect::<Vec<_>>();
    let module_test_names = module
        .lines()
        .filter(|line| line.contains("fn rem6_run_"))
        .collect::<Vec<_>>();
    for family in [
        "gem5_l1_cache",
        "gem5_l2_cache",
        "gem5_l3_cache",
        "gem5_ruby_network",
        "gem5_l1_prefetcher",
        "gem5_l1_icache_prefetcher",
        "gem5_l1_demand",
    ] {
        assert!(
            module_test_names.iter().any(|name| name.contains(family)),
            "tests/cli_run/stats_compat/cache.rs is missing cache test family `{family}`"
        );
        assert!(
            root_test_names.iter().all(|name| !name.contains(family)),
            "tests/cli_run/stats_compat.rs still owns cache test family `{family}`"
        );
    }

    for family in [
        "gem5-l1-cache",
        "gem5-l2-cache",
        "gem5-l3-cache",
        "gem5-ruby-network",
        "gem5-l1-prefetcher",
        "gem5-l1-icache-prefetcher",
        "gem5-l1-demand",
    ] {
        assert!(
            module.contains(family),
            "tests/cli_run/stats_compat/cache.rs is missing cache binary family `{family}`"
        );
        assert!(
            !root.contains(family),
            "tests/cli_run/stats_compat.rs still owns cache binary family `{family}`"
        );
    }

    for marker in [
        "system.cpu.dcache",
        "system.cpu.icache",
        "system.l2",
        "system.l3",
        "system.ruby.network",
        "prefetcher",
    ] {
        assert!(
            module.contains(marker),
            "tests/cli_run/stats_compat/cache.rs is missing cache alias marker `{marker}`"
        );
        assert!(
            !root.contains(marker),
            "tests/cli_run/stats_compat.rs still owns cache alias marker `{marker}`"
        );
    }
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
fn cli_m5_host_actions_o3_branch_keeps_headroom() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions/o3/branch.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_M5_HOST_ACTIONS_O3_BRANCH_LINES,
        "tests/cli_run/m5_host_actions/o3/branch.rs should split large branch evidence helpers before reaching the child-module ceiling, but it has {lines} lines"
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
fn cli_m5_host_actions_o3_runtime_keeps_headroom() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cli_run/m5_host_actions/o3/runtime.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_M5_HOST_ACTIONS_O3_RUNTIME_LINES,
        "tests/cli_run/m5_host_actions/o3/runtime.rs should split large runtime evidence helpers before reaching the child-module ceiling, but it has {lines} lines"
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

    for required in CORE_TEST_ANCHORS.lines().filter(|line| !line.is_empty()) {
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
