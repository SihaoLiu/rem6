use super::*;

const O3_ROOT: &str = "tests/cli_run/m5_host_actions/o3.rs";
const FU_LATENCY: &str = "tests/cli_run/m5_host_actions/o3/fu_latency.rs";
const RUNTIME_STATS: &str = "tests/cli_run/m5_host_actions/o3/runtime_stats.rs";
const FU_LATENCY_TESTS: [&str; 6] = [
    "rem6_run_records_o3_fu_latency_stats_after_detailed_switch",
    "rem6_run_records_o3_float_misc_fu_latency_stats_after_detailed_switch",
    "rem6_run_o3_runtime_json_exposes_extended_float_fu_latency_classes",
    "rem6_run_o3_runtime_json_exposes_vector_integer_fu_latency_classes",
    "rem6_run_text_stats_alias_o3_fu_latency_after_detailed_switch",
    "rem6_run_text_stats_alias_o3_float_misc_fu_latency_after_detailed_switch",
];
const RUNTIME_STATS_TESTS: [&str; 4] = [
    "rem6_run_o3_runtime_json_exposes_iq_iew_commit_matrices",
    "rem6_run_text_stats_alias_o3_runtime_stats_after_detailed_switch",
    "rem6_run_does_not_record_o3_runtime_stats_after_timing_switch",
    "rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch",
];

#[test]
fn o3_runtime_stat_tests_live_in_focused_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join(O3_ROOT)).unwrap();
    let root_functions = rust_function_definition_names(&root);

    for function in FU_LATENCY_TESTS.into_iter().chain(RUNTIME_STATS_TESTS) {
        assert!(
            !root_functions.contains(function),
            "{O3_ROOT} must not own focused runtime-stat test `{function}`"
        );
    }

    assert!(
        module_has_path_attribute(&root, "fu_latency", "o3/fu_latency.rs"),
        "{O3_ROOT} must declare {FU_LATENCY}"
    );
    assert!(
        module_has_path_attribute(&root, "runtime_stats", "o3/runtime_stats.rs",),
        "{O3_ROOT} must declare {RUNTIME_STATS}"
    );

    let fu_latency = fs::read_to_string(crate_dir.join(FU_LATENCY)).unwrap();
    let runtime_stats = fs::read_to_string(crate_dir.join(RUNTIME_STATS)).unwrap();
    let fu_latency_functions = rust_function_definition_names(&fu_latency);
    let runtime_stats_functions = rust_function_definition_names(&runtime_stats);

    for function in FU_LATENCY_TESTS {
        assert!(
            fu_latency_functions.contains(function),
            "{FU_LATENCY} is missing `{function}`"
        );
        assert!(
            !runtime_stats_functions.contains(function),
            "{RUNTIME_STATS} must not duplicate FU-latency test `{function}`"
        );
    }
    for function in RUNTIME_STATS_TESTS {
        assert!(
            runtime_stats_functions.contains(function),
            "{RUNTIME_STATS} is missing `{function}`"
        );
        assert!(
            !fu_latency_functions.contains(function),
            "{FU_LATENCY} must not duplicate runtime-stat alias test `{function}`"
        );
    }
}
