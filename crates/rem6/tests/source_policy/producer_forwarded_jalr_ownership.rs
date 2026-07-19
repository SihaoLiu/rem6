use super::*;

const PREDICTED_CONTROL_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control.rs";
const PREDICTED_CONTROL_DIR: &str = "tests/cli_run/m5_host_actions/o3/predicted_control";
const PRODUCER_FORWARDED_JALR_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_jalr.rs";
const PRODUCER_FORWARDED_LINEAGE_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_lineage.rs";
const MAX_PRODUCER_FORWARDED_LINEAGE_LINES: usize = 900;

#[test]
fn producer_forwarded_jalr_cli_evidence_has_one_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(PREDICTED_CONTROL_ROOT);
    let child_path = crate_dir.join(PRODUCER_FORWARDED_JALR_CHILD);
    let lineage_child_path = crate_dir.join(PRODUCER_FORWARDED_LINEAGE_CHILD);
    let root = fs::read_to_string(&root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let lineage_child = fs::read_to_string(&lineage_child_path).unwrap();
    let child_syntax = syn::parse_file(&child).unwrap();
    let lineage_child_syntax = syn::parse_file(&lineage_child).unwrap();

    assert!(module_has_path_attribute(
        &root,
        "producer_forwarded_jalr",
        "predicted_control/producer_forwarded_jalr.rs",
    ));
    assert_eq!(root.matches("mod producer_forwarded_jalr;").count(), 1);
    assert!(child_path.exists());
    assert!(line_count(&child_path) <= 400);
    assert!(top_level_include_tokens(&child_syntax).is_empty());
    assert!(module_has_path_attribute(
        &root,
        "producer_forwarded_lineage",
        "predicted_control/producer_forwarded_lineage.rs",
    ));
    assert_eq!(root.matches("mod producer_forwarded_lineage;").count(), 1);
    assert!(lineage_child_path.exists());
    assert!(line_count(&lineage_child_path) <= MAX_PRODUCER_FORWARDED_LINEAGE_LINES);
    assert!(top_level_include_tokens(&lineage_child_syntax).is_empty());

    let anchors = [
        "rem6_run_o3_producer_forwarded_jalr_targets_cover_link_route_matrix",
        "rem6_run_o3_unresolved_producer_forwarded_jalr_targets_stay_terminal",
        "rem6_run_o3_producer_forwarded_jalr_target_requires_depth_three",
        "rem6_run_timing_suppresses_o3_producer_forwarded_jalr_targets",
    ];
    let lineage_anchors = [
        "rem6_run_o3_nonadjacent_producer_forwarded_jalr_targets_cover_link_route_matrix",
        "rem6_run_o3_warmed_producer_forwarded_targets_issue_descendants_before_load_response",
        "rem6_run_o3_warmed_producer_forwarded_target_descendant_requires_depth_four",
        "rem6_run_o3_warmed_target_does_not_bypass_unresolved_jalr_source",
        "rem6_run_host_switch_transfers_nonadjacent_producer_forwarded_jalr_window",
        "rem6_run_rejects_live_warmed_producer_forwarded_jalr_checkpoint",
        "rem6_run_timing_suppresses_producer_forwarded_lineage_windows",
    ];
    let child_functions = function_names(&child_syntax);
    let child_test_functions = test_function_names(&child_syntax);
    for anchor in anchors.iter().copied() {
        assert_eq!(
            child_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{PRODUCER_FORWARDED_JALR_CHILD} must own exactly one `fn {anchor}`"
        );
        assert_eq!(
            child_test_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{PRODUCER_FORWARDED_JALR_CHILD} must keep `fn {anchor}` test-attributed"
        );
    }
    let lineage_child_functions = function_names(&lineage_child_syntax);
    let lineage_child_test_functions = test_function_names(&lineage_child_syntax);
    for anchor in lineage_anchors.iter().copied() {
        assert_eq!(
            lineage_child_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{PRODUCER_FORWARDED_LINEAGE_CHILD} must own exactly one `fn {anchor}`"
        );
        assert_eq!(
            lineage_child_test_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{PRODUCER_FORWARDED_LINEAGE_CHILD} must keep `fn {anchor}` test-attributed"
        );
    }

    let mut other_paths = rust_source_files(&crate_dir.join(PREDICTED_CONTROL_DIR));
    other_paths.push(root_path);
    for path in other_paths {
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("fn rem6_run_o3_link_kind_live_target_sources_stay_terminal("),
            "{} retains the removed terminal-only anchor",
            path.strip_prefix(crate_dir).unwrap().display()
        );
        let syntax = syn::parse_file(&source).unwrap();
        let functions = function_names(&syntax);
        if path != child_path {
            for anchor in anchors.iter().copied() {
                assert!(
                    !functions.iter().any(|function| function == anchor),
                    "{} duplicates `fn {anchor}`",
                    path.strip_prefix(crate_dir).unwrap().display()
                );
            }
        }
        if path != lineage_child_path {
            for anchor in lineage_anchors.iter().copied() {
                assert!(
                    !functions.iter().any(|function| function == anchor),
                    "{} duplicates `fn {anchor}`",
                    path.strip_prefix(crate_dir).unwrap().display()
                );
            }
        }
    }
}

#[test]
fn producer_forwarded_jalr_function_scan_descends_into_inline_modules() {
    let syntax = syn::parse_file(
        r#"
        fn outer_helper() {}
        mod nested {
            #[test]
            fn duplicate_anchor() {}
        }
        "#,
    )
    .unwrap();

    assert_eq!(
        function_names(&syntax),
        ["outer_helper", "duplicate_anchor"]
    );
    assert_eq!(test_function_names(&syntax), ["duplicate_anchor"]);
}

fn function_names(syntax: &syn::File) -> Vec<String> {
    let mut names = Vec::new();
    collect_function_names(&syntax.items, false, &mut names);
    names
}

fn test_function_names(syntax: &syn::File) -> Vec<String> {
    let mut names = Vec::new();
    collect_function_names(&syntax.items, true, &mut names);
    names
}

fn collect_function_names(items: &[syn::Item], tests_only: bool, names: &mut Vec<String>) {
    for item in items {
        match item {
            syn::Item::Fn(function)
                if !tests_only
                    || function
                        .attrs
                        .iter()
                        .any(|attribute| attribute.path().is_ident("test")) =>
            {
                names.push(function.sig.ident.to_string());
            }
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    collect_function_names(items, tests_only, names);
                }
            }
            _ => {}
        }
    }
}

fn top_level_include_tokens(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Macro(item) = item else {
                return None;
            };
            item.mac
                .path
                .is_ident("include")
                .then(|| item.mac.tokens.to_string())
        })
        .collect()
}
