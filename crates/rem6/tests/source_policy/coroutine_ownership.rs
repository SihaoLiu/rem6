use super::*;

const COROUTINE_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs";
const COROUTINE_MODULES: [(&str, &str); 7] = [
    ("lifecycle", "coroutine/lifecycle.rs"),
    ("repair", "coroutine/repair.rs"),
    ("round_trip", "coroutine/round_trip.rs"),
    ("round_trip_lifecycle", "coroutine/round_trip_lifecycle.rs"),
    (
        "round_trip_lifecycle_assertions",
        "coroutine/round_trip_lifecycle_assertions.rs",
    ),
    ("round_trip_repair", "coroutine/round_trip_repair.rs"),
    ("suppression", "coroutine/suppression.rs"),
];
const COROUTINE_CONCERNS: [CoroutineConcern; 7] = [
    CoroutineConcern {
        relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs",
        anchors: &[
            "rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two",
            "rem6_run_o3_same_window_indirect_coroutine_requires_branch_lookahead_two",
            "rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal",
            "rem6_run_o3_same_window_indirect_overwritten_coroutine_source_stays_terminal",
        ],
    },
    CoroutineConcern {
        relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs",
        anchors: &[
            "rem6_run_o3_older_branch_discards_same_window_coroutine_chain",
            "rem6_run_o3_older_branch_discards_same_window_indirect_coroutine_chain",
            "rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants",
            "rem6_run_o3_same_window_indirect_coroutine_wrong_target_repairs_descendants",
        ],
    },
    CoroutineConcern {
        relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs",
        anchors: &[
            "rem6_run_host_switch_transfers_o3_same_window_coroutine",
            "rem6_run_o3_same_window_coroutine_checkpoint_boundary",
            "rem6_run_timing_suppresses_o3_same_window_coroutine",
        ],
    },
    CoroutineConcern {
        relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs",
        anchors: &[
            "rem6_run_o3_same_window_coroutine_round_trip_commits_direct",
            "rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram",
        ],
    },
    CoroutineConcern {
        relative:
            "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_repair.rs",
        anchors: &[
            "rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three",
            "rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return",
            "rem6_run_o3_same_window_coroutine_round_trip_terminal_return_repairs_direction",
        ],
    },
    CoroutineConcern {
        relative:
            "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle_assertions.rs",
        anchors: &[],
    },
    CoroutineConcern {
        relative:
            "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle.rs",
        anchors: &[
            "rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip",
            "rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary",
            "rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip",
        ],
    },
];

#[derive(Clone, Copy)]
struct CoroutineConcern {
    relative: &'static str,
    anchors: &'static [&'static str],
}

#[test]
fn coroutine_cli_evidence_uses_focused_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join(COROUTINE_ROOT)).unwrap();
    let syntax = parsed_source(COROUTINE_ROOT, &source);
    let includes = top_level_include_tokens(&syntax);
    assert!(
        includes.is_empty(),
        "{COROUTINE_ROOT} must not inline child sources with include!: {includes:?}"
    );
    let modules = syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Mod(module) = item else {
                return None;
            };
            Some(module.ident.to_string())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        modules,
        COROUTINE_MODULES
            .iter()
            .map(|(module, _)| module.to_string())
            .collect::<Vec<_>>(),
        "{COROUTINE_ROOT} must declare exactly the focused child modules in order"
    );
    for (module, path) in COROUTINE_MODULES {
        assert!(
            module_has_path_attribute(&source, module, path),
            "{COROUTINE_ROOT} must declare `{module}` from `{path}`"
        );
    }
    assert!(line_count(&root.join(COROUTINE_ROOT)) <= 500);

    let root_functions = top_level_function_names(COROUTINE_ROOT, &source);
    let child_functions = COROUTINE_CONCERNS
        .iter()
        .map(|concern| {
            let child_source = fs::read_to_string(root.join(concern.relative)).unwrap();
            let functions = top_level_function_names(concern.relative, &child_source);
            (concern, functions)
        })
        .collect::<Vec<_>>();
    for (owner_index, (owner, owner_functions)) in child_functions.iter().enumerate() {
        assert!(
            line_count(&root.join(owner.relative)) <= 700,
            "{} is oversized",
            owner.relative
        );
        for anchor in owner.anchors {
            assert_eq!(
                function_definition_count(owner_functions, anchor),
                1,
                "{} must own exactly one `fn {anchor}` definition",
                owner.relative
            );
            assert_eq!(
                function_definition_count(&root_functions, anchor),
                0,
                "{COROUTINE_ROOT} must not own `fn {anchor}`"
            );
            for (candidate_index, (candidate, candidate_functions)) in
                child_functions.iter().enumerate()
            {
                if candidate_index == owner_index {
                    continue;
                }
                assert_eq!(
                    function_definition_count(candidate_functions, anchor),
                    0,
                    "{} must not own `fn {anchor}`; it belongs in {}",
                    candidate.relative,
                    owner.relative
                );
            }
        }
    }
}

#[test]
fn coroutine_include_policy_rejects_non_literal_top_level_include() {
    let syntax = parsed_source("synthetic.rs", "include!(concat!(\"nested\", \".rs\"));\n");

    assert_eq!(top_level_include_tokens(&syntax).len(), 1);
}

fn top_level_include_tokens(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Macro(item) = item else {
                return None;
            };
            if !item.mac.path.is_ident("include") {
                return None;
            }
            Some(item.mac.tokens.to_string())
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

fn function_definition_count(functions: &[String], anchor: &str) -> usize {
    functions
        .iter()
        .filter(|function| function.as_str() == anchor)
        .count()
}

fn parsed_source(relative: &str, source: &str) -> syn::File {
    syn::parse_file(source).unwrap_or_else(|error| {
        panic!("failed to parse {relative} for coroutine ownership: {error}")
    })
}
