use super::*;

const PREDICTED_CONTROL_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control.rs";
const PREDICTED_CONTROL_DIR: &str = "tests/cli_run/m5_host_actions/o3/predicted_control";
const PRODUCER_FORWARDED_JALR_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_jalr.rs";

#[test]
fn producer_forwarded_jalr_cli_evidence_has_one_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(PREDICTED_CONTROL_ROOT);
    let child_path = crate_dir.join(PRODUCER_FORWARDED_JALR_CHILD);
    let root = fs::read_to_string(&root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let child_syntax = syn::parse_file(&child).unwrap();

    assert!(module_has_path_attribute(
        &root,
        "producer_forwarded_jalr",
        "predicted_control/producer_forwarded_jalr.rs",
    ));
    assert_eq!(root.matches("mod producer_forwarded_jalr;").count(), 1);
    assert!(child_path.exists());
    assert!(line_count(&child_path) <= 400);
    assert!(top_level_include_tokens(&child_syntax).is_empty());

    let anchors = [
        "rem6_run_o3_producer_forwarded_jalr_targets_cover_link_route_matrix",
        "rem6_run_o3_unresolved_producer_forwarded_jalr_targets_stay_terminal",
        "rem6_run_o3_producer_forwarded_jalr_target_requires_depth_three",
        "rem6_run_timing_suppresses_o3_producer_forwarded_jalr_targets",
    ];
    let child_functions = top_level_function_names(&child_syntax);
    let child_test_functions = top_level_test_function_names(&child_syntax);
    for anchor in anchors {
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

    let mut other_paths = rust_source_files(&crate_dir.join(PREDICTED_CONTROL_DIR));
    other_paths.push(root_path);
    for path in other_paths {
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("fn rem6_run_o3_link_kind_live_target_sources_stay_terminal("),
            "{} retains the removed terminal-only anchor",
            path.strip_prefix(crate_dir).unwrap().display()
        );
        if path == child_path {
            continue;
        }
        let syntax = syn::parse_file(&source).unwrap();
        let functions = top_level_function_names(&syntax);
        for anchor in anchors {
            assert!(
                !functions.iter().any(|function| function == anchor),
                "{} duplicates `fn {anchor}`",
                path.strip_prefix(crate_dir).unwrap().display()
            );
        }
    }
}

fn top_level_function_names(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect()
}

fn top_level_test_function_names(syntax: &syn::File) -> Vec<String> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
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
