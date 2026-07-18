use super::*;

const O3_ROOT: &str = "tests/cli_run/m5_host_actions/o3.rs";
const CONNECTED_PREFIX: &str = "tests/cli_run/m5_host_actions/o3/lsq_store_byte_compose.rs";
const DISJOINT_PREFIX: &str = "tests/cli_run/m5_host_actions/o3/lsq_store_prefix.rs";
const MAX_DISJOINT_PREFIX_LINES: usize = 450;
const DISJOINT_PREFIX_TESTS: [&str; 4] = [
    "rem6_run_o3_detailed_disjoint_store_prefix_composition_direct",
    "rem6_run_o3_detailed_disjoint_store_prefix_composition_cache_fabric_dram",
    "rem6_run_o3_disjoint_store_prefix_rows_are_resident_before_responses",
    "rem6_run_o3_timing_disjoint_store_prefix_uses_transport_without_o3_composition",
];

#[test]
fn disjoint_store_prefix_cli_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join(O3_ROOT)).unwrap();
    let connected = fs::read_to_string(crate_dir.join(CONNECTED_PREFIX)).unwrap();
    let disjoint_path = crate_dir.join(DISJOINT_PREFIX);
    let disjoint = fs::read_to_string(&disjoint_path).unwrap();

    assert!(
        module_has_path_attribute(&root, "lsq_store_prefix", "o3/lsq_store_prefix.rs"),
        "{O3_ROOT} must declare the focused disjoint store-prefix matrix"
    );
    assert!(
        line_count(&disjoint_path) <= MAX_DISJOINT_PREFIX_LINES,
        "{DISJOINT_PREFIX} exceeds {MAX_DISJOINT_PREFIX_LINES} lines"
    );

    let root_functions = rust_function_definition_names(&root);
    let connected_functions = rust_function_definition_names(&connected);
    let disjoint_functions = rust_function_definition_names(&disjoint);
    for function in DISJOINT_PREFIX_TESTS {
        assert!(
            disjoint_functions.contains(function),
            "{DISJOINT_PREFIX} is missing `{function}`"
        );
        assert!(
            !root_functions.contains(function),
            "{O3_ROOT} must not inline `{function}`"
        );
        assert!(
            !connected_functions.contains(function),
            "{CONNECTED_PREFIX} must not duplicate `{function}`"
        );
    }
}
