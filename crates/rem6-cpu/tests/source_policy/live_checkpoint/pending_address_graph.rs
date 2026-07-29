use super::*;

const GRAPH_POLICY: &str = "tests/source_policy/live_checkpoint/pending_address_graph.rs";
const CODEC: &str = "src/riscv_live_checkpoint/codec.rs";
const PENDING_CODEC: &str = "src/riscv_live_checkpoint/codec/pending_address.rs";
const RUNTIME: &str = "src/o3_runtime_live_checkpoint.rs";
const PENDING_SET: &str = "src/o3_runtime_pending_address_set.rs";
const CAPTURE_GRAPH: &str = "src/o3_runtime_live_checkpoint/pending_address/graph.rs";
const GRAPH_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/graph.rs";
const CODEC_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/graph/codec.rs";
const RESTORE_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/graph/restore.rs";
const MATERIALIZATION_TESTS: &str =
    "src/riscv_live_checkpoint_tests/pending_address/graph/restore/materialization.rs";
const SCHEDULING_TESTS: &str =
    "src/riscv_live_checkpoint_tests/pending_address/graph/restore/scheduling.rs";

#[test]
fn pending_address_graph_cpu_sources_are_attached_and_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_unconditional_attachment(
        crate_dir,
        super::POLICY,
        GRAPH_POLICY,
        "pending_address_graph.rs",
        "pending_address_graph",
    );
    for (owner, child, path, module) in [
        (
            "src/riscv_live_checkpoint/codec.rs",
            PENDING_CODEC,
            "codec/pending_address.rs",
            "pending_address",
        ),
        (
            "src/o3_runtime_live_checkpoint/pending_address.rs",
            CAPTURE_GRAPH,
            "pending_address/graph.rs",
            "graph",
        ),
        (GRAPH_TESTS, CODEC_TESTS, "graph/codec.rs", "codec"),
        (GRAPH_TESTS, RESTORE_TESTS, "graph/restore.rs", "restore"),
        (
            RESTORE_TESTS,
            MATERIALIZATION_TESTS,
            "restore/materialization.rs",
            "materialization",
        ),
        (
            RESTORE_TESTS,
            SCHEDULING_TESTS,
            "restore/scheduling.rs",
            "scheduling",
        ),
    ] {
        assert_unconditional_attachment(crate_dir, owner, child, path, module);
    }

    for (relative, maximum) in [
        (GRAPH_POLICY, 280),
        (CODEC, 1_100),
        (PENDING_CODEC, 225),
        (CAPTURE_GRAPH, 625),
        (GRAPH_TESTS, 20),
        (CODEC_TESTS, 750),
        (RESTORE_TESTS, 725),
        (MATERIALIZATION_TESTS, 175),
        (SCHEDULING_TESTS, 100),
        ("src/riscv_core_checkpoint_restore/pending_address.rs", 125),
    ] {
        let path = crate_dir.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn pending_address_graph_wire_runtime_and_rejection_authority_are_mutation_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codec = fs::read_to_string(crate_dir.join(CODEC)).unwrap();
    let pending_codec = fs::read_to_string(crate_dir.join(PENDING_CODEC)).unwrap();
    let runtime = fs::read_to_string(crate_dir.join(RUNTIME)).unwrap();
    let pending_set = fs::read_to_string(crate_dir.join(PENDING_SET)).unwrap();
    let codec_tests = fs::read_to_string(crate_dir.join(CODEC_TESTS)).unwrap();
    let materialization = fs::read_to_string(crate_dir.join(MATERIALIZATION_TESTS)).unwrap();

    let contract = |codec: &str,
                    pending_codec: &str,
                    runtime: &str,
                    pending_set: &str,
                    codec_tests: &str,
                    materialization: &str| {
        graph_contract(
            codec,
            pending_codec,
            runtime,
            pending_set,
            codec_tests,
            materialization,
        )
    };
    assert!(contract(
        &codec,
        &pending_codec,
        &runtime,
        &pending_set,
        &codec_tests,
        &materialization,
    ));

    for (name, mutated) in [
        (
            "legacy version",
            codec.replacen(
                "const VERSION_LEGACY: u8 = 1;",
                "const VERSION_LEGACY: u8 = 0;",
                1,
            ),
        ),
        (
            "pending-single version",
            codec.replacen(
                "const VERSION_PENDING_SINGLE: u8 = 2;",
                "const VERSION_PENDING_SINGLE: u8 = 4;",
                1,
            ),
        ),
        (
            "current version",
            codec.replacen(
                "const VERSION_CURRENT: u8 = 3;",
                "const VERSION_CURRENT: u8 = 4;",
                1,
            ),
        ),
    ] {
        assert_ne!(mutated, codec, "{name} mutation must apply");
        assert!(!contract(
            &mutated,
            &pending_codec,
            &runtime,
            &pending_set,
            &codec_tests,
            &materialization,
        ));
    }

    let singleton = runtime.replacen(
        "pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>",
        "pending_address: Option<RiscvO3LiveCheckpointPendingDataAddress>",
        1,
    );
    assert_ne!(singleton, runtime, "singleton mutation must apply");
    assert!(!contract(
        &codec,
        &pending_codec,
        &singleton,
        &pending_set,
        &codec_tests,
        &materialization,
    ));

    let second_vector = format!(
        "{runtime}\nstruct DuplicatePendingOwner {{ pending_addresses_copy: Vec<RiscvO3LiveCheckpointPendingDataAddress> }}\n"
    );
    assert!(!contract(
        &codec,
        &pending_codec,
        &second_vector,
        &pending_set,
        &codec_tests,
        &materialization,
    ));

    let widened = pending_set.replacen(
        "const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 3;",
        "const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 4;",
        1,
    );
    assert_ne!(widened, pending_set, "capacity mutation must apply");
    assert!(!contract(
        &codec,
        &pending_codec,
        &runtime,
        &widened,
        &codec_tests,
        &materialization,
    ));

    let omitted_v2 = codec_tests.replacen(
        "fn pending_store_v2_fixture_decodes_as_one_logical_row()",
        "fn pending_store_legacy_fixture_is_ignored()",
        1,
    );
    assert_ne!(omitted_v2, codec_tests, "v2 fixture mutation must apply");
    assert!(!contract(
        &codec,
        &pending_codec,
        &runtime,
        &pending_set,
        &omitted_v2,
        &materialization,
    ));

    let weakened = materialization.replacen(
        "assert_materialized_runtime_graph_rejected_atomically();",
        "let _materialized_runtime_graph_is_ignored = true;",
        1,
    );
    assert_ne!(
        weakened, materialization,
        "materialization mutation must apply"
    );
    assert!(!contract(
        &codec,
        &pending_codec,
        &runtime,
        &pending_set,
        &codec_tests,
        &weakened,
    ));
}

fn graph_contract(
    codec: &str,
    pending_codec: &str,
    runtime: &str,
    pending_set: &str,
    codec_tests: &str,
    materialization: &str,
) -> bool {
    let codec = compact_rust_code(&production_rust_source(codec));
    let pending_codec = compact_rust_code(&production_rust_source(pending_codec));
    let runtime = compact_rust_code(&production_rust_source(runtime));
    let pending_set = compact_rust_code(&production_rust_source(pending_set));
    let v2 = unconditional_rust_function_definition(
        codec_tests,
        "pending_store_v2_fixture_decodes_as_one_logical_row",
    );
    let rejection = unconditional_rust_function_definition(
        materialization,
        "pending_load_graph_rejects_materialized_transport_and_bad_stable_owner",
    )
    .map(|source| compact_rust_code(&source));

    codec.contains("constVERSION_LEGACY:u8=1;")
        && codec.contains("constVERSION_PENDING_SINGLE:u8=2;")
        && codec.contains("constVERSION_CURRENT:u8=3;")
        && codec.contains("VERSION_PENDING_SINGLE=>pending_address::read_v2_single(&mutreader)?")
        && codec.contains("VERSION_CURRENT=>pending_address::read_v3_rows(&mutreader)?")
        && pending_codec.contains("fnread_v2_single(")
        && pending_codec.contains("fnread_v3_rows(")
        && runtime.contains("pending_addresses:Vec<RiscvO3LiveCheckpointPendingDataAddress>")
        && runtime
            .matches("Vec<RiscvO3LiveCheckpointPendingDataAddress>")
            .count()
            == 1
        && !runtime.contains("pending_address:Option<RiscvO3LiveCheckpointPendingDataAddress>")
        && pending_set.contains("constO3_PENDING_DATA_ADDRESS_CAPACITY:usize=3;")
        && pending_set.contains("rows:Vec<O3PendingDataAddress>")
        && v2.is_some_and(|source| source.contains("pending-store-v2.bin"))
        && rejection.is_some_and(|source| {
            source.contains("assert_materialized_runtime_graph_rejected_atomically()")
                && source.contains("live.events.push(transport_pending_event(")
        })
}
