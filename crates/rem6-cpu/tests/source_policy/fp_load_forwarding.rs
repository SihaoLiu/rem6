use super::*;

const POLICY_PATH: &str = "source_policy/fp_load_forwarding.rs";
const MEMORY_RESULT_PARENT: &str = "src/o3_runtime_memory_result_tests.rs";
const MEMORY_RESULT_TESTS: &str = "src/o3_runtime_memory_result_tests/fp_load_forwarding.rs";
const MEMORY_RESULT_PAIRS: &str = "src/o3_runtime_memory_result_tests/fp_load_forwarding/pairs.rs";
const CONTROL_PARENT: &str = "src/o3_runtime_control_window_tests.rs";
const CONTROL_TESTS: &str = "src/o3_runtime_control_window_tests/fp_load_forwarding.rs";
const QUEUE_PARENT: &str = "src/o3_runtime_issue/queue_tests.rs";
const QUEUE_TESTS: &str = "src/o3_runtime_issue/queue_tests/fp_load_forwarding.rs";
const SERVICE_PARENT: &str = "src/o3_runtime_issue/service_tests.rs";
const SERVICE_TESTS: &str = "src/o3_runtime_issue/service_tests/fp_load_forwarding.rs";
const DATA_ISSUE_PARENT: &str = "src/riscv_data_issue_tests.rs";
const DATA_ISSUE_TESTS: &str = "src/riscv_data_issue_tests/fp_load_forwarding_cleanup.rs";
const EXPECTED_CLASSIFY_COMPUTE_FINGERPRINT: u64 = 10_044_333_624_911_322_285;
const EXPECTED_LIVE_COMPUTE_OPERANDS_FINGERPRINT: u64 = 9_704_551_913_495_967_811;
const EXPECTED_QUEUE_FAMILY_FINGERPRINTS: [u64; 3] = [
    10_750_045_079_659_943_317,
    3_813_067_519_236_924_062,
    5_220_401_298_704_923_230,
];
const EXPECTED_MEMORY_RESULT_WRAPPER: &str = concat!(
    "fnfrom_memory_results(integer_destinations:implIntoIterator<Item=Register>,",
    "occupied_rows:usize,row_limit:usize,)->Option<Self>{",
    "Self::from_memory_result_destinations(integer_destinations.into_iter().map(",
    "O3ArchitecturalRegister::integer),occupied_rows,row_limit,)}",
);
const EXPECTED_SOURCE_PRODUCERS: &str = concat!(
    "fnsource_producers(runtime:&O3RuntimeState,consumer_index:usize,sources:&[",
    "O3ArchitecturalRegister],)->Vec<O3LiveIssueSourceProducer>{letmutproducers=Vec::new();",
    "forsourceinsources.iter().copied().filter(|source|{source.register_class()!=",
    "O3RegisterClass::Integer||source.architectural()!=0}){letproducer=runtime.snapshot.",
    "reorder_buffer[..consumer_index].iter().rev().copied().find(|producer|{producer.",
    "is_live_staged()&&producer.rename_destination()==Some((source.register_class(),",
    "source.architectural()))});ifletSome(producer)=producer{letproducer=",
    "O3LiveIssueSourceProducer{sequence:producer.sequence(),source,};if!producers.",
    "contains(&producer){producers.push(producer);}}}producers}",
);
const EXPECTED_MATERIALIZE_CANDIDATE: &str = concat!(
    "fnmaterialize_candidate(runtime:&O3RuntimeState,scheduling:&O3LiveIssueSchedulingCandidate,)",
    "->Option<O3LiveSpeculativeIssueCandidate>{letmutproducer_sequences=Vec::new();",
    "letmutforwarded_values=Vec::new();letmutforwarded_ready_tick=0;forproducerin",
    "scheduling.data_producers.iter().copied(){let(value,ready_tick)=matchruntime.",
    "live_issue_source_value(producer.sequence(),producer.source()){Some((value,ready_tick))=>",
    "(Some(value),ready_tick),Noneifscheduling.is_pending_data_address()=>{letregister=producer.",
    "source().integer_register()?;letready_tick=runtime.pending_data_address_committed_producer_",
    "ready_tick(producer.sequence(),register,)?;(None,ready_tick)}None=>returnNone,};if!producer_",
    "sequences.contains(&producer.sequence()){producer_sequences.push(producer.sequence());}",
    "ifletSome(value)=value{if!forwarded_values.iter().any(|forwarded:&",
    "O3LiveIssueForwardedValue|{forwarded.architectural_register()==producer.source()}){",
    "forwarded_values.push(value);}}forwarded_ready_tick=forwarded_ready_tick.max(ready_tick);}",
    "ifletSome(control_sequence)=scheduling.control_dependency{if!producer_sequences.contains(",
    "&control_sequence){producer_sequences.push(control_sequence);}}Some(",
    "O3LiveSpeculativeIssueCandidate{scheduling:scheduling.clone(),producer_sequences,",
    "forwarded_values,forwarded_ready_tick,})}",
);
const EXPECTED_COMPLETED_LOAD_SOURCE: &str = concat!(
    "fncompleted_live_data_access_source(&self,sequence:u64,source:O3ArchitecturalRegister,)",
    "->Option<(O3LiveIssueForwardedValue,u64)>{letmutmatches=self.live_data_accesses.iter().",
    "filter(|live|{live.sequence==sequence&&live.outcome==O3LiveDataAccessOutcome::Completed});",
    "letlive=matches.next()?;ifmatches.next().is_some(){returnNone;}letdata=live.load_data.",
    "as_deref()?;letwriteback=live.execution.execution().memory_access()?.read_response_writeback(",
    "data).ok()??;letready_tick=self.memory_result_writeback_reservation(sequence)?.admitted_tick();",
    "letvalue=match(source.register_class(),writeback.target()){(O3RegisterClass::Integer,",
    "MemoryResponseWritebackTarget::Integer(register))ifsource.integer_register()==Some(register)",
    "=>{O3LiveIssueForwardedValue::Integer(RegisterWrite::new(register,writeback.value()))}",
    "(O3RegisterClass::FloatingPoint,MemoryResponseWritebackTarget::Float(register))ifsource.",
    "float_register()==Some(register)=>{O3LiveIssueForwardedValue::FloatingPoint(",
    "FloatRegisterWrite::new(register,writeback.value(),))}_=>returnNone,};Some((value,ready_tick))}",
);

#[test]
fn fp_load_forwarding_focused_test_owners_are_unconditional_and_bounded() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy_parent = fs::read_to_string(root.join("tests/source_policy.rs")).unwrap();
    let policy_child = fs::read_to_string(root.join("tests").join(POLICY_PATH)).unwrap();
    assert_unconditional_attachment(
        &policy_parent,
        &policy_child,
        POLICY_PATH,
        "fp_load_forwarding",
    );
    assert!(policy_child.lines().count() <= 600);

    let owners: &[(&str, &str, &str, &str, usize, &[&str])] = &[
        (
            MEMORY_RESULT_PARENT,
            MEMORY_RESULT_TESTS,
            "o3_runtime_memory_result_tests/fp_load_forwarding.rs",
            "fp_load_forwarding",
            350,
            &[
                "memory_result_runtime_stages_flw_and_fld_consumers_at_typed_dependency_boundary",
                "memory_depth_one_with_deeper_live_window_stages_fp_consumer",
                "prefixed_fp_load_consumer_stages_before_response",
                "fp_load_vector_destination_remains_unforwardable",
                "fp_load_wrong_class_waw_never_satisfies_consumer",
            ],
        ),
        (
            MEMORY_RESULT_TESTS,
            MEMORY_RESULT_PAIRS,
            "fp_load_forwarding/pairs.rs",
            "pairs",
            200,
            &[
                "memory_result_runtime_pair_blocks_fp_head_and_younger_fp_destinations",
                "memory_result_runtime_pair_retains_same_fp_destination_rows_by_sequence",
                "memory_result_runtime_pair_keeps_same_index_fp_and_integer_destinations_distinct",
                "memory_result_runtime_pair_rejects_matching_vector_to_scalar_consumer",
            ],
        ),
        (
            CONTROL_PARENT,
            CONTROL_TESTS,
            "o3_runtime_control_window_tests/fp_load_forwarding.rs",
            "fp_load_forwarding",
            160,
            &[
                "fp_load_source_materializes_word_and_double_values_at_admitted_writeback",
                "fp_load_source_rejects_wrong_class_wrong_register_and_missing_reservation",
                "fp_load_missing_or_short_response_never_materializes_a_source",
            ],
        ),
        (
            QUEUE_PARENT,
            QUEUE_TESTS,
            "queue_tests/fp_load_forwarding.rs",
            "fp_load_forwarding",
            280,
            &[
                "fp_load_queue_blocks_before_response_and_before_writeback_admission",
                "fp_load_queue_wakes_exactly_at_admitted_memory_result_writeback",
                "fp_load_pending_address_fallback_remains_integer_only",
            ],
        ),
        (
            SERVICE_PARENT,
            SERVICE_TESTS,
            "service_tests/fp_load_forwarding.rs",
            "fp_load_forwarding",
            525,
            &[
                "fp_load_service_uses_clone_without_mutating_canonical_hart",
                "fp_load_service_handles_nearest_fp_waw_and_two_producer_fan_in",
                "fp_load_retry_clears_value_reservation_and_dependent_speculation",
                "fp_load_terminal_failure_invalidates_dependent_fp_suffix",
            ],
        ),
        (
            DATA_ISSUE_PARENT,
            DATA_ISSUE_TESTS,
            "riscv_data_issue_tests/fp_load_forwarding_cleanup.rs",
            "fp_load_forwarding_cleanup",
            325,
            &[
                "fp_load_retry_cleans_production_request_maps_before_fresh_flw_attempt",
                "fp_load_failure_cleans_production_request_maps_before_fresh_fld_attempt",
            ],
        ),
    ];

    for (parent_path, child_path, module_path, module, maximum, tests) in owners {
        let parent = fs::read_to_string(root.join(parent_path)).unwrap();
        let child = fs::read_to_string(root.join(child_path)).unwrap();
        assert_unconditional_attachment(&parent, &child, module_path, module);
        assert!(
            line_count(&root.join(child_path)) <= *maximum,
            "{child_path} exceeds its {maximum}-line cap",
        );
        let test_inventory = if *child_path == MEMORY_RESULT_TESTS {
            child.replacen(
                "#[path = \"fp_load_forwarding/pairs.rs\"]\nmod pairs;",
                "",
                1,
            )
        } else {
            child.clone()
        };
        assert!(
            expected_tests_are_unconditional(&test_inventory, tests),
            "{child_path} must define only the expected unconditional tests",
        );

        let attachment = format!("#[path = \"{module_path}\"]\nmod {module};");
        for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
            let mutated = parent.replacen(&attachment, &format!("{conditional}{attachment}"), 1);
            assert_ne!(
                mutated, parent,
                "attachment mutation must apply to {parent_path}"
            );
            assert_eq!(
                active_unconditional_path_owned_module_declaration_count(
                    &mutated,
                    &child,
                    module_path,
                    module,
                ),
                0,
            );
        }
    }
}

#[test]
fn fp_load_forwarding_keeps_one_typed_memory_result_inventory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let memory = read(root, "src/o3_runtime_memory.rs");
    let memory_window = read(root, "src/o3_runtime_memory_window.rs");
    let window_policy = read(root, "src/riscv_o3_window_policy.rs");
    assert_eq!(
        source_fingerprint(&function(root_function(
            &window_policy,
            "classify_compute_younger",
        ))),
        EXPECTED_CLASSIFY_COMPUTE_FINGERPRINT,
    );
    assert!(typed_memory_result_contract(
        &memory,
        &memory_window,
        &window_policy,
    ));

    let duplicate = format!(
        "{memory_window}\nstruct O3MemoryResultWindowState {{ rows: usize, destinations: Vec<O3ArchitecturalRegister> }}\n"
    );
    assert!(!typed_memory_result_contract(
        &memory,
        &duplicate,
        &window_policy,
    ));

    let exact_membership = ".any(|source| self.unresolved_destinations.contains(source))";
    let erased_membership = ".any(|source| self.unresolved_destinations.iter().any(|unresolved| unresolved.architectural() == source.architectural()))";
    let erased = window_policy.replacen(exact_membership, erased_membership, 1);
    assert_ne!(
        erased, window_policy,
        "typed-membership mutation must apply"
    );
    assert!(!typed_memory_result_contract(
        &memory,
        &memory_window,
        &erased,
    ));

    let decoy = erased.replacen(
        "let depends_on_unresolved_destination = operands",
        concat!(
            "let _typed_membership_decoy = operands.sources().iter()",
            ".any(|source| self.unresolved_destinations.contains(source));\n",
            "        let depends_on_unresolved_destination = operands",
        ),
        1,
    );
    assert_ne!(decoy, erased, "typed-membership decoy mutation must apply");
    assert!(!typed_memory_result_contract(
        &memory,
        &memory_window,
        &decoy,
    ));
}

#[test]
fn fp_load_forwarding_uses_response_owned_conversion_and_exact_typed_lookup() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let queue = read(root, "src/o3_runtime_issue/queue.rs");
    let compute = read(root, "src/o3_runtime_issue/queue/compute.rs");
    let forwarding = read(root, "src/o3_runtime_issue/queue/forwarding.rs");
    let control = read(root, "src/o3_runtime_control_window.rs");
    assert_eq!(
        queue_family_fingerprints(&queue, &compute, &forwarding),
        EXPECTED_QUEUE_FAMILY_FINGERPRINTS,
    );
    assert!(forwarding_contract(&queue, &compute, &forwarding, &control,));

    let third_variant = forwarding.replacen(
        "FloatingPoint(FloatRegisterWrite),",
        "FloatingPoint(FloatRegisterWrite),\n    Vector(Vec<u8>),",
        1,
    );
    assert_ne!(third_variant, forwarding, "variant mutation must apply");
    assert!(!forwarding_contract(
        &queue,
        &compute,
        &third_variant,
        &control,
    ));

    for decoder in [
        "fn forbidden_queue_decode() -> u32 { u32::from_le_bytes([0; 4]) }",
        "fn forbidden_queue_decode() -> u64 { u64::from_be_bytes([0; 8]) }",
        "fn forbidden_queue_decode(bytes: &[u64]) -> u64 { bytes[0] }",
        "fn forbidden_queue_decode(value: u64) -> u64 { value >> 8 }",
        concat!(
            "fn forbidden_queue_decode(payload: &[u64]) -> u64 { payload.iter().copied()",
            ".enumerate().map(|(offset, octet)| octet * 256_u64.pow(offset as u32)).sum() }",
        ),
    ] {
        let byte_decode = format!("{forwarding}\n{decoder}\n");
        assert!(!forwarding_contract(
            &queue,
            &compute,
            &byte_decode,
            &control,
        ));
    }

    let typed_lookup = "Some((source.register_class(), source.architectural()))";
    let erased_lookup = "Some((O3RegisterClass::Integer, source.architectural()))";
    let erased = forwarding.replacen(typed_lookup, erased_lookup, 1);
    assert_ne!(erased, forwarding, "typed lookup mutation must apply");
    assert!(!forwarding_contract(&queue, &compute, &erased, &control,));
}

#[test]
fn fp_load_forwarding_retains_supported_arithmetic_and_clone_isolation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let operands = read(root, "src/o3_live_compute_operands.rs");
    let operand_tests = read(root, "src/o3_live_compute_operands_tests.rs");
    let service = read(root, "src/o3_runtime_issue/service.rs");
    let compact_operands = function(root_function(&operands, "o3_live_compute_operands"));
    assert_eq!(
        source_fingerprint(&compact_operands),
        EXPECTED_LIVE_COMPUTE_OPERANDS_FINGERPRINT,
    );
    let compact_operand_tests =
        compact_rust_code(&rust_code_without_comments_and_literals(&operand_tests));

    for (single, double) in [
        ("FloatAddS{", "FloatAddD{"),
        ("FloatSubS{", "FloatSubD{"),
        ("FloatMulS{", "FloatMulD{"),
        ("FloatDivS{", "FloatDivD{"),
        ("FloatMultiplyAddS{", "FloatMultiplyAddD{"),
        ("FloatMultiplySubtractS{", "FloatMultiplySubtractD{"),
        (
            "FloatNegativeMultiplySubtractS{",
            "FloatNegativeMultiplySubtractD{",
        ),
        ("FloatNegativeMultiplyAddS{", "FloatNegativeMultiplyAddD{"),
        ("FloatSqrtS{", "FloatSqrtD{"),
    ] {
        assert_eq!(
            (
                compact_operands.matches(single).count(),
                compact_operands.matches(double).count(),
            ),
            (1, 1),
            "S/D inventory changed for {single} and {double}",
        );
    }
    assert!(compact_operands.contains("_=>None"));
    for unsupported in [
        "FloatLessOrEqualS",
        "FloatEqualD",
        "FloatConvertSFromW",
        "FloatMoveXFromS",
        "FloatClassD",
        "VectorAddVv",
        "MoveFromScalar",
    ] {
        assert!(!operands.contains(unsupported));
        assert!(compact_operand_tests.contains(unsupported));
    }

    let missing_arm = operands.replacen(
        "RiscvInstruction::FloatAddS { rd, rs1, rs2, .. }\n        | ",
        "",
        1,
    );
    assert_ne!(
        missing_arm, operands,
        "active S dispatch mutation must apply"
    );
    let dead_decoy = missing_arm.replacen(
        "    match instruction {",
        "    let _dispatch_decoy = stringify!(FloatAddS {});\n\n    match instruction {",
        1,
    );
    assert_ne!(dead_decoy, missing_arm, "dead dispatch decoy must apply");
    let compact_decoy = function(root_function(&dead_decoy, "o3_live_compute_operands"));
    assert_ne!(
        source_fingerprint(&compact_decoy),
        EXPECTED_LIVE_COMPUTE_OPERANDS_FINGERPRINT,
        "inactive dispatch must change the normalized owner",
    );

    let prepare = rust_function_definition(&service, "prepare_live_issue_batch")
        .expect("prepare_live_issue_batch owner");
    let prepare = compact_rust_code(&prepare);
    assert!(prepare.starts_with(
        "fnprepare_live_issue_batch(&self,hart:&RiscvHartState,queue:&O3LiveIssueQueue,"
    ));
    assert!(!prepare.contains("unsafe"));
    assert_eq!(
        prepare
            .matches("letmutspeculative_hart=hart.clone();")
            .count(),
        1
    );
    assert_eq!(
        prepare
            .matches("speculative_hart.write(write.register(),write.value());")
            .count(),
        1,
    );
    assert_eq!(
        prepare
            .matches("speculative_hart.write_float(write.register(),write.value());")
            .count(),
        1,
    );
    assert_eq!(prepare.matches("hart.write(").count(), 1);
    assert_eq!(prepare.matches("hart.write_float(").count(), 1);
}

#[test]
fn fp_load_forwarding_preserves_o3_compatibility_versions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime = read(root, "src/o3_runtime_checkpoint.rs");
    let pending = read(root, "src/o3_pipeline.rs");
    let handoff = read(root, "src/riscv_execution_mode_handoff/codec.rs");
    assert!(compatibility_version_contract(&runtime, &pending, &handoff));

    let runtime_mutated = runtime.replacen(
        "O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 23",
        "O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 24",
        1,
    );
    assert_ne!(runtime_mutated, runtime, "O3RT mutation must apply");
    assert!(!compatibility_version_contract(
        &runtime_mutated,
        &pending,
        &handoff,
    ));
    let pending_mutated = pending.replacen(
        "O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 2",
        "O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 3",
        1,
    );
    assert_ne!(pending_mutated, pending, "O3PS mutation must apply");
    assert!(!compatibility_version_contract(
        &runtime,
        &pending_mutated,
        &handoff,
    ));
    let handoff_mutated = handoff.replacen("VERSION_CURRENT: u8 = 7", "VERSION_CURRENT: u8 = 8", 1);
    assert_ne!(handoff_mutated, handoff, "O3DH mutation must apply");
    assert!(!compatibility_version_contract(
        &runtime,
        &pending,
        &handoff_mutated,
    ));
    let relaxed_handoff = handoff.replacen(
        "version == VERSION_CURRENT",
        "version >= VERSION_CURRENT",
        1,
    );
    assert_ne!(relaxed_handoff, handoff, "version relaxation must apply");
    assert!(!compatibility_version_contract(
        &runtime,
        &pending,
        &relaxed_handoff,
    ));
}

fn assert_unconditional_attachment(parent: &str, child: &str, path: &str, module: &str) {
    assert_eq!(
        active_unconditional_path_owned_module_declaration_count(parent, child, path, module),
        1,
        "{module} must have one unconditional path-owned attachment",
    );
}

fn expected_tests_are_unconditional(source: &str, tests: &[&str]) -> bool {
    inner_attribute_lines(source).is_empty()
        && !source.contains("#[cfg")
        && !source.contains("#[cfg_attr")
        && rust_test_attribute_count(source) == tests.len()
        && tests
            .iter()
            .all(|test| top_level_rust_test_function_definition_count(source, test) == 1)
}

fn typed_memory_result_contract(memory: &str, memory_window: &str, window_policy: &str) -> bool {
    let definitions = production_struct_definitions(memory_window)
        .into_iter()
        .filter(|definition| {
            production_defines_exact_named_item(definition, "struct", "O3MemoryResultWindowState")
        })
        .collect::<Vec<_>>();
    if definitions.len() != 1
        || production_named_struct_fields(&definitions[0]) != ["rows", "destinations"]
        || !compact_rust_code(&definitions[0]).contains("destinations:Vec<O3ArchitecturalRegister>")
        || definitions[0].contains("integer_destinations")
    {
        return false;
    }

    let destination = function(root_function(
        memory,
        "o3_memory_result_architectural_destination",
    ));
    let wrapper = function(root_function(window_policy, "from_memory_results"));
    let typed = function(root_function(
        window_policy,
        "from_memory_result_destinations",
    ));
    let classify = function(root_function(window_policy, "classify_compute_younger"));
    destination.contains("o3_memory_result_destination(access)?")
        && destination
            .contains("O3ArchitecturalRegister::from_class_index(register_class,architectural)")
        && compact_rust_code(&production_rust_source(memory_window))
            .contains("letdestination=o3_memory_result_architectural_destination(access)?;")
        && wrapper == EXPECTED_MEMORY_RESULT_WRAPPER
        && typed.contains("Item=O3ArchitecturalRegister")
        && typed.contains("letmutunresolved_destinations=Vec::new();")
        && source_fingerprint(&classify) == EXPECTED_CLASSIFY_COMPUTE_FINGERPRINT
        && classify.contains(".any(|source|self.unresolved_destinations.contains(source))")
}

fn forwarding_contract(queue: &str, compute: &str, forwarding: &str, control: &str) -> bool {
    let Some(definition) = production_enum_definition(forwarding, "O3LiveIssueForwardedValue")
    else {
        return false;
    };
    if compact_rust_code(&definition)
        != "enumO3LiveIssueForwardedValue{Integer(RegisterWrite),FloatingPoint(FloatRegisterWrite),}"
    {
        return false;
    }
    let producers = function(root_function(forwarding, "source_producers"));
    let materialize = function(root_function(forwarding, "materialize_candidate"));
    let completed = function(root_function(control, "completed_live_data_access_source"));
    queue_family_fingerprints(queue, compute, forwarding) == EXPECTED_QUEUE_FAMILY_FINGERPRINTS
        && producers == EXPECTED_SOURCE_PRODUCERS
        && materialize == EXPECTED_MATERIALIZE_CANDIDATE
        && completed == EXPECTED_COMPLETED_LOAD_SOURCE
}

fn queue_family_fingerprints(queue: &str, compute: &str, forwarding: &str) -> [u64; 3] {
    [queue, compute, forwarding].map(production_source_fingerprint)
}

fn production_source_fingerprint(source: &str) -> u64 {
    source_fingerprint(&compact_rust_code(&production_rust_source(source)))
}

fn source_fingerprint(source: &str) -> u64 {
    source.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn compatibility_version_contract(runtime: &str, pending: &str, handoff: &str) -> bool {
    let compact_runtime = compact_rust_code(&production_rust_source(runtime));
    let compact_pending = compact_rust_code(&production_rust_source(pending));
    let compact_handoff = compact_rust_code(&production_rust_source(handoff));
    compact_runtime
        .contains("constO3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS:u8=23;")
        && compact_runtime.contains("constO3_RUNTIME_CHECKPOINT_VERSION:u8=O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS;")
        && pending.contains("const O3_PENDING_STATE_CHECKPOINT_MAGIC: [u8; 4] = *b\"O3PS\";")
        && compact_pending.contains("constO3_PENDING_STATE_CHECKPOINT_VERSION:u8=2;")
        && handoff.contains("pub(super) const MAGIC: [u8; 4] = *b\"O3DH\";")
        && compact_handoff.contains("pub(super)constVERSION_CURRENT:u8=7;")
        && compact_handoff.matches("version==VERSION_CURRENT").count() == 3
}

fn root_function(source: &str, name: &str) -> String {
    rust_function_definition(source, name).unwrap_or_else(|| panic!("missing function {name}"))
}

fn function(source: String) -> String {
    compact_rust_code(&source)
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap()
}
