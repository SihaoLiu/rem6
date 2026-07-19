use super::*;

const PREDICTED_CONTROL_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control.rs";
const SAME_LINK_ROOT: &str = "tests/cli_run/m5_host_actions/o3/predicted_control/same_link.rs";
const RETURN_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_return.rs";
const LEGACY_RETURN_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/same_link/return_descendant.rs";
const SCALAR_RETURN_CHILD: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/producer_forwarded_scalar_return.rs";

#[test]
fn producer_forwarded_return_cli_evidence_has_one_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let predicted_root_path = crate_dir.join(PREDICTED_CONTROL_ROOT);
    let same_link_root_path = crate_dir.join(SAME_LINK_ROOT);
    let child_path = crate_dir.join(RETURN_CHILD);
    let legacy_child_path = crate_dir.join(LEGACY_RETURN_CHILD);
    let scalar_child_path = crate_dir.join(SCALAR_RETURN_CHILD);
    let predicted_root = fs::read_to_string(&predicted_root_path).unwrap();
    let same_link_root = fs::read_to_string(&same_link_root_path).unwrap();
    let child = fs::read_to_string(&child_path).unwrap();
    let scalar_child = fs::read_to_string(&scalar_child_path).unwrap();
    let predicted_root_syntax = syn::parse_file(&predicted_root).unwrap();
    let same_link_root_syntax = syn::parse_file(&same_link_root).unwrap();
    let child_syntax = syn::parse_file(&child).unwrap();
    let scalar_child_syntax = syn::parse_file(&scalar_child).unwrap();
    let predicted_root_functions =
        top_level_function_names(PREDICTED_CONTROL_ROOT, &predicted_root);
    let same_link_root_functions = top_level_function_names(SAME_LINK_ROOT, &same_link_root);
    let child_functions = top_level_function_names(RETURN_CHILD, &child);
    let scalar_child_functions = top_level_function_names(SCALAR_RETURN_CHILD, &scalar_child);

    for (relative, syntax) in [
        (PREDICTED_CONTROL_ROOT, &predicted_root_syntax),
        (SAME_LINK_ROOT, &same_link_root_syntax),
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
        module_has_path_attribute(
            &predicted_root,
            "producer_forwarded_return",
            "predicted_control/producer_forwarded_return.rs",
        ),
        "{PREDICTED_CONTROL_ROOT} must attach the direct-return child"
    );
    assert_eq!(
        predicted_root
            .matches("mod producer_forwarded_return;")
            .count(),
        1,
        "{PREDICTED_CONTROL_ROOT} must attach producer_forwarded_return exactly once"
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
    assert!(!same_link_root.contains("mod return_descendant;"));
    assert!(
        !legacy_child_path.exists(),
        "obsolete {LEGACY_RETURN_CHILD} remains"
    );
    assert!(!same_link_root.contains("mod scalar_return;"));
    assert!(!crate_dir
        .join("tests/cli_run/m5_host_actions/o3/predicted_control/same_link/scalar_return.rs")
        .exists());
    assert!(
        line_count(&same_link_root_path) <= 600,
        "{SAME_LINK_ROOT} is oversized"
    );
    assert!(child_path.exists(), "missing {RETURN_CHILD}");
    assert!(scalar_child_path.exists(), "missing {SCALAR_RETURN_CHILD}");
    assert!(
        line_count(&child_path) <= 320,
        "{RETURN_CHILD} is oversized"
    );
    assert!(
        line_count(&scalar_child_path) <= 380,
        "{SCALAR_RETURN_CHILD} is oversized"
    );
    for anchor in [
        "rem6_run_o3_producer_forwarded_return_descendants_cover_link_shape_route_matrix",
        "rem6_run_o3_producer_forwarded_return_requires_branch_lookahead_two",
        "rem6_run_timing_suppresses_o3_producer_forwarded_returns",
    ] {
        assert_eq!(
            child_functions
                .iter()
                .filter(|function| function.as_str() == anchor)
                .count(),
            1,
            "{RETURN_CHILD} must own exactly one `fn {anchor}` definition"
        );
        for (relative, functions) in [
            (SAME_LINK_ROOT, &same_link_root_functions),
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
            (SAME_LINK_ROOT, &same_link_root_functions),
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
