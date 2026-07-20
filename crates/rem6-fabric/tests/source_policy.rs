use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{
    Block, Expr, ExprCall, ExprMethodCall, Fields, ImplItem, ImplItemFn, Item, Type, UseTree,
    Visibility,
};

const MAX_FACADE_LINES: usize = 200;
const MAX_SOURCE_LINES: usize = 1800;

#[test]
fn fabric_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);
    let source = fs::read_to_string(&path).unwrap();
    let syntax = syn::parse_file(&source).unwrap();
    let expected_modules = [
        "activity",
        "model",
        "path",
        "qos",
        "snapshot",
        "telemetry",
        "types",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    let mut declared_modules = BTreeSet::new();
    let mut reexported_modules = BTreeSet::new();

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused fabric modules, but it has {lines} lines"
    );
    assert!(
        syntax.attrs.is_empty(),
        "src/lib.rs must not use crate-level attributes to alter the facade"
    );

    for item in syntax.items {
        match item {
            Item::Mod(module) => {
                assert!(
                    module.attrs.is_empty(),
                    "fabric facade modules must not use conditional or path attributes"
                );
                assert!(
                    matches!(module.vis, Visibility::Inherited),
                    "fabric facade modules must remain private"
                );
                assert!(
                    module.content.is_none(),
                    "fabric facade modules must live in separate source files"
                );
                declared_modules.insert(module.ident.to_string());
            }
            Item::Use(item_use) => {
                assert!(
                    item_use.attrs.is_empty(),
                    "fabric facade re-exports must not use conditional attributes"
                );
                assert!(
                    matches!(item_use.vis, Visibility::Public(_)),
                    "fabric facade imports must be public re-exports"
                );
                reexported_modules.insert(use_root(&item_use.tree));
            }
            other => panic!(
                "src/lib.rs must contain only module declarations and public re-exports, found {}",
                item_kind(&other)
            ),
        }
    }

    assert_eq!(declared_modules, expected_modules);
    assert_eq!(reexported_modules, expected_modules);
}

#[test]
fn fabric_runtime_domains_live_in_focused_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (module, expected_public_items) in [
        (
            "activity",
            &[
                "FabricActivityProfile",
                "FabricLaneActivity",
                "FabricLinkActivity",
                "FabricVirtualNetworkActivity",
            ][..],
        ),
        ("model", &["FabricModel", "FabricTransaction"][..]),
        (
            "path",
            &[
                "FabricPath",
                "FabricPathHop",
                "FabricRouterStage",
                "FabricSerialLinkRate",
                "FabricSerialLinkTiming",
            ][..],
        ),
        (
            "qos",
            &[
                "FabricQosRequest",
                "QosError",
                "QosFixedPriorityPolicy",
                "QosGrant",
                "QosPriority",
                "QosPriorityPolicy",
                "QosProportionalFairPolicy",
                "QosProportionalFairPolicySnapshot",
                "QosProportionalFairScoreSnapshot",
                "QosQueueArbiter",
                "QosQueueArbiterSnapshot",
                "QosQueuePolicyKind",
                "QosQueuedRequest",
                "QosRequestId",
                "QosRequestorId",
            ][..],
        ),
        (
            "snapshot",
            &[
                "FabricSnapshot",
                "FabricLaneSnapshot",
                "FabricRouterInputVcSnapshot",
                "FabricRouterOutputPortSnapshot",
            ][..],
        ),
        (
            "telemetry",
            &[
                "FabricActivityMarker",
                "FabricHopActivity",
                "FabricHopTiming",
                "FabricRouterTiming",
                "FabricTransfer",
                "FabricWaitForMarker",
            ][..],
        ),
        (
            "types",
            &[
                "FabricError",
                "FabricLinkId",
                "FabricPacket",
                "FabricPacketId",
                "FabricRouterId",
                "VirtualNetworkId",
            ][..],
        ),
    ] {
        let path = crate_dir.join(format!("src/{module}.rs"));
        assert!(
            path.exists(),
            "fabric {module} code belongs in src/{module}.rs"
        );
        let source = fs::read_to_string(path).unwrap();
        let syntax = syn::parse_file(&source).unwrap();
        let actual_public_items = syntax
            .items
            .iter()
            .filter_map(public_item_name)
            .collect::<BTreeSet<_>>();
        let expected_public_items = expected_public_items
            .iter()
            .copied()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();

        assert_eq!(
            actual_public_items, expected_public_items,
            "src/{module}.rs owns the wrong public fabric items"
        );
    }

    let model_source = fs::read_to_string(crate_dir.join("src/model.rs")).unwrap();
    let model_syntax = syn::parse_file(&model_source).unwrap();
    let model_items = model_syntax
        .items
        .iter()
        .filter_map(item_name)
        .collect::<BTreeSet<_>>();
    for definition in ["FabricLaneKey", "FabricWaitRecord"] {
        assert!(
            model_items.contains(definition),
            "src/model.rs is missing private runtime state `{definition}`"
        );
    }
}

#[test]
fn fabric_hop_activity_uses_one_timing_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden_identifiers = production_forbidden_identifier_hits(
        &crate_dir.join("src"),
        &["FabricRouterActivity", "FabricLaneActivityRecord"],
    );
    assert!(
        forbidden_identifiers.is_empty(),
        "obsolete fabric telemetry identifiers remain in production source: {}",
        forbidden_identifiers.join(", ")
    );
    let telemetry = fs::read_to_string(crate_dir.join("src/telemetry.rs")).unwrap();
    let telemetry_syntax = syn::parse_file(&telemetry).unwrap();
    let model = fs::read_to_string(crate_dir.join("src/model.rs")).unwrap();
    let model_syntax = syn::parse_file(&model).unwrap();

    assert_eq!(
        named_struct_fields(&telemetry_syntax, "FabricHopTiming"),
        [
            "arrival_tick",
            "depart_tick",
            "ingress_tick",
            "link",
            "router",
            "serialization_ticks",
            "start_tick",
            "virtual_network",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert_eq!(
        named_struct_fields(&telemetry_syntax, "FabricHopActivity"),
        [
            "bytes",
            "credit_delay_ticks",
            "flits",
            "hop_index",
            "packet",
            "timing",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    assert!(type_is_single_generic_path(
        named_struct_field_type(&telemetry_syntax, "FabricHopTiming", "router"),
        "Option",
        &["FabricRouterTiming"]
    ));
    assert!(type_path_ends_with(
        named_struct_field_type(&telemetry_syntax, "FabricHopActivity", "timing"),
        &["FabricHopTiming"]
    ));
    let queue_delay_ticks =
        impl_method(&telemetry_syntax, "FabricHopActivity", "queue_delay_ticks");
    assert!(
        matches!(queue_delay_ticks.vis, Visibility::Public(_))
            && queue_delay_ticks.sig.constness.is_some(),
        "FabricHopActivity::queue_delay_ticks must retain its public const API"
    );
    let activity_log = named_struct_field_type(&model_syntax, "FabricModel", "activity_log");
    let Type::Path(activity_log) = activity_log else {
        panic!("FabricModel.activity_log must be Vec<FabricHopActivity>");
    };
    let inner = activity_log
        .path
        .segments
        .last()
        .filter(|segment| segment.ident == "Vec")
        .and_then(|segment| match &segment.arguments {
            syn::PathArguments::AngleBracketed(arguments) => arguments.args.first(),
            _ => None,
        });
    assert!(matches!(
        inner,
        Some(syn::GenericArgument::Type(Type::Path(path)))
            if path.path.is_ident("FabricHopActivity")
    ));
    assert_eq!(
        fabric_hop_construction_shape(
            &impl_method(&model_syntax, "FabricModel", "reserve_transfer").block
        ),
        FabricHopConstructionShape {
            timing_new_calls: 1,
            activity_new_calls: 1,
            canonical_flows: 1,
        },
        "reserve_transfer must construct one canonical timing, clone it into activity, and retain the original in the transfer"
    );
}

#[test]
fn fabric_hop_construction_scan_binds_and_orders_canonical_timing() {
    let canonical = syn::parse_file(
        r#"
        impl FabricModel {
            fn reserve_transfer(&mut self) {
                for hop in hops {
                    let timing = FabricHopTiming::new();
                    let activity = FabricHopActivity::new(timing.clone());
                    self.activity_log.push(activity);
                    timings.push(timing);
                }
            }
        }
        "#,
    )
    .unwrap();
    assert_eq!(
        fabric_hop_construction_shape(
            &impl_method(&canonical, "FabricModel", "reserve_transfer").block
        ),
        FabricHopConstructionShape {
            timing_new_calls: 1,
            activity_new_calls: 1,
            canonical_flows: 1,
        }
    );

    let unrelated = syn::parse_file(
        r#"
        impl FabricModel {
            fn reserve_transfer(&mut self) {
                for hop in hops {
                    let canonical_timing = FabricHopTiming::new();
                    let timing = legacy_timing();
                    let activity = FabricHopActivity::new(timing.clone());
                    self.activity_log.push(activity);
                    timings.push(timing);
                }
            }
        }
        "#,
    )
    .unwrap();
    assert_eq!(
        fabric_hop_construction_shape(
            &impl_method(&unrelated, "FabricModel", "reserve_transfer").block
        ),
        FabricHopConstructionShape {
            timing_new_calls: 1,
            activity_new_calls: 1,
            canonical_flows: 0,
        }
    );
}

#[test]
fn forbidden_identifier_scan_covers_aliases_reexports_and_nested_items() {
    let syntax = syn::parse_file(
        r#"
        type TypeAliasSentinel = u64;
        pub use legacy::CurrentRouterActivity as ReexportAliasSentinel;
        pub use legacy::ReexportTargetSentinel as CurrentLaneActivity;
        mod nested {
            enum NestedItemSentinel {}
        }
        "#,
    )
    .unwrap();

    assert_eq!(
        forbidden_identifier_names(
            &syntax,
            &[
                "TypeAliasSentinel",
                "ReexportAliasSentinel",
                "ReexportTargetSentinel",
                "NestedItemSentinel",
            ]
        ),
        [
            "NestedItemSentinel".to_string(),
            "ReexportAliasSentinel".to_string(),
            "ReexportTargetSentinel".to_string(),
            "TypeAliasSentinel".to_string(),
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn qos_grant_keeps_only_selected_queue_index() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/qos.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();

    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "QosGrant" => Some(&item.fields),
            _ => None,
        })
        .expect("src/qos.rs must define QosGrant");
    let Fields::Named(fields) = fields else {
        panic!("QosGrant must remain a named-field struct");
    };
    let field_names = fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        field_names,
        ["queue_index".to_string()].into_iter().collect(),
        "QosGrant must not cache metadata already owned by its candidate queue"
    );

    let mut public_methods = BTreeSet::new();
    for item in &syntax.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        let Type::Path(self_ty) = item_impl.self_ty.as_ref() else {
            continue;
        };
        if !self_ty.path.is_ident("QosGrant") {
            continue;
        }
        for item in &item_impl.items {
            if let ImplItem::Fn(method) = item {
                if matches!(method.vis, Visibility::Public(_)) {
                    public_methods.insert(method.sig.ident.to_string());
                }
            }
        }
    }
    assert_eq!(
        public_methods,
        ["queue_index".to_string()].into_iter().collect(),
        "QosGrant public access must expose only the selected queue position"
    );
}

#[test]
fn fabric_source_files_stay_within_size_limit() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut oversized = Vec::new();

    for path in rust_source_files(&src_dir) {
        let lines = line_count(&path);
        if lines > MAX_SOURCE_LINES {
            oversized.push(format!(
                "{} has {lines} lines",
                path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap()
                    .display()
            ));
        }
    }

    assert!(
        oversized.is_empty(),
        "fabric source files exceed {MAX_SOURCE_LINES} lines: {}",
        oversized.join(", ")
    );
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}

fn production_forbidden_identifier_hits(root: &Path, forbidden: &[&str]) -> Vec<String> {
    let mut hits = Vec::new();
    for path in rust_source_files(root) {
        let source = fs::read_to_string(&path).unwrap();
        let syntax = syn::parse_file(&source).unwrap();
        for identifier in forbidden_identifier_names(&syntax, forbidden) {
            hits.push(format!(
                "{}::{identifier}",
                path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap()
                    .display()
            ));
        }
    }
    hits
}

fn forbidden_identifier_names(syntax: &syn::File, forbidden: &[&str]) -> BTreeSet<String> {
    let mut visitor = IdentifierCollector::default();
    visitor.visit_file(syntax);
    visitor
        .identifiers
        .into_iter()
        .filter(|identifier| forbidden.contains(&identifier.as_str()))
        .collect()
}

#[derive(Default)]
struct IdentifierCollector {
    identifiers: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for IdentifierCollector {
    fn visit_ident(&mut self, identifier: &'ast syn::Ident) {
        self.identifiers.insert(identifier.to_string());
    }
}

fn named_struct_fields(syntax: &syn::File, name: &str) -> BTreeSet<String> {
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == name => Some(&item.fields),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing struct `{name}`"));
    let Fields::Named(fields) = fields else {
        panic!("struct `{name}` must use named fields");
    };
    fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect()
}

fn named_struct_field_type<'a>(
    syntax: &'a syn::File,
    struct_name: &str,
    field_name: &str,
) -> &'a Type {
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == struct_name => Some(&item.fields),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing struct `{struct_name}`"));
    let Fields::Named(fields) = fields else {
        panic!("struct `{struct_name}` must use named fields");
    };
    &fields
        .named
        .iter()
        .find(|field| {
            field
                .ident
                .as_ref()
                .is_some_and(|ident| ident == field_name)
        })
        .unwrap_or_else(|| panic!("missing `{struct_name}.{field_name}`"))
        .ty
}

fn type_path_ends_with(ty: &Type, expected: &[&str]) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let actual = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    actual.ends_with(
        &expected
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>(),
    )
}

fn type_is_single_generic_path(ty: &Type, outer: &str, inner: &[&str]) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    if segment.ident != outer {
        return false;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    let mut arguments = arguments.args.iter();
    let Some(syn::GenericArgument::Type(inner_ty)) = arguments.next() else {
        return false;
    };
    arguments.next().is_none() && type_path_ends_with(inner_ty, inner)
}

fn impl_method<'a>(syntax: &'a syn::File, self_type: &str, method: &str) -> &'a ImplItemFn {
    syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) => Some(item),
            _ => None,
        })
        .find_map(|item| {
            let Type::Path(self_ty) = item.self_ty.as_ref() else {
                return None;
            };
            if !self_ty.path.is_ident(self_type) {
                return None;
            }
            item.items.iter().find_map(|item| match item {
                ImplItem::Fn(item) if item.sig.ident == method => Some(item),
                _ => None,
            })
        })
        .unwrap_or_else(|| panic!("missing `{self_type}::{method}`"))
}

#[derive(Debug, Default, Eq, PartialEq)]
struct FabricHopConstructionShape {
    timing_new_calls: usize,
    activity_new_calls: usize,
    canonical_flows: usize,
}

fn fabric_hop_construction_shape(block: &Block) -> FabricHopConstructionShape {
    let mut visitor = FabricHopConstructionVisitor::default();
    visitor.visit_block(block);
    visitor.shape.canonical_flows = block
        .stmts
        .iter()
        .filter_map(|statement| match statement {
            syn::Stmt::Expr(Expr::ForLoop(for_loop), _) => Some(&for_loop.body),
            _ => None,
        })
        .filter(|body| block_has_canonical_hop_flow(body))
        .count();
    visitor.shape
}

#[derive(Default)]
struct FabricHopConstructionVisitor {
    shape: FabricHopConstructionShape,
}

impl<'ast> Visit<'ast> for FabricHopConstructionVisitor {
    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if expr_path_ends_with(call.func.as_ref(), &["FabricHopTiming", "new"]) {
            self.shape.timing_new_calls += 1;
        }
        if expr_path_ends_with(call.func.as_ref(), &["FabricHopActivity", "new"]) {
            self.shape.activity_new_calls += 1;
        }
        syn::visit::visit_expr_call(self, call);
    }
}

fn block_has_canonical_hop_flow(block: &Block) -> bool {
    let mut timings = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            local_constructor_binding(statement, &["FabricHopTiming", "new"])
                .map(|(binding, call)| (index, binding, call))
        });
    let Some((timing_index, timing_binding, _)) = timings.next() else {
        return false;
    };
    if timings.next().is_some() {
        return false;
    }

    let mut activities = block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| {
            local_constructor_binding(statement, &["FabricHopActivity", "new"])
                .map(|(binding, call)| (index, binding, call))
        });
    let Some((activity_index, activity_binding, activity_call)) = activities.next() else {
        return false;
    };
    if activities.next().is_some()
        || !activity_call
            .args
            .last()
            .is_some_and(|argument| expr_is_clone_of(argument, &timing_binding))
    {
        return false;
    }

    let activity_pushes = statement_indexes(block, |call| {
        call.method == "push"
            && expr_is_self_field(call.receiver.as_ref(), "activity_log")
            && call
                .args
                .first()
                .is_some_and(|argument| expr_is_path(argument, &activity_binding))
    });
    let timing_pushes = statement_indexes(block, |call| {
        call.method == "push"
            && expr_is_path(call.receiver.as_ref(), "timings")
            && call
                .args
                .first()
                .is_some_and(|argument| expr_is_path(argument, &timing_binding))
    });
    matches!(
        (activity_pushes.as_slice(), timing_pushes.as_slice()),
        ([activity_push], [timing_push])
            if timing_index < activity_index
                && activity_index < *activity_push
                && activity_push < timing_push
    )
}

fn local_constructor_binding<'a>(
    statement: &'a syn::Stmt,
    constructor: &[&str],
) -> Option<(String, &'a ExprCall)> {
    let syn::Stmt::Local(local) = statement else {
        return None;
    };
    let syn::Pat::Ident(binding) = &local.pat else {
        return None;
    };
    let init = local.init.as_ref()?;
    let Expr::Call(call) = init.expr.as_ref() else {
        return None;
    };
    expr_path_ends_with(call.func.as_ref(), constructor).then(|| (binding.ident.to_string(), call))
}

fn statement_indexes(block: &Block, predicate: impl Fn(&ExprMethodCall) -> bool) -> Vec<usize> {
    block
        .stmts
        .iter()
        .enumerate()
        .filter_map(|(index, statement)| match statement {
            syn::Stmt::Expr(Expr::MethodCall(call), _) if predicate(call) => Some(index),
            _ => None,
        })
        .collect()
}

fn expr_path_ends_with(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    let actual = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    actual.ends_with(
        &expected
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>(),
    )
}

fn expr_is_path(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Path(path) if path.path.is_ident(expected))
}

fn expr_is_self_field(expr: &Expr, expected: &str) -> bool {
    matches!(
        expr,
        Expr::Field(field)
            if expr_is_path(field.base.as_ref(), "self")
                && matches!(&field.member, syn::Member::Named(member) if member == expected)
    )
}

fn expr_is_clone_of(expr: &Expr, expected: &str) -> bool {
    matches!(
        expr,
        Expr::MethodCall(call)
            if call.method == "clone" && expr_is_path(call.receiver.as_ref(), expected)
    )
}

fn use_root(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => path.ident.to_string(),
        _ => panic!("fabric facade re-exports must start with a module path"),
    }
}

fn item_name(item: &Item) -> Option<String> {
    match item {
        Item::Enum(item) => Some(item.ident.to_string()),
        Item::Struct(item) => Some(item.ident.to_string()),
        Item::Trait(item) => Some(item.ident.to_string()),
        Item::Type(item) => Some(item.ident.to_string()),
        Item::Union(item) => Some(item.ident.to_string()),
        _ => None,
    }
}

fn public_item_name(item: &Item) -> Option<String> {
    match item {
        Item::Enum(item) if matches!(item.vis, Visibility::Public(_)) => {
            Some(item.ident.to_string())
        }
        Item::Struct(item) if matches!(item.vis, Visibility::Public(_)) => {
            Some(item.ident.to_string())
        }
        Item::Trait(item) if matches!(item.vis, Visibility::Public(_)) => {
            Some(item.ident.to_string())
        }
        Item::Type(item) if matches!(item.vis, Visibility::Public(_)) => {
            Some(item.ident.to_string())
        }
        Item::Union(item) if matches!(item.vis, Visibility::Public(_)) => {
            Some(item.ident.to_string())
        }
        _ => None,
    }
}

fn item_kind(item: &Item) -> &'static str {
    match item {
        Item::Const(_) => "const",
        Item::Enum(_) => "enum",
        Item::Fn(_) => "function",
        Item::Impl(_) => "impl",
        Item::Macro(_) => "macro",
        Item::Mod(_) => "module",
        Item::Static(_) => "static",
        Item::Struct(_) => "struct",
        Item::Trait(_) => "trait",
        Item::Type(_) => "type alias",
        Item::Union(_) => "union",
        Item::Use(_) => "use",
        _ => "unsupported item",
    }
}
