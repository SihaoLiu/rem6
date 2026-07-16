use super::*;

const COROUTINE_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs";
const COROUTINE_INCLUDES: [&str; 5] = [
    "coroutine/suppression.rs",
    "coroutine/repair.rs",
    "coroutine/lifecycle.rs",
    "coroutine/round_trip.rs",
    "coroutine/round_trip_repair.rs",
];
const COROUTINE_CONCERNS: [CoroutineConcern; 5] = [
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
];

#[derive(Clone, Copy)]
struct CoroutineConcern {
    relative: &'static str,
    anchors: &'static [&'static str],
}

#[test]
fn coroutine_cli_evidence_uses_focused_same_namespace_includes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join(COROUTINE_ROOT)).unwrap();
    let includes = coroutine_include_paths(COROUTINE_ROOT, &source);
    assert_eq!(
        includes, COROUTINE_INCLUDES,
        "{COROUTINE_ROOT} must contain exactly the focused same-namespace includes in order"
    );
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

fn coroutine_include_paths(relative: &str, source: &str) -> Vec<String> {
    parsed_source(relative, source)
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Macro(item) = item else {
                return None;
            };
            if !item.mac.path.is_ident("include") {
                return None;
            }
            let literal = syn::parse2::<syn::LitStr>(item.mac.tokens).ok()?;
            let path = literal.value();
            path.starts_with("coroutine/").then_some(path)
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
