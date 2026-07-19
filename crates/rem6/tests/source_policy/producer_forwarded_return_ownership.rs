use super::*;

const PREDICTED_CONTROL_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control.rs";
const SAME_LINK_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control/same_link.rs";
const RETURN_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/same_link/return_descendant.rs";
const SCALAR_RETURN_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_scalar_return.rs";

#[test]
fn producer_forwarded_return_cli_evidence_has_one_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join(SAME_LINK_ROOT);
    let predicted_root_path = crate_dir.join(PREDICTED_CONTROL_ROOT);
    let child_path = crate_dir.join(RETURN_CHILD);
    let scalar_child_path = crate_dir.join(SCALAR_RETURN_CHILD);
    let root = fs::read_to_string(&root_path).unwrap();
    let predicted_root = fs::read_to_string(&predicted_root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let scalar_child = fs::read_to_string(&scalar_child_path).unwrap();
    let root_syntax = syn::parse_file(&root).unwrap();
    let predicted_root_syntax = syn::parse_file(&predicted_root).unwrap();
    let child_syntax = syn::parse_file(&child).unwrap();
    let scalar_child_syntax = syn::parse_file(&scalar_child).unwrap();
    let root_functions = top_level_function_names(SAME_LINK_ROOT, &root);
    let predicted_root_functions =
        top_level_function_names(PREDICTED_CONTROL_ROOT, &predicted_root);
    let child_functions = top_level_function_names(RETURN_CHILD, &child);
    let scalar_child_functions = top_level_function_names(SCALAR_RETURN_CHILD, &scalar_child);

    for (relative, syntax) in [
        (PREDICTED_CONTROL_ROOT, &predicted_root_syntax),
        (SAME_LINK_ROOT, &root_syntax),
        (RETURN_CHILD, &child_syntax),
        (SCALAR_RETURN_CHILD, &scalar_child_syntax),
    ] {
        let includes = top_level_include_tokens(syntax);
        assert!(
            includes.is_empty(),
            "{relative} must not hide test fragments with include!: {includes:?}"
        );
    }

    assert!(
        module_has_path_attribute(&root, "return_descendant", "same_link/return_descendant.rs",),
        "{SAME_LINK_ROOT} must attach the return-descendant child"
    );
    assert_eq!(
        root.matches("mod return_descendant;").count(),
        1,
        "{SAME_LINK_ROOT} must attach return_descendant exactly once"
    );
    assert!(
        module_has_path_attribute(
            &predicted_root,
            "producer_forwarded_scalar_return",
            "predicted_control/producer_forwarded_scalar_return.rs",
        ),
        "{PREDICTED_CONTROL_ROOT} must attach the scalar-return child"
    );
    assert_eq!(
        predicted_root
            .matches("mod producer_forwarded_scalar_return;")
            .count(),
        1,
        "{PREDICTED_CONTROL_ROOT} must attach producer_forwarded_scalar_return exactly once"
    );
    assert!(!root.contains("mod scalar_return;"));
    assert!(!crate_dir
        .join("tests/cli_run/m5_host_actions/o3/predicted_control/same_link/scalar_return.rs")
        .exists());
    assert!(
        line_count(&root_path) <= 600,
        "{SAME_LINK_ROOT} is oversized"
    );
    assert!(child_path.exists(), "missing {RETURN_CHILD}");
    assert!(scalar_child_path.exists(), "missing {SCALAR_RETURN_CHILD}");
    assert!(
        line_count(&child_path) <= 240,
        "{RETURN_CHILD} is oversized"
    );
    assert!(
        line_count(&scalar_child_path) <= 380,
        "{SCALAR_RETURN_CHILD} is oversized"
    );
    for anchor in [
        "rem6_run_o3_live_same_link_return_descendants_cover_link_and_route_diagonal",
        "rem6_run_o3_live_same_link_return_requires_branch_lookahead_two",
    ] {
        assert_eq!(
            child_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{RETURN_CHILD} must own exactly one `fn {anchor}` definition"
        );
        assert_eq!(
            root_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            0,
            "{SAME_LINK_ROOT} must not own `fn {anchor}`"
        );
    }
    for anchor in [
        "rem6_run_o3_producer_forwarded_scalar_returns_cover_link_shape_route_matrix",
        "rem6_run_o3_producer_forwarded_scalar_return_lookahead_one_keeps_return_unfetched",
        "rem6_run_o3_producer_forwarded_scalar_return_rejects_non_link_scalar",
        "rem6_run_timing_suppresses_o3_producer_forwarded_scalar_returns",
    ] {
        assert_eq!(
            scalar_child_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{SCALAR_RETURN_CHILD} must own exactly one `fn {anchor}` definition"
        );
        for (relative, functions) in [
            (SAME_LINK_ROOT, &root_functions),
            (PREDICTED_CONTROL_ROOT, &predicted_root_functions),
        ] {
            assert_eq!(
                functions
                    .iter()
                    .filter(|function| function.as_str() == anchor)
                    .count(),
                0,
                "{relative} must not own `fn {anchor}`"
            );
        }
    }
}

fn top_level_function_names(relative: &str, source: &str) -> Vec<String> {
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

#[test]
fn producer_forwarded_return_include_policy_detects_spaced_macro() {
    let syntax = syn::parse_file("include ! (concat!(\"hidden\", \".rs\"));\n").unwrap();

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
            item.mac
                .path
                .is_ident("include")
                .then(|| item.mac.tokens.to_string())
        })
        .collect()
}
