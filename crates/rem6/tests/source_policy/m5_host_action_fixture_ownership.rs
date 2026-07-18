use super::*;

#[test]
fn cli_m5_host_actions_o3_fixtures_live_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/cli_run/m5_host_actions.rs");
    let o3_root_path = crate_dir.join("tests/cli_run/m5_host_actions/o3.rs");
    let fixtures_path = crate_dir.join("tests/cli_run/m5_host_actions/o3/fixtures.rs");

    assert!(
        fixtures_path.exists(),
        "O3 host-action binary fixtures belong in tests/cli_run/m5_host_actions/o3/fixtures.rs"
    );

    let root = fs::read_to_string(root_path).unwrap();
    let o3_root = fs::read_to_string(o3_root_path).unwrap();
    let fixtures = fs::read_to_string(fixtures_path).unwrap();
    let root_functions = rust_function_definition_names(&root);
    let fixture_functions = rust_function_definition_names(&fixtures);
    let misplaced = root_functions
        .iter()
        .filter(|name| {
            name.starts_with("detailed_o3_")
                || name.starts_with("multicore_hart_detailed_o3_")
                || name.starts_with("multicore_hart1_detailed_o3_")
                || name.starts_with("sparse_three_core_detailed_o3_")
                || name.starts_with("timing_switch_o3_")
                || name.starts_with("live_retire_gate_")
                || name.as_str() == "functional_store_conditional_failure_binary"
                || name.as_str() == "scheduled_host_restore_missing_label_binary"
        })
        .collect::<Vec<_>>();

    assert!(
        misplaced.is_empty(),
        "tests/cli_run/m5_host_actions.rs must not own O3 fixtures: {misplaced:?}"
    );

    for fixture in [
        "detailed_o3_runtime_stats_binary",
        "detailed_o3_branch_repair_words",
        "timing_switch_o3_dump_stats_binary",
    ] {
        assert!(
            fixture_functions.contains(fixture),
            "focused O3 fixture module is missing `{fixture}`"
        );
    }

    assert!(
        module_has_path_attribute(&o3_root, "fixtures", "o3/fixtures.rs"),
        "tests/cli_run/m5_host_actions/o3.rs must declare the focused fixture module"
    );
    assert!(
        has_exact_trimmed_source_line(&o3_root, "use fixtures::*;"),
        "O3 host-action tests must import their focused fixture builders"
    );
}
