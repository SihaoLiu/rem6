use super::*;

const MAX_PERSISTENT_IQ_CLI_LINES: usize = 900;
const MAX_PERSISTENT_IQ_POLICY_LINES: usize = 600;
const PERSISTENT_IQ_CLI: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq.rs";
const O3_CLI_DIR: &str = "tests/cli_run/m5_host_actions/o3";
const RETIRED_GENERAL_IQ_OWNERS: [&str; 2] = [
    "tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs",
    "tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs",
];
const PERSISTENT_IQ_ANCHORS: [&str; 11] = [
    "rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct",
    "rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct",
    "rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy",
    "rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct",
    "rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix",
    "rem6_run_o3_persistent_iq_text_stats_expose_queue_counters",
    "rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters",
    "rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup",
    "rem6_run_host_switch_preserves_o3_persistent_iq_ticks",
    "rem6_run_o3_persistent_iq_checkpoint_boundary",
    "rem6_run_timing_suppresses_o3_persistent_iq_surface",
];
const MOVED_GENERAL_IQ_ANCHORS: [&str; 6] = [
    "rem6_run_o3_general_iq_oldest_ready_width_one_direct",
    "rem6_run_o3_general_iq_oldest_ready_width_two_direct",
    "rem6_run_o3_general_iq_control_release_orders_descendant",
    "rem6_run_host_switch_preserves_o3_general_iq_ticks",
    "rem6_run_o3_general_iq_checkpoint_boundary",
    "rem6_run_timing_suppresses_o3_general_iq_surface",
];

#[test]
fn o3_persistent_iq_focused_owners_exist_and_stay_bounded() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli = crate_dir.join(PERSISTENT_IQ_CLI);
    let policy = crate_dir.join("tests/source_policy/o3_persistent_iq_ownership.rs");

    assert!(cli.is_file(), "missing {}", cli.display());
    assert!(line_count(&cli) <= MAX_PERSISTENT_IQ_CLI_LINES);
    assert!(line_count(&policy) <= MAX_PERSISTENT_IQ_POLICY_LINES);
    for retired in RETIRED_GENERAL_IQ_OWNERS {
        assert!(
            !crate_dir.join(retired).exists(),
            "retired general-IQ owner still exists: {retired}",
        );
    }

    let source = fs::read_to_string(&cli).unwrap();
    let definitions = parsed_function_definition_names(PERSISTENT_IQ_CLI, &source);
    let global_definitions = rust_source_files(&crate_dir.join(O3_CLI_DIR))
        .into_iter()
        .flat_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().display().to_string();
            let source = fs::read_to_string(&path).unwrap();
            parsed_function_definition_names(&relative, &source)
        })
        .collect::<Vec<_>>();
    for anchor in PERSISTENT_IQ_ANCHORS {
        assert_eq!(
            function_definition_count(&definitions, anchor),
            1,
            "{PERSISTENT_IQ_CLI} must define exactly one `fn {anchor}`"
        );
        assert_eq!(
            CORE_TEST_ANCHORS
                .lines()
                .filter(|registered| *registered == anchor)
                .count(),
            1,
            "core_test_anchors.txt must register `{anchor}` exactly once"
        );
        assert_eq!(
            function_definition_count(&global_definitions, anchor),
            1,
            "{O3_CLI_DIR} must define `{anchor}` exactly once globally",
        );
    }
    for anchor in MOVED_GENERAL_IQ_ANCHORS {
        assert_eq!(
            function_definition_count(&definitions, anchor),
            0,
            "{PERSISTENT_IQ_CLI} must not retain moved general-IQ anchor `{anchor}`"
        );
        assert_eq!(
            function_definition_count(&global_definitions, anchor),
            0,
            "{O3_CLI_DIR} must not retain moved general-IQ anchor `{anchor}`",
        );
        assert_eq!(
            CORE_TEST_ANCHORS
                .lines()
                .filter(|registered| *registered == anchor)
                .count(),
            0,
            "core_test_anchors.txt must not retain moved general-IQ anchor `{anchor}`",
        );
    }
}

fn parsed_function_definition_names(relative: &str, source: &str) -> Vec<String> {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"))
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect()
}

fn function_definition_count(definitions: &[String], anchor: &str) -> usize {
    definitions
        .iter()
        .filter(|definition| definition.as_str() == anchor)
        .count()
}
