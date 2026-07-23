use super::*;
use syn::visit::Visit;

const CORE_TEST_ANCHORS: &str = include_str!("core_test_anchors.txt");
const WRITEBACK_ROOT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port.rs";
const FIXED_FU: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs";
const RESULT_SUPPORT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_support.rs";
const RESULT_CLASSES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs";
const RESULT_SCALAR_SUFFIX: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/scalar_suffix.rs";
const RESULT_PAIRS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/pairs.rs";
const RESULT_CLASSES_OLD_SUPPORT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs";
const RESULT_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs";
const RESULT_BOUNDARIES_SUPPORT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries/support.rs";
const STORE_CONDITIONAL_RESULT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs";
const YOUNGER_ATOMIC_RESULT: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/younger_atomic_result.rs";
const YOUNGER_ATOMIC_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/younger_atomic_result/boundaries.rs";
const DEPENDENT_RESULT_ADDRESS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs";
const DEPENDENT_RESULT_ADDRESS_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs";
const TWO_PENDING_RESULT_ADDRESS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs";
const TWO_PENDING_RESULT_ADDRESS_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs";
const THREE_PENDING_RESULT_ADDRESS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs";
const THREE_PENDING_RESULT_ADDRESS_FIXTURE: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs";
const THREE_PENDING_RESULT_ADDRESS_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs";
const WRITEBACK_ROOT_MODULES: [ExpectedModuleDeclaration; 7] = [
    ExpectedModuleDeclaration {
        name: "result_support",
        path: "writeback_port/result_support.rs",
    },
    ExpectedModuleDeclaration {
        name: "result_classes",
        path: "writeback_port/result_classes.rs",
    },
    ExpectedModuleDeclaration {
        name: "result_boundaries",
        path: "writeback_port/result_boundaries.rs",
    },
    ExpectedModuleDeclaration {
        name: "store_conditional_result",
        path: "writeback_port/store_conditional_result.rs",
    },
    ExpectedModuleDeclaration {
        name: "younger_atomic_result",
        path: "writeback_port/younger_atomic_result.rs",
    },
    ExpectedModuleDeclaration {
        name: "dependent_result_address",
        path: "writeback_port/dependent_result_address.rs",
    },
    ExpectedModuleDeclaration {
        name: "fixed_fu",
        path: "writeback_port/fixed_fu.rs",
    },
];
const YOUNGER_ATOMIC_CHILD_MODULES: [ExpectedModuleDeclaration; 1] = [ExpectedModuleDeclaration {
    name: "boundaries",
    path: "younger_atomic_result/boundaries.rs",
}];
const DEPENDENT_RESULT_ADDRESS_CHILD_MODULES: [ExpectedModuleDeclaration; 3] = [
    ExpectedModuleDeclaration {
        name: "boundaries",
        path: "dependent_result_address/boundaries.rs",
    },
    ExpectedModuleDeclaration {
        name: "two_pending",
        path: "dependent_result_address/two_pending.rs",
    },
    ExpectedModuleDeclaration {
        name: "three_pending",
        path: "dependent_result_address/three_pending.rs",
    },
];
const TWO_PENDING_RESULT_ADDRESS_CHILD_MODULES: [ExpectedModuleDeclaration; 1] =
    [ExpectedModuleDeclaration {
        name: "boundaries",
        path: "two_pending/boundaries.rs",
    }];
const THREE_PENDING_RESULT_ADDRESS_CHILD_MODULES: [ExpectedModuleDeclaration; 2] = [
    ExpectedModuleDeclaration {
        name: "boundaries",
        path: "three_pending/boundaries.rs",
    },
    ExpectedModuleDeclaration {
        name: "fixture",
        path: "three_pending/fixture.rs",
    },
];
const RESULT_BOUNDARY_SUPPORT_MODULES: [ExpectedModuleDeclaration; 1] =
    [ExpectedModuleDeclaration {
        name: "support",
        path: "result_boundaries/support.rs",
    }];
const RESULT_CLASS_CHILD_MODULES: [ExpectedModuleDeclaration; 3] = [
    ExpectedModuleDeclaration {
        name: "pairs",
        path: "result_classes/pairs.rs",
    },
    ExpectedModuleDeclaration {
        name: "scalar_suffix",
        path: "result_classes/scalar_suffix.rs",
    },
    ExpectedModuleDeclaration {
        name: "translated_mmio_pairs",
        path: "result_classes/translated_mmio_pairs.rs",
    },
];
const RESULT_CLASS_TEST_PREFIX: &str = "rem6_run_o3_memory_result_writeback_";
const FIXED_FU_ANCHORS: [&str; 11] = [
    "rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision",
    "rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision",
    "rem6_run_o3_writeback_port_json_exposes_counters",
    "rem6_run_o3_writeback_port_text_stats_expose_counters",
    "rem6_run_o3_writeback_port_stats_dump_exposes_counters",
    "rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission",
    "rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram",
    "rem6_run_timing_suppresses_o3_writeback_port_surface",
    "rem6_run_o3_writeback_wrong_path_reservation_never_publishes",
    "rem6_run_o3_writeback_port_checkpoint_boundary",
    "rem6_run_host_switch_preserves_o3_writeback_port_ticks",
];
const RESULT_CLASS_ANCHORS: [&str; 4] = [
    "rem6_run_o3_memory_result_writeback_matrix_direct",
    "rem6_run_o3_memory_result_writeback_matrix_cache_fabric_dram",
    "rem6_run_o3_memory_result_writeback_width_two_exact_fit",
    "rem6_run_o3_memory_result_writeback_readfile_mmio",
];
const RESULT_SCALAR_SUFFIX_ANCHORS: [&str; 5] = [
    "rem6_run_o3_memory_result_scalar_suffix_matrix_direct",
    "rem6_run_o3_memory_result_scalar_suffix_matrix_cache_fabric_dram",
    "rem6_run_o3_memory_result_scalar_suffix_width_two_exact_fit_direct",
    "rem6_run_o3_memory_result_scalar_suffix_readfile_mmio",
    "rem6_run_timing_suppresses_o3_memory_result_scalar_suffix",
];
const RESULT_PAIR_ANCHORS: [&str; 5] = [
    "rem6_run_o3_memory_result_pair_matrix_direct",
    "rem6_run_o3_memory_result_pair_matrix_cache_fabric_dram",
    "rem6_run_o3_memory_result_pair_width_two_exact_fit_direct",
    "rem6_run_o3_memory_result_pair_boundaries",
    "rem6_run_timing_suppresses_o3_memory_result_pairs",
];
const RESULT_BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_memory_result_writeback_rejects_resultless_and_unsupported_shapes",
    "rem6_run_o3_memory_result_writeback_all_inactive_vector_issues_no_request",
    "rem6_run_o3_memory_result_writeback_denied_amo_traps_before_transport",
    "rem6_run_o3_memory_result_writeback_live_checkpoint_rejects",
    "rem6_run_o3_memory_result_writeback_live_mode_switch_rejects",
    "rem6_run_timing_suppresses_o3_memory_result_writeback_surface",
];
const STORE_CONDITIONAL_RESULT_ANCHORS: [&str; 6] = [
    "rem6_run_o3_store_conditional_result_width_one_serializes_direct",
    "rem6_run_o3_store_conditional_result_width_two_exact_fit_direct",
    "rem6_run_o3_store_conditional_result_cache_fabric_dram",
    "rem6_run_o3_store_conditional_failure_is_local_and_deferred",
    "rem6_run_o3_store_conditional_result_live_actions_reject",
    "rem6_run_timing_suppresses_o3_store_conditional_result_surface",
];
const YOUNGER_ATOMIC_RESULT_ANCHORS: [&str; 3] = [
    "rem6_run_o3_younger_atomic_result_matrix_direct",
    "rem6_run_o3_younger_atomic_result_matrix_cache_fabric_dram",
    "rem6_run_timing_suppresses_o3_younger_atomic_result",
];
const YOUNGER_ATOMIC_BOUNDARY_ANCHORS: [&str; 1] =
    ["rem6_run_o3_younger_atomic_result_boundaries_and_live_actions"];
const DEPENDENT_RESULT_ADDRESS_ANCHORS: [&str; 3] = [
    "rem6_run_o3_dependent_result_address_matrix_direct",
    "rem6_run_o3_dependent_result_address_matrix_cache_fabric_dram",
    "rem6_run_timing_suppresses_o3_dependent_result_address",
];
const DEPENDENT_RESULT_ADDRESS_BOUNDARY_ANCHORS: [&str; 1] =
    ["rem6_run_o3_dependent_result_address_boundaries_and_live_actions"];
const TWO_PENDING_RESULT_ADDRESS_ANCHORS: [&str; 6] = [
    "rem6_run_o3_two_pending_result_address_sibling_width_one_direct",
    "rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy",
    "rem6_run_o3_two_pending_result_address_chain_width_one_direct",
    "rem6_run_o3_two_pending_result_address_chain_width_two_hierarchy",
    "rem6_run_o3_two_pending_result_address_atomic_sibling_direct",
    "rem6_run_o3_two_pending_result_address_atomic_chain_hierarchy",
];
const TWO_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS: [&str; 5] = [
    "rem6_run_o3_two_pending_result_address_replays_first_failure",
    "rem6_run_o3_two_pending_result_address_replays_second_failure",
    "rem6_run_o3_two_pending_result_address_rejects_atomic_chain_overlap",
    "rem6_run_o3_two_pending_result_address_rejects_live_checkpoint_and_handoff",
    "rem6_run_o3_two_pending_result_address_timing_mode_suppresses_o3_evidence",
];
const THREE_PENDING_RESULT_ADDRESS_ANCHORS: [&str; 6] = [
    "rem6_run_o3_three_pending_sibling_width_one_direct",
    "rem6_run_o3_three_pending_sibling_width_two_direct",
    "rem6_run_o3_three_pending_sibling_width_four_hierarchy",
    "rem6_run_o3_three_pending_chain_width_four_direct",
    "rem6_run_o3_three_pending_chain_width_two_hierarchy",
    "rem6_run_o3_three_pending_mixed_fanout_width_two_hierarchy",
];
const THREE_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_three_pending_rejects_fourth_unresolved",
    "rem6_run_o3_three_pending_rejects_nonadjacent_graph",
    "rem6_run_o3_three_pending_replays_middle_failure",
    "rem6_run_o3_three_pending_checkpoint_boundary",
    "rem6_run_host_switch_preserves_o3_three_pending_transport_ticks",
    "rem6_run_timing_suppresses_o3_three_pending_surface",
];
const RESULT_SUPPORT_HELPERS: [&str; 12] = [
    "data_trace",
    "event_str",
    "json_u64",
    "assert_event_order",
    "assert_resource_counter",
    "memory_dump_hex",
    "assert_register",
    "assert_register_absent",
    "rob_entry_at_sequence",
    "assert_rob_sequence_absent",
    "memory_result_event_at_pc",
    "result_memory_trace",
];
const RESULT_SUPPORT_FUNCTIONS: [&str; 13] = [
    "data_trace",
    "event_str",
    "json_u64",
    "assert_event_order",
    "assert_resource_counter",
    "memory_dump_hex",
    "assert_register",
    "assert_register_absent",
    "rob_entries",
    "rob_entry_at_sequence",
    "assert_rob_sequence_absent",
    "memory_result_event_at_pc",
    "result_memory_trace",
];
const RESULT_BOUNDARY_SUPPORT_HELPERS: [&str; 2] = [
    "pmp_denied_amo_output",
    "assert_denied_amo_failure_diagnostics",
];
const WRITEBACK_ROOT_MAX_LINES: usize = 550;
const FIXED_FU_MAX_LINES: usize = 800;
const RESULT_SUPPORT_MAX_LINES: usize = 160;
const RESULT_CLASSES_MAX_LINES: usize = 700;
const RESULT_CLASSES_AGGREGATE_MAX_LINES: usize = 805;
const RESULT_SCALAR_SUFFIX_MAX_LINES: usize = 500;
const RESULT_PAIRS_MAX_LINES: usize = 650;
const RESULT_CLASS_FAMILY_AGGREGATE_MAX_LINES: usize = 1950;
const RESULT_BOUNDARIES_MAX_LINES: usize = 700;
const RESULT_BOUNDARIES_SUPPORT_MAX_LINES: usize = 140;
const RESULT_BOUNDARIES_AGGREGATE_MAX_LINES: usize = 800;
const STORE_CONDITIONAL_RESULT_MAX_LINES: usize = 650;
const YOUNGER_ATOMIC_RESULT_MAX_LINES: usize = 450;
const YOUNGER_ATOMIC_BOUNDARIES_MAX_LINES: usize = 350;
const YOUNGER_ATOMIC_AGGREGATE_MAX_LINES: usize = 750;
const DEPENDENT_RESULT_ADDRESS_MAX_LINES: usize = 650;
const DEPENDENT_RESULT_ADDRESS_BOUNDARIES_MAX_LINES: usize = 450;
const DEPENDENT_RESULT_ADDRESS_AGGREGATE_MAX_LINES: usize = 1000;
const TWO_PENDING_RESULT_ADDRESS_MAX_LINES: usize = 700;
const TWO_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES: usize = 500;
const TWO_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES: usize = 1050;
const THREE_PENDING_RESULT_ADDRESS_MAX_LINES: usize = 550;
const THREE_PENDING_RESULT_ADDRESS_FIXTURE_MAX_LINES: usize = 450;
const THREE_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES: usize = 550;
const THREE_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES: usize = 1275;

#[derive(Clone, Copy)]
struct ExpectedModuleDeclaration {
    name: &'static str,
    path: &'static str,
}

#[derive(Debug)]
struct ModuleDeclaration {
    name: String,
    path_attributes: Vec<Option<String>>,
    inline: bool,
}

#[test]
fn writeback_result_class_cli_evidence_has_focused_ownership() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(WRITEBACK_ROOT);
    let fixed_fu_path = crate_dir.join(FIXED_FU);
    let support_path = crate_dir.join(RESULT_SUPPORT);
    let child_path = crate_dir.join(RESULT_CLASSES);
    let scalar_suffix_path = crate_dir.join(RESULT_SCALAR_SUFFIX);
    let pairs_path = crate_dir.join(RESULT_PAIRS);
    let old_support_path = crate_dir.join(RESULT_CLASSES_OLD_SUPPORT);
    let boundary_path = crate_dir.join(RESULT_BOUNDARIES);
    let boundary_support_path = crate_dir.join(RESULT_BOUNDARIES_SUPPORT);
    let store_conditional_path = crate_dir.join(STORE_CONDITIONAL_RESULT);
    let younger_atomic_path = crate_dir.join(YOUNGER_ATOMIC_RESULT);
    let younger_atomic_boundaries_path = crate_dir.join(YOUNGER_ATOMIC_BOUNDARIES);
    let dependent_result_address_path = crate_dir.join(DEPENDENT_RESULT_ADDRESS);
    let dependent_result_address_boundaries_path =
        crate_dir.join(DEPENDENT_RESULT_ADDRESS_BOUNDARIES);
    let two_pending_result_address_path = crate_dir.join(TWO_PENDING_RESULT_ADDRESS);
    let two_pending_result_address_boundaries_path =
        crate_dir.join(TWO_PENDING_RESULT_ADDRESS_BOUNDARIES);
    let three_pending_result_address_path = crate_dir.join(THREE_PENDING_RESULT_ADDRESS);
    let three_pending_result_address_fixture_path =
        crate_dir.join(THREE_PENDING_RESULT_ADDRESS_FIXTURE);
    let three_pending_result_address_boundaries_path =
        crate_dir.join(THREE_PENDING_RESULT_ADDRESS_BOUNDARIES);
    let root = fs::read_to_string(&root_path).unwrap();
    let fixed_fu = fs::read_to_string(&fixed_fu_path);
    let child = fs::read_to_string(&child_path).unwrap();
    let scalar_suffix = fs::read_to_string(&scalar_suffix_path);
    let pairs = fs::read_to_string(&pairs_path);
    let support = fs::read_to_string(&support_path);
    let boundary = fs::read_to_string(&boundary_path);
    let boundary_support = fs::read_to_string(&boundary_support_path);
    let store_conditional = fs::read_to_string(&store_conditional_path);
    let younger_atomic = fs::read_to_string(&younger_atomic_path);
    let younger_atomic_boundaries = fs::read_to_string(&younger_atomic_boundaries_path);
    let dependent_result_address = fs::read_to_string(&dependent_result_address_path);
    let dependent_result_address_boundaries =
        fs::read_to_string(&dependent_result_address_boundaries_path);
    let two_pending_result_address = fs::read_to_string(&two_pending_result_address_path);
    let two_pending_result_address_boundaries =
        fs::read_to_string(&two_pending_result_address_boundaries_path);
    let three_pending_result_address = fs::read_to_string(&three_pending_result_address_path);
    let three_pending_result_address_fixture =
        fs::read_to_string(&three_pending_result_address_fixture_path);
    let three_pending_result_address_boundaries =
        fs::read_to_string(&three_pending_result_address_boundaries_path);

    let root_functions = top_level_function_names(WRITEBACK_ROOT, &root);
    let mut boundary_failures = Vec::new();
    if line_count(&root_path) >= WRITEBACK_ROOT_MAX_LINES {
        boundary_failures.push(format!(
            "{WRITEBACK_ROOT} must remain below {WRITEBACK_ROOT_MAX_LINES} lines"
        ));
    }
    for helper in RESULT_SUPPORT_HELPERS {
        if root_functions.iter().any(|name| name == helper) {
            boundary_failures.push(format!(
                "{WRITEBACK_ROOT} must not own result helper `{helper}`"
            ));
        }
    }
    boundary_failures.extend(module_declaration_failures(
        WRITEBACK_ROOT,
        &root,
        &WRITEBACK_ROOT_MODULES,
    ));
    match &fixed_fu {
        Ok(fixed_fu) => boundary_failures.extend(support_leaf_failures(FIXED_FU, fixed_fu)),
        Err(_) => boundary_failures.push(format!("{FIXED_FU} must exist")),
    }
    for (relative, source) in [
        (WRITEBACK_ROOT, root.as_str()),
        (RESULT_CLASSES, child.as_str()),
    ] {
        let includes = top_level_include_paths(relative, source);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{relative} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if old_support_path.exists() {
        boundary_failures.push(format!("{RESULT_CLASSES_OLD_SUPPORT} must be removed"));
    }
    boundary_failures.extend(module_declaration_failures(
        RESULT_CLASSES,
        &child,
        &RESULT_CLASS_CHILD_MODULES,
    ));
    if scalar_suffix.is_err() {
        boundary_failures.push(format!("{RESULT_SCALAR_SUFFIX} must exist"));
    }
    if let Ok(scalar_suffix) = &scalar_suffix {
        let includes = top_level_include_paths(RESULT_SCALAR_SUFFIX, scalar_suffix);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{RESULT_SCALAR_SUFFIX} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if pairs.is_err() {
        boundary_failures.push(format!("{RESULT_PAIRS} must exist"));
    }
    if let Ok(pairs) = &pairs {
        let includes = top_level_include_paths(RESULT_PAIRS, pairs);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{RESULT_PAIRS} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if support.is_err() {
        boundary_failures.push(format!("{RESULT_SUPPORT} must exist"));
    }
    if boundary.is_err() {
        boundary_failures.push(format!("{RESULT_BOUNDARIES} must exist"));
    }
    if store_conditional.is_err() {
        boundary_failures.push(format!("{STORE_CONDITIONAL_RESULT} must exist"));
    }
    if younger_atomic.is_err() {
        boundary_failures.push(format!("{YOUNGER_ATOMIC_RESULT} must exist"));
    }
    if younger_atomic_boundaries.is_err() {
        boundary_failures.push(format!("{YOUNGER_ATOMIC_BOUNDARIES} must exist"));
    }
    if dependent_result_address.is_err() {
        boundary_failures.push(format!("{DEPENDENT_RESULT_ADDRESS} must exist"));
    }
    if dependent_result_address_boundaries.is_err() {
        boundary_failures.push(format!("{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must exist"));
    }
    if two_pending_result_address.is_err() {
        boundary_failures.push(format!("{TWO_PENDING_RESULT_ADDRESS} must exist"));
    }
    if two_pending_result_address_boundaries.is_err() {
        boundary_failures.push(format!(
            "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must exist"
        ));
    }
    if three_pending_result_address.is_err() {
        boundary_failures.push(format!("{THREE_PENDING_RESULT_ADDRESS} must exist"));
    }
    if three_pending_result_address_fixture.is_err() {
        boundary_failures.push(format!("{THREE_PENDING_RESULT_ADDRESS_FIXTURE} must exist"));
    }
    if three_pending_result_address_boundaries.is_err() {
        boundary_failures.push(format!(
            "{THREE_PENDING_RESULT_ADDRESS_BOUNDARIES} must exist"
        ));
    }
    match &boundary {
        Ok(boundary) => {
            boundary_failures.extend(boundary_support_module_failures(
                RESULT_BOUNDARIES,
                boundary,
            ));
            let includes = top_level_include_paths(RESULT_BOUNDARIES, boundary);
            if !includes.is_empty() {
                boundary_failures.push(format!(
                    "{RESULT_BOUNDARIES} must not contain top-level include! fragments: {includes:?}"
                ));
            }
        }
        Err(_) => boundary_failures.push(format!("{RESULT_BOUNDARIES} must exist")),
    }
    match &boundary_support {
        Ok(boundary_support) => {
            let includes = top_level_include_paths(RESULT_BOUNDARIES_SUPPORT, boundary_support);
            if !includes.is_empty() {
                boundary_failures.push(format!(
                    "{RESULT_BOUNDARIES_SUPPORT} must not contain top-level include! fragments: {includes:?}"
                ));
            }
        }
        Err(_) => boundary_failures.push(format!("{RESULT_BOUNDARIES_SUPPORT} must exist")),
    }
    if let Ok(store_conditional) = &store_conditional {
        let includes = top_level_include_paths(STORE_CONDITIONAL_RESULT, store_conditional);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{STORE_CONDITIONAL_RESULT} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(younger_atomic) = &younger_atomic {
        boundary_failures.extend(module_declaration_failures(
            YOUNGER_ATOMIC_RESULT,
            younger_atomic,
            &YOUNGER_ATOMIC_CHILD_MODULES,
        ));
        let includes = top_level_include_paths(YOUNGER_ATOMIC_RESULT, younger_atomic);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{YOUNGER_ATOMIC_RESULT} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(younger_atomic_boundaries) = &younger_atomic_boundaries {
        let includes =
            top_level_include_paths(YOUNGER_ATOMIC_BOUNDARIES, younger_atomic_boundaries);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{YOUNGER_ATOMIC_BOUNDARIES} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(dependent_result_address) = &dependent_result_address {
        boundary_failures.extend(module_declaration_failures(
            DEPENDENT_RESULT_ADDRESS,
            dependent_result_address,
            &DEPENDENT_RESULT_ADDRESS_CHILD_MODULES,
        ));
        let includes = top_level_include_paths(DEPENDENT_RESULT_ADDRESS, dependent_result_address);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{DEPENDENT_RESULT_ADDRESS} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(dependent_result_address_boundaries) = &dependent_result_address_boundaries {
        let includes = top_level_include_paths(
            DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
            dependent_result_address_boundaries,
        );
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(two_pending_result_address) = &two_pending_result_address {
        boundary_failures.extend(module_declaration_failures(
            TWO_PENDING_RESULT_ADDRESS,
            two_pending_result_address,
            &TWO_PENDING_RESULT_ADDRESS_CHILD_MODULES,
        ));
        let includes =
            top_level_include_paths(TWO_PENDING_RESULT_ADDRESS, two_pending_result_address);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{TWO_PENDING_RESULT_ADDRESS} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(two_pending_result_address_boundaries) = &two_pending_result_address_boundaries {
        let includes = top_level_include_paths(
            TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
            two_pending_result_address_boundaries,
        );
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    if let Ok(three_pending_result_address) = &three_pending_result_address {
        boundary_failures.extend(module_declaration_failures(
            THREE_PENDING_RESULT_ADDRESS,
            three_pending_result_address,
            &THREE_PENDING_RESULT_ADDRESS_CHILD_MODULES,
        ));
        let includes =
            top_level_include_paths(THREE_PENDING_RESULT_ADDRESS, three_pending_result_address);
        if !includes.is_empty() {
            boundary_failures.push(format!(
                "{THREE_PENDING_RESULT_ADDRESS} must not contain top-level include! fragments: {includes:?}"
            ));
        }
    }
    for (relative, source) in [
        (
            THREE_PENDING_RESULT_ADDRESS_FIXTURE,
            &three_pending_result_address_fixture,
        ),
        (
            THREE_PENDING_RESULT_ADDRESS_BOUNDARIES,
            &three_pending_result_address_boundaries,
        ),
    ] {
        if let Ok(source) = source {
            let includes = top_level_include_paths(relative, source);
            if !includes.is_empty() {
                boundary_failures.push(format!(
                    "{relative} must not contain top-level include! fragments: {includes:?}"
                ));
            }
        }
    }
    assert!(
        boundary_failures.is_empty(),
        "writeback result ownership boundary is incomplete:\n{}",
        boundary_failures.join("\n")
    );
    let fixed_fu = fixed_fu.unwrap();
    let support = support.unwrap();
    let scalar_suffix = scalar_suffix.unwrap();
    let pairs = pairs.unwrap();
    let boundary = boundary.unwrap();
    let boundary_support = boundary_support.unwrap();
    let store_conditional = store_conditional.unwrap();
    let younger_atomic = younger_atomic.unwrap();
    let younger_atomic_boundaries = younger_atomic_boundaries.unwrap();
    let dependent_result_address = dependent_result_address.unwrap();
    let dependent_result_address_boundaries = dependent_result_address_boundaries.unwrap();
    let two_pending_result_address = two_pending_result_address.unwrap();
    let two_pending_result_address_boundaries = two_pending_result_address_boundaries.unwrap();
    let three_pending_result_address = three_pending_result_address.unwrap();
    let three_pending_result_address_fixture = three_pending_result_address_fixture.unwrap();
    let three_pending_result_address_boundaries = three_pending_result_address_boundaries.unwrap();

    assert!(
        line_count(&fixed_fu_path) <= FIXED_FU_MAX_LINES,
        "{FIXED_FU} must remain at or below {FIXED_FU_MAX_LINES} lines"
    );
    assert!(
        line_count(&support_path) <= RESULT_SUPPORT_MAX_LINES,
        "{RESULT_SUPPORT} must remain at or below {RESULT_SUPPORT_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path) <= RESULT_CLASSES_MAX_LINES,
        "{RESULT_CLASSES} must remain at or below {RESULT_CLASSES_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path) + line_count(&support_path) <= RESULT_CLASSES_AGGREGATE_MAX_LINES,
        "result-class implementation must remain at or below {RESULT_CLASSES_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&scalar_suffix_path) <= RESULT_SCALAR_SUFFIX_MAX_LINES,
        "{RESULT_SCALAR_SUFFIX} must remain at or below {RESULT_SCALAR_SUFFIX_MAX_LINES} lines"
    );
    assert!(
        line_count(&pairs_path) <= RESULT_PAIRS_MAX_LINES,
        "{RESULT_PAIRS} must remain at or below {RESULT_PAIRS_MAX_LINES} lines"
    );
    assert!(
        line_count(&child_path)
            + line_count(&support_path)
            + line_count(&scalar_suffix_path)
            + line_count(&pairs_path)
            <= RESULT_CLASS_FAMILY_AGGREGATE_MAX_LINES,
        "result-class family must remain at or below {RESULT_CLASS_FAMILY_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&boundary_path) <= RESULT_BOUNDARIES_MAX_LINES,
        "{RESULT_BOUNDARIES} must remain at or below {RESULT_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&boundary_support_path) <= RESULT_BOUNDARIES_SUPPORT_MAX_LINES,
        "{RESULT_BOUNDARIES_SUPPORT} must remain at or below {RESULT_BOUNDARIES_SUPPORT_MAX_LINES} lines"
    );
    assert!(
        line_count(&boundary_path) + line_count(&boundary_support_path)
            <= RESULT_BOUNDARIES_AGGREGATE_MAX_LINES,
        "result-boundary implementation must remain at or below {RESULT_BOUNDARIES_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&store_conditional_path) <= STORE_CONDITIONAL_RESULT_MAX_LINES,
        "{STORE_CONDITIONAL_RESULT} must remain at or below {STORE_CONDITIONAL_RESULT_MAX_LINES} lines"
    );
    assert!(
        line_count(&younger_atomic_path) <= YOUNGER_ATOMIC_RESULT_MAX_LINES,
        "{YOUNGER_ATOMIC_RESULT} must remain at or below {YOUNGER_ATOMIC_RESULT_MAX_LINES} lines"
    );
    assert!(
        line_count(&younger_atomic_boundaries_path) <= YOUNGER_ATOMIC_BOUNDARIES_MAX_LINES,
        "{YOUNGER_ATOMIC_BOUNDARIES} must remain at or below {YOUNGER_ATOMIC_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&younger_atomic_path) + line_count(&younger_atomic_boundaries_path)
            <= YOUNGER_ATOMIC_AGGREGATE_MAX_LINES,
        "younger-atomic result evidence must remain at or below {YOUNGER_ATOMIC_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&dependent_result_address_path) <= DEPENDENT_RESULT_ADDRESS_MAX_LINES,
        "{DEPENDENT_RESULT_ADDRESS} must remain at or below {DEPENDENT_RESULT_ADDRESS_MAX_LINES} lines"
    );
    assert!(
        line_count(&dependent_result_address_boundaries_path)
            <= DEPENDENT_RESULT_ADDRESS_BOUNDARIES_MAX_LINES,
        "{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must remain at or below {DEPENDENT_RESULT_ADDRESS_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&dependent_result_address_path)
            + line_count(&dependent_result_address_boundaries_path)
            <= DEPENDENT_RESULT_ADDRESS_AGGREGATE_MAX_LINES,
        "dependent result-address evidence must remain at or below {DEPENDENT_RESULT_ADDRESS_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&two_pending_result_address_path) <= TWO_PENDING_RESULT_ADDRESS_MAX_LINES,
        "{TWO_PENDING_RESULT_ADDRESS} must remain at or below {TWO_PENDING_RESULT_ADDRESS_MAX_LINES} lines"
    );
    assert!(
        line_count(&two_pending_result_address_boundaries_path)
            <= TWO_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES,
        "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must remain at or below {TWO_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&two_pending_result_address_path)
            + line_count(&two_pending_result_address_boundaries_path)
            < TWO_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES,
        "two-pending result-address evidence must remain below {TWO_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES} aggregate lines"
    );
    assert!(
        line_count(&three_pending_result_address_path)
            <= THREE_PENDING_RESULT_ADDRESS_MAX_LINES,
        "{THREE_PENDING_RESULT_ADDRESS} must remain at or below {THREE_PENDING_RESULT_ADDRESS_MAX_LINES} lines"
    );
    assert!(
        line_count(&three_pending_result_address_fixture_path)
            <= THREE_PENDING_RESULT_ADDRESS_FIXTURE_MAX_LINES,
        "{THREE_PENDING_RESULT_ADDRESS_FIXTURE} must remain at or below {THREE_PENDING_RESULT_ADDRESS_FIXTURE_MAX_LINES} lines"
    );
    assert!(
        line_count(&three_pending_result_address_boundaries_path)
            <= THREE_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES,
        "{THREE_PENDING_RESULT_ADDRESS_BOUNDARIES} must remain at or below {THREE_PENDING_RESULT_ADDRESS_BOUNDARIES_MAX_LINES} lines"
    );
    assert!(
        line_count(&three_pending_result_address_path)
            + line_count(&three_pending_result_address_fixture_path)
            + line_count(&three_pending_result_address_boundaries_path)
            <= THREE_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES,
        "three-pending result-address evidence must remain at or below {THREE_PENDING_RESULT_ADDRESS_AGGREGATE_MAX_LINES} aggregate lines"
    );
    let support_leaf_failures = support_leaf_failures(RESULT_SUPPORT, &support);
    assert!(
        support_leaf_failures.is_empty(),
        "{RESULT_SUPPORT} must remain a leaf support module:\n{}",
        support_leaf_failures.join("\n")
    );
    let support_function_failures =
        result_support_function_inventory_failures(RESULT_SUPPORT, &support);
    assert!(
        support_function_failures.is_empty(),
        "{RESULT_SUPPORT} must own exactly the result support helper inventory:\n{}",
        support_function_failures.join("\n")
    );
    assert!(
        top_level_module_names(RESULT_BOUNDARIES_SUPPORT, &boundary_support).is_empty(),
        "{RESULT_BOUNDARIES_SUPPORT} must remain a leaf support module"
    );
    assert!(
        top_level_module_names(STORE_CONDITIONAL_RESULT, &store_conditional).is_empty(),
        "{STORE_CONDITIONAL_RESULT} must remain a leaf module"
    );
    assert!(
        top_level_module_names(RESULT_SCALAR_SUFFIX, &scalar_suffix).is_empty(),
        "{RESULT_SCALAR_SUFFIX} must remain a leaf module"
    );
    assert!(
        top_level_module_names(RESULT_PAIRS, &pairs).is_empty(),
        "{RESULT_PAIRS} must remain a leaf module"
    );
    assert!(
        top_level_module_names(YOUNGER_ATOMIC_BOUNDARIES, &younger_atomic_boundaries).is_empty(),
        "{YOUNGER_ATOMIC_BOUNDARIES} must remain a leaf module"
    );
    assert!(
        top_level_module_names(
            DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
            &dependent_result_address_boundaries,
        )
        .is_empty(),
        "{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must remain a leaf module"
    );
    assert!(
        top_level_module_names(
            TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
            &two_pending_result_address_boundaries,
        )
        .is_empty(),
        "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must remain a leaf module"
    );
    assert!(
        top_level_module_names(
            THREE_PENDING_RESULT_ADDRESS_FIXTURE,
            &three_pending_result_address_fixture,
        )
        .is_empty(),
        "{THREE_PENDING_RESULT_ADDRESS_FIXTURE} must remain a leaf module"
    );
    assert!(
        top_level_module_names(
            THREE_PENDING_RESULT_ADDRESS_BOUNDARIES,
            &three_pending_result_address_boundaries,
        )
        .is_empty(),
        "{THREE_PENDING_RESULT_ADDRESS_BOUNDARIES} must remain a leaf module"
    );

    let child_functions = top_level_function_names(RESULT_CLASSES, &child);
    let support_functions = top_level_function_names(RESULT_SUPPORT, &support);
    for helper in RESULT_SUPPORT_HELPERS {
        assert_eq!(
            root_functions.iter().filter(|name| *name == helper).count(),
            0,
            "{WRITEBACK_ROOT} must not own `{helper}`"
        );
        assert_eq!(
            child_functions
                .iter()
                .filter(|name| *name == helper)
                .count(),
            0,
            "{RESULT_CLASSES} must not own `{helper}`"
        );
        assert_eq!(
            support_functions
                .iter()
                .filter(|name| *name == helper)
                .count(),
            1,
            "{RESULT_SUPPORT} must own exactly one `{helper}`"
        );
    }
    assert_eq!(
        top_level_function_names(RESULT_BOUNDARIES_SUPPORT, &boundary_support),
        RESULT_BOUNDARY_SUPPORT_HELPERS,
        "{RESULT_BOUNDARIES_SUPPORT} must own exactly the focused PMP subprocess helper"
    );

    let fixed_fu_tests = top_level_test_names(FIXED_FU, &fixed_fu);
    assert_eq!(
        fixed_fu_tests, FIXED_FU_ANCHORS,
        "{FIXED_FU} must own exactly the required fixed-FU test anchors in order"
    );
    for anchor in FIXED_FU_ANCHORS {
        assert_eq!(
            fixed_fu.matches(anchor).count(),
            1,
            "{FIXED_FU} must contain fixed-FU anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain fixed-FU anchor `{anchor}`"
            );
        }
    }

    let child_tests = result_class_tests(RESULT_CLASSES, &child);
    assert_eq!(
        child_tests, RESULT_CLASS_ANCHORS,
        "{RESULT_CLASSES} must own exactly the required result-class test anchors in order"
    );
    for (relative, source) in [(WRITEBACK_ROOT, root.as_str()), (RESULT_SUPPORT, &support)] {
        assert!(
            result_class_tests(relative, source).is_empty(),
            "{relative} must not own result-class-prefixed tests"
        );
    }

    let scalar_suffix_tests = top_level_test_names(RESULT_SCALAR_SUFFIX, &scalar_suffix);
    assert_eq!(
        scalar_suffix_tests, RESULT_SCALAR_SUFFIX_ANCHORS,
        "{RESULT_SCALAR_SUFFIX} must own exactly the required scalar-suffix anchors in order"
    );
    for anchor in RESULT_SCALAR_SUFFIX_ANCHORS {
        assert_eq!(
            scalar_suffix.matches(anchor).count(),
            1,
            "{RESULT_SCALAR_SUFFIX} must contain scalar-suffix anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain scalar-suffix anchor `{anchor}`"
            );
        }
    }

    let pair_tests = top_level_test_names(RESULT_PAIRS, &pairs);
    assert_eq!(
        pair_tests, RESULT_PAIR_ANCHORS,
        "{RESULT_PAIRS} must own exactly the required pair anchors in order"
    );
    for anchor in RESULT_PAIR_ANCHORS {
        assert_eq!(
            pairs.matches(anchor).count(),
            1,
            "{RESULT_PAIRS} must contain pair anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain pair anchor `{anchor}`"
            );
        }
    }

    let boundary_tests = top_level_test_names(RESULT_BOUNDARIES, &boundary);
    assert_eq!(
        boundary_tests, RESULT_BOUNDARY_ANCHORS,
        "{RESULT_BOUNDARIES} must own exactly the required boundary test anchors in order"
    );
    for anchor in RESULT_BOUNDARY_ANCHORS {
        assert_eq!(
            boundary.matches(anchor).count(),
            1,
            "{RESULT_BOUNDARIES} must contain boundary anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain boundary anchor `{anchor}`"
            );
        }
    }

    let store_conditional_tests =
        top_level_test_names(STORE_CONDITIONAL_RESULT, &store_conditional);
    assert_eq!(
        store_conditional_tests, STORE_CONDITIONAL_RESULT_ANCHORS,
        "{STORE_CONDITIONAL_RESULT} must own exactly the required SC result test anchors in order"
    );
    for anchor in STORE_CONDITIONAL_RESULT_ANCHORS {
        assert_eq!(
            store_conditional.matches(anchor).count(),
            1,
            "{STORE_CONDITIONAL_RESULT} must contain SC result anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain SC result anchor `{anchor}`"
            );
        }
    }

    let younger_atomic_tests = top_level_test_names(YOUNGER_ATOMIC_RESULT, &younger_atomic);
    assert_eq!(
        younger_atomic_tests, YOUNGER_ATOMIC_RESULT_ANCHORS,
        "{YOUNGER_ATOMIC_RESULT} must own exactly the required younger-atomic anchors in order"
    );
    for anchor in YOUNGER_ATOMIC_RESULT_ANCHORS {
        assert_eq!(
            younger_atomic.matches(anchor).count(),
            1,
            "{YOUNGER_ATOMIC_RESULT} must contain younger-atomic anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain younger-atomic anchor `{anchor}`"
            );
        }
    }

    let younger_atomic_boundary_tests =
        top_level_test_names(YOUNGER_ATOMIC_BOUNDARIES, &younger_atomic_boundaries);
    assert_eq!(
        younger_atomic_boundary_tests, YOUNGER_ATOMIC_BOUNDARY_ANCHORS,
        "{YOUNGER_ATOMIC_BOUNDARIES} must own exactly the required boundary anchor"
    );
    for anchor in YOUNGER_ATOMIC_BOUNDARY_ANCHORS {
        assert_eq!(
            younger_atomic_boundaries.matches(anchor).count(),
            1,
            "{YOUNGER_ATOMIC_BOUNDARIES} must contain boundary anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain younger-atomic boundary anchor `{anchor}`"
            );
        }
    }

    let dependent_result_address_tests =
        top_level_test_names(DEPENDENT_RESULT_ADDRESS, &dependent_result_address);
    assert_eq!(
        dependent_result_address_tests, DEPENDENT_RESULT_ADDRESS_ANCHORS,
        "{DEPENDENT_RESULT_ADDRESS} must own exactly the required dependent-address anchors in order"
    );
    for anchor in DEPENDENT_RESULT_ADDRESS_ANCHORS {
        assert_eq!(
            dependent_result_address.matches(anchor).count(),
            1,
            "{DEPENDENT_RESULT_ADDRESS} must contain dependent-address anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain dependent-address anchor `{anchor}`"
            );
        }
    }

    let dependent_result_address_boundary_tests = top_level_test_names(
        DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
        &dependent_result_address_boundaries,
    );
    assert_eq!(
        dependent_result_address_boundary_tests, DEPENDENT_RESULT_ADDRESS_BOUNDARY_ANCHORS,
        "{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must own exactly the required boundary anchor"
    );
    for anchor in DEPENDENT_RESULT_ADDRESS_BOUNDARY_ANCHORS {
        assert_eq!(
            dependent_result_address_boundaries.matches(anchor).count(),
            1,
            "{DEPENDENT_RESULT_ADDRESS_BOUNDARIES} must contain boundary anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain dependent-address boundary anchor `{anchor}`"
            );
        }
    }

    let two_pending_result_address_tests =
        top_level_test_names(TWO_PENDING_RESULT_ADDRESS, &two_pending_result_address);
    assert_eq!(
        two_pending_result_address_tests, TWO_PENDING_RESULT_ADDRESS_ANCHORS,
        "{TWO_PENDING_RESULT_ADDRESS} must own exactly the six positive anchors in order"
    );
    for anchor in TWO_PENDING_RESULT_ADDRESS_ANCHORS {
        assert_eq!(
            two_pending_result_address.matches(anchor).count(),
            1,
            "{TWO_PENDING_RESULT_ADDRESS} must contain positive anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
                two_pending_result_address_boundaries.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain two-pending positive anchor `{anchor}`"
            );
        }
    }

    let two_pending_result_address_boundary_tests = top_level_test_names(
        TWO_PENDING_RESULT_ADDRESS_BOUNDARIES,
        &two_pending_result_address_boundaries,
    );
    assert_eq!(
        two_pending_result_address_boundary_tests,
        TWO_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS,
        "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must own exactly the five boundary anchors in order"
    );
    for anchor in TWO_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS {
        assert_eq!(
            two_pending_result_address_boundaries.matches(anchor).count(),
            1,
            "{TWO_PENDING_RESULT_ADDRESS_BOUNDARIES} must contain boundary anchor `{anchor}` exactly once"
        );
        for (relative, source) in [
            (WRITEBACK_ROOT, root.as_str()),
            (FIXED_FU, fixed_fu.as_str()),
            (RESULT_CLASSES, child.as_str()),
            (RESULT_SCALAR_SUFFIX, scalar_suffix.as_str()),
            (RESULT_PAIRS, pairs.as_str()),
            (RESULT_SUPPORT, support.as_str()),
            (RESULT_BOUNDARIES, boundary.as_str()),
            (RESULT_BOUNDARIES_SUPPORT, boundary_support.as_str()),
            (STORE_CONDITIONAL_RESULT, store_conditional.as_str()),
            (YOUNGER_ATOMIC_RESULT, younger_atomic.as_str()),
            (
                YOUNGER_ATOMIC_BOUNDARIES,
                younger_atomic_boundaries.as_str(),
            ),
            (DEPENDENT_RESULT_ADDRESS, dependent_result_address.as_str()),
            (
                DEPENDENT_RESULT_ADDRESS_BOUNDARIES,
                dependent_result_address_boundaries.as_str(),
            ),
            (
                TWO_PENDING_RESULT_ADDRESS,
                two_pending_result_address.as_str(),
            ),
        ] {
            assert_eq!(
                source.matches(anchor).count(),
                0,
                "{relative} must not contain two-pending boundary anchor `{anchor}`"
            );
        }
    }

    let three_pending_result_address_tests =
        top_level_test_names(THREE_PENDING_RESULT_ADDRESS, &three_pending_result_address);
    assert_eq!(
        three_pending_result_address_tests, THREE_PENDING_RESULT_ADDRESS_ANCHORS,
        "{THREE_PENDING_RESULT_ADDRESS} must own exactly the six positive anchors in order"
    );
    let three_pending_result_address_boundary_tests = top_level_test_names(
        THREE_PENDING_RESULT_ADDRESS_BOUNDARIES,
        &three_pending_result_address_boundaries,
    );
    assert_eq!(
        three_pending_result_address_boundary_tests,
        THREE_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS,
        "{THREE_PENDING_RESULT_ADDRESS_BOUNDARIES} must own exactly the six boundary anchors in order"
    );
    let owned_sources = [
        (
            THREE_PENDING_RESULT_ADDRESS,
            three_pending_result_address.as_str(),
        ),
        (
            THREE_PENDING_RESULT_ADDRESS_FIXTURE,
            three_pending_result_address_fixture.as_str(),
        ),
        (
            THREE_PENDING_RESULT_ADDRESS_BOUNDARIES,
            three_pending_result_address_boundaries.as_str(),
        ),
    ];
    for (anchor, owner) in THREE_PENDING_RESULT_ADDRESS_ANCHORS
        .into_iter()
        .map(|anchor| (anchor, THREE_PENDING_RESULT_ADDRESS))
        .chain(
            THREE_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS
                .into_iter()
                .map(|anchor| (anchor, THREE_PENDING_RESULT_ADDRESS_BOUNDARIES)),
        )
    {
        let owners = owned_sources
            .iter()
            .filter_map(|(relative, source)| source.contains(anchor).then_some(*relative))
            .collect::<Vec<_>>();
        assert_eq!(owners, vec![owner], "three-pending anchor `{anchor}` owner");
    }

    let registered_core_anchors = CORE_TEST_ANCHORS
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let two_pending_anchors = TWO_PENDING_RESULT_ADDRESS_ANCHORS
        .into_iter()
        .chain(TWO_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS)
        .collect::<Vec<_>>();
    let three_pending_anchors = THREE_PENDING_RESULT_ADDRESS_ANCHORS
        .into_iter()
        .chain(THREE_PENDING_RESULT_ADDRESS_BOUNDARY_ANCHORS)
        .collect::<Vec<_>>();
    for anchor in two_pending_anchors.iter().chain(&three_pending_anchors) {
        assert_eq!(
            registered_core_anchors
                .iter()
                .filter(|registered| *registered == anchor)
                .count(),
            1,
            "core_test_anchors.txt must register writeback anchor `{anchor}` exactly once"
        );
    }
    let dependent_result_address_tail = "rem6_run_timing_suppresses_o3_dependent_result_address";
    let dependent_result_address_tail_index = registered_core_anchors
        .iter()
        .position(|anchor| *anchor == dependent_result_address_tail)
        .expect("core_test_anchors.txt must retain the dependent-result-address tail anchor");
    let two_pending_anchor_start = dependent_result_address_tail_index + 1;
    assert_eq!(
        registered_core_anchors
            .get(two_pending_anchor_start..two_pending_anchor_start + two_pending_anchors.len()),
        Some(two_pending_anchors.as_slice()),
        "two-pending anchors must immediately follow the dependent-result-address anchors"
    );
    let three_pending_anchor_start = two_pending_anchor_start + two_pending_anchors.len();
    assert_eq!(
        registered_core_anchors.get(
            three_pending_anchor_start..three_pending_anchor_start + three_pending_anchors.len()
        ),
        Some(three_pending_anchors.as_slice()),
        "three-pending anchors must immediately follow the two-pending anchors"
    );

    assert_rustfmt_clean(&fixed_fu_path);
    assert_rustfmt_clean(&child_path);
    assert_rustfmt_clean(&scalar_suffix_path);
    assert_rustfmt_clean(&pairs_path);
    assert_rustfmt_clean(&support_path);
    assert_rustfmt_clean(&boundary_path);
    assert_rustfmt_clean(&boundary_support_path);
    assert_rustfmt_clean(&store_conditional_path);
    assert_rustfmt_clean(&younger_atomic_path);
    assert_rustfmt_clean(&younger_atomic_boundaries_path);
    assert_rustfmt_clean(&dependent_result_address_path);
    assert_rustfmt_clean(&dependent_result_address_boundaries_path);
    assert_rustfmt_clean(&two_pending_result_address_path);
    assert_rustfmt_clean(&two_pending_result_address_boundaries_path);
    assert_rustfmt_clean(&three_pending_result_address_path);
    assert_rustfmt_clean(&three_pending_result_address_fixture_path);
    assert_rustfmt_clean(&three_pending_result_address_boundaries_path);
}

#[test]
fn writeback_result_module_declaration_policy_rejects_non_external_or_wrong_path_modules() {
    let valid = r#"
#[path = "writeback_port/result_support.rs"]
mod result_support;

#[path = "writeback_port/result_classes.rs"]
mod result_classes;

#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#;
    assert!(module_declaration_failures("synthetic.rs", valid, &WRITEBACK_ROOT_MODULES).is_empty());

    for (label, source) in [
        (
            "inline module",
            r#"
#[path = "writeback_port/result_support.rs"]
mod result_support {}
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
        (
            "missing path",
            r#"
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
        (
            "duplicate path",
            r#"
#[path = "wrong.rs"]
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
        (
            "wrong path",
            r#"
#[path = "writeback_port/wrong.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
        (
            "wrong SC path",
            r#"
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/wrong.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
        (
            "wrong dependent-address path",
            r#"
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
#[path = "writeback_port/store_conditional_result.rs"]
mod store_conditional_result;
#[path = "writeback_port/younger_atomic_result.rs"]
mod younger_atomic_result;
#[path = "writeback_port/wrong.rs"]
mod dependent_result_address;
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
"#,
        ),
    ] {
        assert!(
            !module_declaration_failures("synthetic.rs", source, &WRITEBACK_ROOT_MODULES)
                .is_empty(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn writeback_younger_atomic_boundary_module_policy_rejects_wrong_ownership() {
    let valid = r#"
#[path = "younger_atomic_result/boundaries.rs"]
mod boundaries;
"#;
    assert!(
        module_declaration_failures("synthetic.rs", valid, &YOUNGER_ATOMIC_CHILD_MODULES)
            .is_empty()
    );

    for source in [
        "mod boundaries;",
        "#[path = \"wrong.rs\"]\nmod boundaries;",
        "#[path = \"younger_atomic_result/boundaries.rs\"]\nmod boundaries {}",
    ] {
        assert!(!module_declaration_failures(
            "synthetic.rs",
            source,
            &YOUNGER_ATOMIC_CHILD_MODULES
        )
        .is_empty());
    }
}

#[test]
fn writeback_dependent_result_address_boundary_module_policy_rejects_wrong_ownership() {
    let valid = r#"
#[path = "dependent_result_address/boundaries.rs"]
mod boundaries;
#[path = "dependent_result_address/two_pending.rs"]
mod two_pending;
#[path = "dependent_result_address/three_pending.rs"]
mod three_pending;
"#;
    assert!(module_declaration_failures(
        "synthetic.rs",
        valid,
        &DEPENDENT_RESULT_ADDRESS_CHILD_MODULES,
    )
    .is_empty());

    for source in [
        "#[path = \"dependent_result_address/boundaries.rs\"]\nmod boundaries;",
        "#[path = \"dependent_result_address/boundaries.rs\"]\nmod boundaries;\n#[path = \"wrong.rs\"]\nmod two_pending;",
        "#[path = \"dependent_result_address/boundaries.rs\"]\nmod boundaries;\n#[path = \"dependent_result_address/two_pending.rs\"]\nmod two_pending {}",
        "mod boundaries;\n#[path = \"dependent_result_address/two_pending.rs\"]\nmod two_pending;",
        "#[path = \"wrong.rs\"]\nmod boundaries;\n#[path = \"dependent_result_address/two_pending.rs\"]\nmod two_pending;",
        "#[path = \"dependent_result_address/boundaries.rs\"]\nmod boundaries {}\n#[path = \"dependent_result_address/two_pending.rs\"]\nmod two_pending;",
    ] {
        assert!(!module_declaration_failures(
            "synthetic.rs",
            source,
            &DEPENDENT_RESULT_ADDRESS_CHILD_MODULES,
        )
        .is_empty());
    }
}

#[test]
fn writeback_three_pending_result_address_module_policy_rejects_wrong_ownership() {
    let valid = r#"
#[path = "three_pending/boundaries.rs"]
mod boundaries;
#[path = "three_pending/fixture.rs"]
mod fixture;
"#;
    assert!(module_declaration_failures(
        "synthetic.rs",
        valid,
        &THREE_PENDING_RESULT_ADDRESS_CHILD_MODULES,
    )
    .is_empty());

    for source in [
        "#[path = \"three_pending/boundaries.rs\"]\nmod boundaries;",
        "#[path = \"wrong.rs\"]\nmod boundaries;\n#[path = \"three_pending/fixture.rs\"]\nmod fixture;",
        "#[path = \"three_pending/boundaries.rs\"]\nmod boundaries;\n#[path = \"three_pending/fixture.rs\"]\nmod fixture {}",
    ] {
        assert!(!module_declaration_failures(
            "synthetic.rs",
            source,
            &THREE_PENDING_RESULT_ADDRESS_CHILD_MODULES,
        )
        .is_empty());
    }
}

#[test]
fn writeback_two_pending_result_address_boundary_module_policy_rejects_wrong_ownership() {
    let valid = r#"
#[path = "two_pending/boundaries.rs"]
mod boundaries;
"#;
    assert!(module_declaration_failures(
        "synthetic.rs",
        valid,
        &TWO_PENDING_RESULT_ADDRESS_CHILD_MODULES,
    )
    .is_empty());

    for source in [
        "mod boundaries;",
        "#[path = \"wrong.rs\"]\nmod boundaries;",
        "#[path = \"two_pending/boundaries.rs\"]\nmod boundaries {}",
        "#[path = \"wrong.rs\"]\n#[path = \"two_pending/boundaries.rs\"]\nmod boundaries;",
    ] {
        assert!(!module_declaration_failures(
            "synthetic.rs",
            source,
            &TWO_PENDING_RESULT_ADDRESS_CHILD_MODULES,
        )
        .is_empty());
    }
}

#[test]
fn writeback_result_boundary_support_module_policy_rejects_non_external_or_wrong_path_modules() {
    let valid = r#"
#[path = "result_boundaries/support.rs"]
mod support;
"#;
    assert!(boundary_support_module_failures("synthetic.rs", valid).is_empty());

    for (label, source) in [
        (
            "inline module",
            r#"
#[path = "result_boundaries/support.rs"]
mod support {}
"#,
        ),
        (
            "missing path",
            r#"
mod support;
"#,
        ),
        (
            "duplicate path",
            r#"
#[path = "wrong.rs"]
#[path = "result_boundaries/support.rs"]
mod support;
"#,
        ),
        (
            "wrong path",
            r#"
#[path = "result_boundaries/wrong.rs"]
mod support;
"#,
        ),
    ] {
        assert!(
            !boundary_support_module_failures("synthetic.rs", source).is_empty(),
            "{label} must be rejected"
        );
    }
}

#[test]
fn writeback_result_support_leaf_policy_rejects_includes_and_child_modules() {
    assert!(support_leaf_failures("synthetic.rs", "use super::*;\nfn helper() {}\n").is_empty());
    assert!(!support_leaf_failures("synthetic.rs", "mod nested;\n").is_empty());
    assert!(!support_leaf_failures("synthetic.rs", "include!(\"nested.rs\");\n").is_empty());
    assert!(
        !support_leaf_failures("synthetic.rs", "fn helper() { include!(\"nested.rs\"); }\n")
            .is_empty()
    );
    assert!(!support_leaf_failures(
        "synthetic.rs",
        "macro_rules! hidden { () => { include!(\"nested.rs\"); } }\n"
    )
    .is_empty());
    assert!(
        !support_leaf_failures("synthetic.rs", "include!(concat!(\"nested\", \".rs\"));\n")
            .is_empty()
    );
}

#[test]
fn writeback_result_support_helper_policy_rejects_extra_exported_functions() {
    let mut valid = String::new();
    for helper in RESULT_SUPPORT_FUNCTIONS {
        valid.push_str(&format!("pub(super) fn {helper}() {{}}\n"));
    }
    assert!(result_support_function_inventory_failures("synthetic.rs", &valid).is_empty());

    let extra = format!("{valid}pub(super) fn pmp_denied_amo_output() {{}}\n");
    assert!(
        !result_support_function_inventory_failures("synthetic.rs", &extra).is_empty(),
        "extra exported support helper must be rejected"
    );
}

fn top_level_test_names(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            function
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("test"))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn result_class_tests(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            let name = function.sig.ident.to_string();
            (name.starts_with(RESULT_CLASS_TEST_PREFIX)
                && function
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("test")))
            .then_some(name)
        })
        .collect()
}

fn top_level_function_names(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
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

fn top_level_include_paths(relative: &str, source: &str) -> Vec<String> {
    let syntax = parsed_source(relative, source);
    let mut visitor = IncludeMacroVisitor::default();
    visitor.visit_file(&syntax);
    visitor.paths
}

#[derive(Default)]
struct IncludeMacroVisitor {
    paths: Vec<String>,
}

impl<'ast> Visit<'ast> for IncludeMacroVisitor {
    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        if item.path.is_ident("include") {
            self.paths
                .push(top_level_include_argument(item.tokens.clone()));
        }
        nested_include_arguments(item.tokens.clone(), &mut self.paths);
        syn::visit::visit_macro(self, item);
    }
}

fn nested_include_arguments(tokens: proc_macro2::TokenStream, paths: &mut Vec<String>) {
    let tokens = tokens.into_iter().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        if let proc_macro2::TokenTree::Group(group) = token {
            nested_include_arguments(group.stream(), paths);
        }
        let Some(proc_macro2::TokenTree::Ident(ident)) = tokens.get(index) else {
            continue;
        };
        let Some(proc_macro2::TokenTree::Punct(bang)) = tokens.get(index + 1) else {
            continue;
        };
        let Some(proc_macro2::TokenTree::Group(arguments)) = tokens.get(index + 2) else {
            continue;
        };
        if ident == "include" && bang.as_char() == '!' {
            paths.push(top_level_include_argument(arguments.stream()));
        }
    }
}

fn top_level_include_argument(tokens: proc_macro2::TokenStream) -> String {
    syn::parse2::<syn::LitStr>(tokens.clone())
        .map(|literal| literal.value())
        .unwrap_or_else(|_| tokens.to_string())
}

fn top_level_module_names(relative: &str, source: &str) -> Vec<String> {
    top_level_module_declarations(relative, source)
        .into_iter()
        .map(|module| module.name)
        .collect()
}

fn top_level_module_declarations(relative: &str, source: &str) -> Vec<ModuleDeclaration> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            Some(ModuleDeclaration {
                name: module.ident.to_string(),
                path_attributes: module
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("path"))
                    .map(path_attribute_literal)
                    .collect(),
                inline: module.content.is_some(),
            })
        })
        .collect()
}

fn path_attribute_literal(attr: &syn::Attribute) -> Option<String> {
    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };
    let syn::Expr::Lit(expr) = &name_value.value else {
        return None;
    };
    let syn::Lit::Str(literal) = &expr.lit else {
        return None;
    };
    Some(literal.value())
}

fn module_declaration_failures(
    relative: &str,
    source: &str,
    expected: &[ExpectedModuleDeclaration],
) -> Vec<String> {
    let modules = top_level_module_declarations(relative, source);
    let actual_names = modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<Vec<_>>();
    let expected_names = expected
        .iter()
        .map(|module| module.name)
        .collect::<Vec<_>>();

    let mut failures = Vec::new();
    if actual_names != expected_names {
        failures.push(format!(
            "{relative} must declare exactly the normal modules {expected_names:?}"
        ));
    }

    for expected_module in expected {
        let Some(module) = modules
            .iter()
            .find(|module| module.name == expected_module.name)
        else {
            continue;
        };
        if module.inline {
            failures.push(format!(
                "{relative} module `{}` must be an external module declaration",
                expected_module.name
            ));
        }
        match module.path_attributes.as_slice() {
            [] => failures.push(format!(
                "{relative} module `{}` must declare #[path = \"{}\"]",
                expected_module.name, expected_module.path
            )),
            [Some(actual_path)] if actual_path == expected_module.path => {}
            [Some(actual_path)] => failures.push(format!(
                "{relative} module `{}` must use #[path = \"{}\"], got #[path = \"{}\"]",
                expected_module.name, expected_module.path, actual_path
            )),
            [None] => failures.push(format!(
                "{relative} module `{}` must use a string-literal #[path = \"{}\"]",
                expected_module.name, expected_module.path
            )),
            path_attributes => failures.push(format!(
                "{relative} module `{}` must declare exactly one #[path = \"{}\"], found {}",
                expected_module.name,
                expected_module.path,
                path_attributes.len()
            )),
        }
    }
    failures
}

fn boundary_support_module_failures(relative: &str, source: &str) -> Vec<String> {
    module_declaration_failures(relative, source, &RESULT_BOUNDARY_SUPPORT_MODULES)
}

fn result_support_function_inventory_failures(relative: &str, source: &str) -> Vec<String> {
    let support_functions = top_level_function_names(relative, source);
    let expected_functions = RESULT_SUPPORT_FUNCTIONS.to_vec();
    if support_functions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        == expected_functions
    {
        Vec::new()
    } else {
        vec![format!(
            "{relative} must own exactly top-level functions {expected_functions:?}, got {support_functions:?}"
        )]
    }
}

fn support_leaf_failures(relative: &str, source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let includes = top_level_include_paths(relative, source);
    if !includes.is_empty() {
        failures.push(format!(
            "{relative} must not contain top-level include! fragments: {includes:?}"
        ));
    }
    let modules = top_level_module_names(relative, source);
    if !modules.is_empty() {
        failures.push(format!(
            "{relative} must not declare child modules: {modules:?}"
        ));
    }
    failures
}

fn assert_rustfmt_clean(path: &Path) {
    let rustfmt = std::env::var_os("RUSTFMT").unwrap_or_else(|| "rustfmt".into());
    let output = std::process::Command::new(rustfmt)
        .args(["--check", "--edition", "2021"])
        .arg(path)
        .output()
        .unwrap_or_else(|error| panic!("failed to run rustfmt for {}: {error}", path.display()));
    assert!(
        output.status.success(),
        "rustfmt check failed for {}:\nstdout:\n{}\nstderr:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn parsed_source(relative: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| {
        panic!("failed to parse {relative} for writeback ownership: {error}")
    })
}
