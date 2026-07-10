use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Item, UseTree, Visibility};

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
        ("model", &["FabricModel"][..]),
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
                "FabricRouterActivity",
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
