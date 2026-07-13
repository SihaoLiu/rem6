use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::{self, Visit};
use syn::{
    Attribute, Expr, ExprLit, ExprRepeat, ImplItemFn, Item, Lit, Meta, TraitItemFn, TypeArray,
};

use super::rust_source_files;

const CONSUMERS: &[(&str, &str)] = &[
    ("CLI config", "src/config.rs"),
    ("host-event config", "src/config/host_event.rs"),
    ("host-action summaries", "src/host_actions.rs"),
    ("run execution summaries", "src/run_execution_summary.rs"),
    (
        "O3 execution-mode debug stats",
        "src/debug_output/o3_execution_mode_stats.rs",
    ),
    (
        "O3 checkpoint-restore debug JSON",
        "src/debug_output/o3_checkpoint_restore_json.rs",
    ),
    ("host-action debug JSON", "src/debug_output/host_action.rs"),
    ("O3 runtime stats", "src/stats_output/o3_runtime.rs"),
    (
        "O3 snapshot/restore stats",
        "src/stats_output/o3_runtime_snapshot_restore.rs",
    ),
    ("host-action stats", "src/stats_output/host_actions.rs"),
    ("CPU checker stats", "src/stats_output/cpu.rs"),
];

const FORBIDDEN_LOCAL_DECLARATIONS: &[&str] = &[
    "EXECUTION_MODE_STAT_LANES",
    "EXECUTION_MODE_AUTHORITY_JSON_LANES",
    "EXECUTION_MODE_STATS",
    "O3_CHECKPOINT_RESTORE_AUTHORITY_STAT_LANES",
    "execution_mode_authority_lane_index",
    "execution_mode_index",
    "parse_execution_mode",
    "execution_mode_name",
];

const FORBIDDEN_STANDALONE_LANE_NAMES: &[&str] = &["functional", "timing", "detailed"];

const FORBIDDEN_STATIC_SUFFIX_FRAGMENTS: &[&str] = &[
    "execution_mode.functional",
    "execution_mode.timing",
    "execution_mode.detailed",
    "checkpoint_restore.execution_mode_authority.mode.functional",
    "checkpoint_restore.execution_mode_authority.mode.timing",
    "checkpoint_restore.execution_mode_authority.mode.detailed",
];

const TEST_ONLY_SOURCES: &[TestOnlySource] = &[TestOnlySource {
    source: "stats_output/host_actions/tests.rs",
    parent: "src/stats_output/host_actions.rs",
    module: "tests",
    path: "host_actions/tests.rs",
}];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TestOnlySource {
    source: &'static str,
    parent: &'static str,
    module: &'static str,
    path: &'static str,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct ProductionSourceFacts {
    declared_items: BTreeSet<String>,
    identifiers: BTreeSet<String>,
    string_literals: Vec<String>,
    fixed_three_arrays: usize,
}

impl<'ast> Visit<'ast> for ProductionSourceFacts {
    fn visit_item(&mut self, item: &'ast Item) {
        if item_has_cfg_test(item) {
            return;
        }
        if let Some(name) = declared_item_name(item) {
            self.declared_items.insert(name.to_string());
        }
        visit::visit_item(self, item);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        self.identifiers.insert(ident.to_string());
    }

    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.string_literals.push(literal.value());
    }

    fn visit_token_stream(&mut self, tokens: &'ast TokenStream) {
        collect_token_stream_string_literals(tokens, &mut self.string_literals);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        if attributes_have_cfg_test(&function.attrs) {
            return;
        }
        self.declared_items.insert(function.sig.ident.to_string());
        visit::visit_impl_item_fn(self, function);
    }

    fn visit_trait_item_fn(&mut self, function: &'ast TraitItemFn) {
        if attributes_have_cfg_test(&function.attrs) {
            return;
        }
        self.declared_items.insert(function.sig.ident.to_string());
        visit::visit_trait_item_fn(self, function);
    }

    fn visit_type_array(&mut self, array: &'ast TypeArray) {
        if expression_is_three(&array.len) {
            self.fixed_three_arrays += 1;
        }
        visit::visit_type_array(self, array);
    }

    fn visit_expr_repeat(&mut self, array: &'ast ExprRepeat) {
        if expression_is_three(&array.len) {
            self.fixed_three_arrays += 1;
        }
        visit::visit_expr_repeat(self, array);
    }
}

#[test]
fn execution_mode_cli_lanes_have_one_representation_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let authority_path = crate_dir.join("src/execution_mode_lanes.rs");

    assert!(
        lib.contains("mod execution_mode_lanes;"),
        "src/lib.rs must declare the shared execution-mode lane authority"
    );
    assert!(
        authority_path.exists(),
        "CLI execution-mode lane mappings belong in src/execution_mode_lanes.rs"
    );

    let authority = fs::read_to_string(authority_path).unwrap();
    for anchor in [
        "macro_rules! define_execution_mode_lanes",
        "pub(crate) const EXECUTION_MODE_LANE_COUNT",
        "pub(crate) const EXECUTION_MODE_LANES",
        "pub(crate) fn execution_mode_from_name(",
        "pub(crate) const fn execution_mode_name(",
        "pub(crate) fn execution_mode_lane_index(",
        "ExecutionMode::$variant => $name",
    ] {
        assert!(
            authority.contains(anchor),
            "execution-mode lane authority is missing `{anchor}`"
        );
    }
    assert_eq!(
        authority.matches("define_execution_mode_lanes! {").count(),
        1,
        "execution-mode lane rows must have one declaration"
    );

    for (name, relative) in CONSUMERS {
        let source = fs::read_to_string(crate_dir.join(relative)).unwrap();
        let facts = production_source_facts(relative, &source);
        assert!(
            facts.identifiers.contains("execution_mode_lanes"),
            "{name} must consume the shared execution-mode lane authority"
        );
        for forbidden in FORBIDDEN_LOCAL_DECLARATIONS {
            assert!(
                !facts.declared_items.contains(*forbidden),
                "{name} must not declare local execution-mode authority `{forbidden}`"
            );
        }
        assert_eq!(
            facts.fixed_three_arrays, 0,
            "{name} must derive every fixed execution-mode counter dimension from the shared authority"
        );
    }

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir.join("src")).unwrap();
        if relative == Path::new("execution_mode_lanes.rs")
            || verified_test_only_source(crate_dir, relative)
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let facts = production_source_facts(&relative.display().to_string(), &source);
        for literal in facts.string_literals {
            assert!(
                !FORBIDDEN_STANDALONE_LANE_NAMES.contains(&literal.as_str())
                    && !FORBIDDEN_STATIC_SUFFIX_FRAGMENTS
                        .iter()
                        .any(|fragment| literal.contains(fragment)),
                "{} must consume the shared execution-mode representation instead of `{literal}`",
                relative.display()
            );
        }
    }
}

#[test]
fn fixed_three_array_detection_covers_type_and_expression_forms() {
    let facts = production_source_facts(
        "fixture.rs",
        r#"
            const FIRST: [u64; 3] = [0; 3];
            const SECOND: [u64; 3] = [0u64; 3];
            const THIRD: [Option<u64>; 3] = [Some(0); 3];
            const SAFE: [u64; EXECUTION_MODE_LANE_COUNT] =
                [0; EXECUTION_MODE_LANE_COUNT];
        "#,
    );

    assert_eq!(facts.fixed_three_arrays, 6);
}

#[test]
fn cfg_test_items_do_not_hide_following_production_items() {
    let facts = production_source_facts(
        "fixture.rs",
        r#"
            #[cfg(test)]
            mod tests {
                const TEST_LANE: &str = "functional";
                const TEST_COUNTS: [u64; 3] = [0; 3];
            }

            const PRODUCTION_AFTER_TESTS: &str = "production";
        "#,
    );

    assert!(!facts
        .string_literals
        .iter()
        .any(|value| value == "functional"));
    assert_eq!(facts.fixed_three_arrays, 0);
    assert!(facts.declared_items.contains("PRODUCTION_AFTER_TESTS"));
    assert!(facts
        .string_literals
        .iter()
        .any(|value| value == "production"));
}

#[test]
fn macro_string_literals_are_scanned() {
    let facts = production_source_facts(
        "fixture.rs",
        r#"
            fn lane_matches(mode: &str) -> bool {
                matches!(mode, "functional" | "timing" | "detailed")
            }
        "#,
    );

    for lane in FORBIDDEN_STANDALONE_LANE_NAMES {
        assert!(facts.string_literals.iter().any(|value| value == lane));
    }
}

#[test]
fn associated_function_declarations_are_scanned() {
    let facts = production_source_facts(
        "fixture.rs",
        r#"
            trait LaneIndex {
                fn execution_mode_index(&self) -> usize;
            }

            struct Lane;

            impl Lane {
                fn execution_mode_name(&self) -> &'static str {
                    "unknown"
                }
            }
        "#,
    );

    assert!(facts.declared_items.contains("execution_mode_index"));
    assert!(facts.declared_items.contains("execution_mode_name"));
}

fn production_source_facts(name: &str, source: &str) -> ProductionSourceFacts {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {name} for source policy: {error}"));
    let mut facts = ProductionSourceFacts::default();
    facts.visit_file(&syntax);
    facts
}

fn item_has_cfg_test(item: &Item) -> bool {
    attributes_have_cfg_test(item_attributes(item))
}

fn attributes_have_cfg_test(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && matches!(
                &attribute.meta,
                Meta::List(list) if list.tokens.to_string() == "test"
            )
    })
}

fn collect_token_stream_string_literals(tokens: &TokenStream, output: &mut Vec<String>) {
    for token in tokens.clone() {
        match token {
            TokenTree::Group(group) => {
                collect_token_stream_string_literals(&group.stream(), output);
            }
            TokenTree::Literal(literal) => {
                if let Ok(literal) = syn::parse_str::<syn::LitStr>(&literal.to_string()) {
                    output.push(literal.value());
                }
            }
            TokenTree::Ident(_) | TokenTree::Punct(_) => {}
        }
    }
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn declared_item_name(item: &Item) -> Option<&syn::Ident> {
    match item {
        Item::Const(item) => Some(&item.ident),
        Item::Enum(item) => Some(&item.ident),
        Item::ExternCrate(item) => Some(&item.ident),
        Item::Fn(item) => Some(&item.sig.ident),
        Item::Mod(item) => Some(&item.ident),
        Item::Static(item) => Some(&item.ident),
        Item::Struct(item) => Some(&item.ident),
        Item::Trait(item) => Some(&item.ident),
        Item::TraitAlias(item) => Some(&item.ident),
        Item::Type(item) => Some(&item.ident),
        Item::Union(item) => Some(&item.ident),
        _ => None,
    }
}

fn expression_is_three(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Lit(ExprLit {
            lit: Lit::Int(value),
            ..
        }) if matches!(value.base10_parse::<usize>(), Ok(3))
    )
}

fn verified_test_only_source(crate_dir: &Path, relative: &Path) -> bool {
    let Some(spec) = TEST_ONLY_SOURCES
        .iter()
        .find(|spec| relative == Path::new(spec.source))
    else {
        return false;
    };
    let parent_source = fs::read_to_string(crate_dir.join(spec.parent)).unwrap();
    let parent = syn::parse_file(&parent_source).unwrap_or_else(|error| {
        panic!(
            "failed to parse test-only source parent {}: {error}",
            spec.parent
        )
    });
    let gated = parent.items.iter().any(|item| {
        let Item::Mod(module) = item else {
            return false;
        };
        module.ident == spec.module
            && module.content.is_none()
            && item_has_cfg_test(item)
            && module_path(module).as_deref() == Some(spec.path)
    });
    assert!(
        gated,
        "test-only source {} must be declared by cfg(test) module {} in {}",
        spec.source, spec.module, spec.parent
    );
    true
}

fn module_path(module: &syn::ItemMod) -> Option<String> {
    module.attrs.iter().find_map(|attribute| {
        if !attribute.path().is_ident("path") {
            return None;
        }
        let Meta::NameValue(value) = &attribute.meta else {
            return None;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(path),
            ..
        }) = &value.value
        else {
            return None;
        };
        Some(path.value())
    })
}
