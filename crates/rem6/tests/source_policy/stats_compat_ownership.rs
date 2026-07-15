use std::{fs, path::Path};

use super::{has_exact_trimmed_source_line, line_count};

const MAX_STATS_COMPAT_ROOT_LINES: usize = 15_800;
const MAX_STATS_COMPAT_MODULE_LINES: usize = 1800;
const MAX_STATS_COMPAT_IN_ORDER_BRANCH_PREDICTION_MATRIX_LINES: usize = 1100;
const MAX_STATS_COMPAT_SELECTED_BRANCH_PREDICTION_MATRIX_LINES: usize = 1000;
const MAX_STATS_COMPAT_MASKED_UNIT_STRIDE_VECTOR_MEMORY_LINES: usize = 1000;

#[test]
fn exact_trimmed_source_line_rejects_commented_include() {
    let include = "include!(\"stats_compat/selected_branch_predictor_matrix.rs\");";

    assert!(has_exact_trimmed_source_line(
        "  include!(\"stats_compat/selected_branch_predictor_matrix.rs\");\n",
        include,
    ));
    assert!(!has_exact_trimmed_source_line(
        "// include!(\"stats_compat/selected_branch_predictor_matrix.rs\");\n",
        include,
    ));
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
fn stats_compat_selected_branch_prediction_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/stats_compat.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let include = "include!(\"stats_compat/selected_branch_predictor_matrix.rs\");";

    assert!(
        has_exact_trimmed_source_line(&root, include),
        "tests/cli_run/stats_compat.rs must include the focused selected branch predictor matrix"
    );

    let module_path =
        crate_dir.join("tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs");
    assert!(
        module_path.exists(),
        "selected branch predictor compatibility tests belong in tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let module_lines = module.lines().count();
    assert!(
        module_lines <= MAX_STATS_COMPAT_SELECTED_BRANCH_PREDICTION_MATRIX_LINES,
        "tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs exceeds {MAX_STATS_COMPAT_SELECTED_BRANCH_PREDICTION_MATRIX_LINES} lines: {module_lines}"
    );
    let module_function_names = module.lines().filter_map(|line| {
        let signature = line.trim_start().strip_prefix("fn ")?;
        signature.split_once('(').map(|(name, _)| name)
    });
    for function_name in module_function_names {
        let definition = format!("fn {function_name}(");
        assert!(
            !root.contains(&definition),
            "tests/cli_run/stats_compat.rs still owns `{definition}`"
        );
    }

    for anchor in [
        "fn rem6_run_stats_emit_in_order_nested_branch_speculation_rollback",
        "fn rem6_run_stats_emit_selected_branch_predictor_family_rollback_counters",
        "fn rem6_run_stats_use_selected_multiperspective_perceptron_for_fetch_steering",
        "fn rem6_run_stats_use_retired_tage_sc_l_training_for_later_fetch_steering",
    ] {
        assert!(
            module.contains(anchor),
            "tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs is missing `{anchor}`"
        );
        assert!(
            !root.contains(anchor),
            "tests/cli_run/stats_compat.rs still owns `{anchor}`"
        );
    }

    for family in [
        "in-order-nested-branch-speculation",
        "in-order-gshare-branch-steering",
        "in-order-multiperspective-perceptron-branch-steering",
        "in-order-tage-sc-l-training-feedback",
    ] {
        assert!(
            module.contains(family),
            "tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs is missing matrix binary family `{family}`"
        );
        assert!(
            !root.contains(family),
            "tests/cli_run/stats_compat.rs still owns selected predictor binary family `{family}`"
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
fn stats_compat_masked_unit_stride_vector_memory_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/stats_compat.rs");
    let root = fs::read_to_string(&root_path).unwrap();
    let include = "include!(\"stats_compat/masked_unit_stride_vector_memory.rs\");";

    assert!(
        has_exact_trimmed_source_line(&root, include),
        "tests/cli_run/stats_compat.rs must include the focused masked unit-stride vector memory tests with an exact non-commented include"
    );

    let module_path =
        crate_dir.join("tests/cli_run/stats_compat/masked_unit_stride_vector_memory.rs");
    assert!(
        module_path.exists(),
        "masked unit-stride vector memory tests belong in tests/cli_run/stats_compat/masked_unit_stride_vector_memory.rs"
    );
    let module = fs::read_to_string(module_path).unwrap();
    let module_lines = module.lines().count();
    assert!(
        module_lines <= MAX_STATS_COMPAT_MASKED_UNIT_STRIDE_VECTOR_MEMORY_LINES,
        "tests/cli_run/stats_compat/masked_unit_stride_vector_memory.rs exceeds {MAX_STATS_COMPAT_MASKED_UNIT_STRIDE_VECTOR_MEMORY_LINES} lines: {module_lines}"
    );

    let expected_function_names = [
        "rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory",
        "rem6_run_in_order_pipeline_suppresses_masked_vector_unit_stride_cross_line_load",
        "rem6_run_in_order_pipeline_suppresses_masked_vector_unit_stride_cross_line_store",
        "rem6_run_in_order_pipeline_suppresses_noncontiguous_masked_vector_unit_stride_cross_line_store",
        "rem6_run_in_order_pipeline_suppresses_noncontiguous_masked_vector_unit_stride_cross_line_load",
        "rem6_run_in_order_pipeline_suppresses_all_inactive_masked_vector_unit_stride_cross_line_load",
        "rem6_run_in_order_pipeline_suppresses_all_inactive_masked_vector_unit_stride_cross_line_store",
        "rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory_element_widths",
        "masked_unit_stride_vector_memory_program",
        "masked_unit_stride_cross_line_suppressed_load_program",
        "masked_unit_stride_cross_line_suppressed_store_program",
        "masked_unit_stride_noncontiguous_cross_line_suppressed_store_program",
        "masked_unit_stride_noncontiguous_cross_line_suppressed_load_program",
        "masked_unit_stride_all_inactive_cross_line_suppressed_load_program",
        "masked_unit_stride_all_inactive_cross_line_suppressed_store_program",
        "masked_unit_stride_vector_memory_width_program",
        "vector_lanes_to_bytes",
    ];
    let module_function_names = module
        .lines()
        .filter_map(|line| {
            let signature = line.trim_start().strip_prefix("fn ")?;
            signature.split_once('(').map(|(name, _)| name)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        module_function_names, expected_function_names,
        "tests/cli_run/stats_compat/masked_unit_stride_vector_memory.rs must own exactly the expected eight tests and nine builders/utilities"
    );
    for function_name in expected_function_names {
        let definition = format!("fn {function_name}(");
        assert!(
            !root.contains(&definition),
            "tests/cli_run/stats_compat.rs still owns `{definition}`"
        );
    }

    for family in [
        "in-order-vector-unit-stride-masked-load-store",
        "in-order-vector-masked-e32-m1-cross-line-load-suppression",
        "in-order-vector-masked-e32-m1-noncontig-cross-line-load-suppression",
        "in-order-vector-masked-e32-m1-all-inactive-cross-line-load-suppression",
        "in-order-vector-unit-stride-masked-load-store-{name}",
    ] {
        assert!(
            module.contains(family),
            "tests/cli_run/stats_compat/masked_unit_stride_vector_memory.rs is missing binary family `{family}`"
        );
        assert!(
            !root.contains(family),
            "tests/cli_run/stats_compat.rs still owns binary family `{family}`"
        );
    }
}
