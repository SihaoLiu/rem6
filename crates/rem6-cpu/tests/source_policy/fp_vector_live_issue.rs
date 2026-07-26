use super::*;

const MAX_LIVE_COMPUTE_OPERAND_LINES: usize = 320;
const MAX_LIVE_COMPUTE_OPERAND_TEST_LINES: usize = 320;
const MAX_LIVE_COMPUTE_DOUBLE_PRECISION_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_LIVE_WINDOW_MIXED_COMPUTE_TEST_LINES: usize = 120;
const MAX_LIVE_COMPUTE_QUEUE_LINES: usize = 320;
const MAX_O3_RUNTIME_ISSUE_QUEUE_LINES: usize = 600;
const MAX_O3_RUNTIME_ISSUE_QUEUE_MIXED_COMPUTE_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_CALENDAR_MIXED_COMPUTE_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_ISSUE_SERVICE_MIXED_COMPUTE_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_ISSUE_FORWARDING_LINES: usize = 260;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TEST_LINES: usize = 360;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_SERVICE_TEST_LINES: usize = 360;
const MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TRANSACTION_TEST_LINES: usize = 240;
const MAX_O3_RUNTIME_TYPED_FORWARDING_CONTROL_TEST_LINES: usize = 220;
const QUEUE_TYPED_FORWARDING_TESTS: &[&str] = &[
    "typed_live_forwarding_discovers_fp_source_producer",
    "typed_live_forwarding_materializes_exact_fp_write_and_ready_tick",
    "typed_live_forwarding_rejects_wrong_class_write",
    "typed_live_forwarding_selects_nearest_fp_waw_producer",
    "typed_live_forwarding_keeps_two_fp_fanin_producers",
    "typed_live_forwarding_filters_integer_x0_without_filtering_fp_f0",
];
const SERVICE_TYPED_FORWARDING_TESTS: &[&str] = &[
    "typed_live_forwarding_waits_for_fp_writeback_and_computes_nine",
    "typed_live_forwarding_uses_dynamic_frm_without_mutating_canonical_status",
    "typed_live_forwarding_vector_result_feeds_integer",
];
const TRANSACTION_TYPED_FORWARDING_TESTS: &[&str] =
    &["typed_live_forwarding_transaction_failure_rolls_back_exact_state"];
const CONTROL_TYPED_FORWARDING_TESTS: &[&str] =
    &["typed_live_forwarding_recursive_invalidation_is_sequence_owned"];
const MIGRATION_LEDGER: &str = "../../docs/architecture/gem5-to-rem6-migration.md";

#[test]
fn fp_vector_live_issue_uses_one_focused_operand_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let operand_path = root.join("src/o3_live_compute_operands.rs");
    let test_path = root.join("src/o3_live_compute_operands_tests.rs");
    let double_precision_relative = "o3_live_compute_operands_tests/double_precision.rs";
    let double_precision_test_path = root.join("src").join(double_precision_relative);
    let runtime_path = root.join("src/o3_runtime.rs");
    assert!(operand_path.exists());
    assert!(test_path.exists());
    assert!(double_precision_test_path.exists());
    assert!(line_count(&operand_path) <= MAX_LIVE_COMPUTE_OPERAND_LINES);
    assert!(line_count(&test_path) <= MAX_LIVE_COMPUTE_OPERAND_TEST_LINES);
    assert!(
        line_count(&double_precision_test_path) <= MAX_LIVE_COMPUTE_DOUBLE_PRECISION_TEST_LINES
    );

    let source = fs::read_to_string(&operand_path).unwrap();
    let tests = fs::read_to_string(&test_path).unwrap();
    let double_precision_tests = fs::read_to_string(&double_precision_test_path).unwrap();
    let runtime = fs::read_to_string(&runtime_path).unwrap();
    let compact = compact_rust_code(&production_rust_source(&source));
    let compact_tests = compact_rust_code(&tests);
    let compact_double_precision_tests = compact_rust_code(
        &rust_code_without_comments_and_literals(&double_precision_tests),
    );
    let compact_runtime = compact_rust_code(&runtime);
    assert_eq!(
        compact
            .matches("pub(crate)fno3_live_compute_operands(")
            .count(),
        1
    );
    assert!(source.contains("o3_predicted_scalar_descendant_operands"));
    assert!(runtime
        .contains("#[path = \"o3_live_compute_operands.rs\"]\nmod o3_live_compute_operands;"));
    assert!(compact_runtime.contains("pub(crate)useo3_live_compute_operands::{o3_live_compute_operands,O3ArchitecturalRegister,O3LiveComputeClass,O3LiveComputeOperands,};"));
    assert!(!compact_runtime.contains("pub(crate)modo3_live_compute_operands;"));
    assert!(!runtime.contains("#[allow(unused_imports)]"));
    assert!(!compact.contains("const_:fn("));

    let double_precision_module = "double_precision";
    assert_eq!(
        active_unconditional_path_owned_module_declaration_count(
            &tests,
            &double_precision_tests,
            double_precision_relative,
            double_precision_module,
        ),
        1,
    );
    let attachment =
        format!("#[path = \"{double_precision_relative}\"]\nmod {double_precision_module};");
    for mutated_tests in [
        tests.replacen(&attachment, &format!("#[cfg(any())]\n{attachment}"), 1),
        tests.replacen(&attachment, "", 1),
    ] {
        assert_ne!(mutated_tests, tests, "attachment mutation must apply");
        assert_eq!(
            active_unconditional_path_owned_module_declaration_count(
                &mutated_tests,
                &double_precision_tests,
                double_precision_relative,
                double_precision_module,
            ),
            0,
        );
    }

    assert!(focused_test_definitions_are_unconditional(
        &double_precision_tests,
        &[
            "double_precision_live_compute_operands_match_single_precision_inventory",
            "double_precision_live_compute_operands_deduplicate_sources_without_reordering",
        ],
    ));
    for form in [
        "FloatAddD",
        "FloatSubD",
        "FloatMulD",
        "FloatDivD",
        "FloatMultiplyAddD",
        "FloatMultiplySubtractD",
        "FloatNegativeMultiplySubtractD",
        "FloatNegativeMultiplyAddD",
        "FloatSqrtD",
    ] {
        assert!(
            compact_double_precision_tests.contains(form),
            "double-precision child missing {form}",
        );
    }

    for required in [
        "O3LiveComputeClass",
        "ScalarInteger",
        "ScalarFloat",
        "VectorToScalar",
        "O3ArchitecturalRegister",
        "O3LiveComputeOperands",
        "FloatAddS",
        "FloatSubS",
        "FloatMulS",
        "FloatMultiplyAddS",
        "FloatMultiplySubtractS",
        "FloatNegativeMultiplySubtractS",
        "FloatNegativeMultiplyAddS",
        "FloatDivS",
        "FloatSqrtS",
        "VectorScalarMove",
        "MoveToScalar",
        "VectorMaskReduction",
        "PopCount",
        "FirstSet",
        "Unmasked",
    ] {
        assert!(source.contains(required), "source missing {required}");
        assert!(tests.contains(required), "tests missing {required}");
    }

    for (single_precision, double_precision) in [
        (
            "RiscvInstruction::FloatAddS{",
            "RiscvInstruction::FloatAddD{",
        ),
        (
            "RiscvInstruction::FloatSubS{",
            "RiscvInstruction::FloatSubD{",
        ),
        (
            "RiscvInstruction::FloatMulS{",
            "RiscvInstruction::FloatMulD{",
        ),
        (
            "RiscvInstruction::FloatDivS{",
            "RiscvInstruction::FloatDivD{",
        ),
        (
            "RiscvInstruction::FloatMultiplyAddS{",
            "RiscvInstruction::FloatMultiplyAddD{",
        ),
        (
            "RiscvInstruction::FloatMultiplySubtractS{",
            "RiscvInstruction::FloatMultiplySubtractD{",
        ),
        (
            "RiscvInstruction::FloatNegativeMultiplySubtractS{",
            "RiscvInstruction::FloatNegativeMultiplySubtractD{",
        ),
        (
            "RiscvInstruction::FloatNegativeMultiplyAddS{",
            "RiscvInstruction::FloatNegativeMultiplyAddD{",
        ),
        (
            "RiscvInstruction::FloatSqrtS{",
            "RiscvInstruction::FloatSqrtD{",
        ),
    ] {
        assert_eq!(
            (
                compact.matches(single_precision).count(),
                compact.matches(double_precision).count(),
            ),
            (1, 1),
            "live arithmetic inventory changed for {single_precision} and {double_precision}",
        );
    }

    for positive_pattern in [
        "RiscvVectorScalarMoveInstruction::MoveToScalar{",
        "RiscvVectorMaskReductionInstruction::PopCount{",
        "RiscvVectorMaskReductionInstruction::FirstSet{",
    ] {
        assert_eq!(compact.matches(positive_pattern).count(), 1);
    }

    for unsupported_pattern in [
        "RiscvInstruction::FloatLessOrEqualS{",
        "RiscvInstruction::FloatLessThanS{",
        "RiscvInstruction::FloatEqualS{",
        "RiscvInstruction::FloatConvert",
        "RiscvInstruction::FloatMove",
        "RiscvInstruction::FloatSignInject",
        "RiscvInstruction::FloatMin",
        "RiscvInstruction::FloatMax",
        "RiscvInstruction::FloatClass",
        "RiscvVectorMaskMode::Masked",
        "RiscvInstruction::VectorAddVv{",
        "RiscvInstruction::VectorFloat(",
        "RiscvVectorScalarMoveInstruction::MoveFromScalar{",
    ] {
        assert!(
            !compact.contains(unsupported_pattern),
            "source admitted unsupported pattern {unsupported_pattern}"
        );
    }

    for negative_pattern in [
        "FloatLessOrEqualS",
        "FloatEqualD",
        "FloatConvertSFromW",
        "FloatConvertWFromS",
        "FloatConvertSFromD",
        "FloatMoveXFromS",
        "FloatMoveDFromX",
        "FloatSignInjectNegD",
        "FloatMinS",
        "FloatMaxD",
        "FloatClassD",
        "VectorAddVv",
        "MoveFromScalar",
        "Ebreak",
    ] {
        assert!(
            tests.contains(negative_pattern),
            "tests missing negative {negative_pattern}"
        );
    }
    for masked_reduction in [
        "RiscvVectorMaskReductionInstruction::PopCount{rd:r(1),vs2:v(2),mask:RiscvVectorMaskMode::Masked",
        "RiscvVectorMaskReductionInstruction::FirstSet{rd:r(1),vs2:v(2),mask:RiscvVectorMaskMode::Masked",
    ] {
        assert!(compact_tests.contains(masked_reduction));
    }
}

#[test]
fn fp_vector_live_issue_locks_task2_window_and_staging_ownership() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window_source = fs::read_to_string(root.join("src/riscv_o3_window_policy.rs")).unwrap();
    let staging_source = fs::read_to_string(root.join("src/o3_runtime_live_window.rs")).unwrap();
    let live_window_tests_path = root.join("src/o3_runtime_live_window_tests.rs");
    let mixed_compute_tests_path = root.join("src/o3_runtime_live_window_tests/mixed_compute.rs");
    let live_window_tests = fs::read_to_string(&live_window_tests_path).unwrap();
    assert!(mixed_compute_tests_path.exists());
    assert!(
        line_count(&mixed_compute_tests_path)
            <= MAX_O3_RUNTIME_LIVE_WINDOW_MIXED_COMPUTE_TEST_LINES
    );
    let mixed_compute_tests = fs::read_to_string(&mixed_compute_tests_path).unwrap();
    let compact_window = compact_rust_code(&window_source);
    let compact_staging = compact_rust_code(&staging_source);
    let compact_mixed_compute_tests = compact_rust_code(&mixed_compute_tests);

    assert!(window_source.contains("unresolved_destinations: Vec<O3ArchitecturalRegister>"));
    assert!(window_source.contains("live_destinations: Vec<O3ArchitecturalRegister>"));
    assert!(window_source.contains("fn classify_compute_younger("));
    assert!(!window_source.contains("fn classify_scalar_younger("));
    assert!(window_source.contains("o3_live_compute_operands(instruction)"));
    assert!(window_source.contains("O3ArchitecturalRegister::integer"));
    assert!(compact_window.contains("source.register_class()==O3RegisterClass::Vector"));
    assert!(compact_window.contains("self.live_destinations.contains(source)"));

    assert!(staging_source.contains("o3_live_compute_operands(instruction)"));
    assert!(compact_staging.contains(".map(|operands|operands.destination())"));
    assert!(compact_staging.contains(".or_else(||{"));
    assert!(staging_source.contains("o3_scalar_integer_destination(instruction)"));
    assert!(staging_source.contains(".map(O3ArchitecturalRegister::integer)"));
    assert!(compact_staging
        .contains(".map(|destination|(destination.register_class(),destination.architectural()))"));
    assert_eq!(
        path_owned_module_declaration_count(
            &live_window_tests,
            "o3_runtime_live_window_tests/mixed_compute.rs",
            "mixed_compute",
        ),
        1
    );
    assert!(compact_mixed_compute_tests
        .contains("fnstage_live_instruction_tracks_mixed_compute_destinations("));
    assert!(compact_mixed_compute_tests.contains("addi(0,1)"));
    assert!(compact_mixed_compute_tests.contains("O3RegisterClass::FloatingPoint,4"));
    assert!(compact_mixed_compute_tests.contains("O3RegisterClass::Integer,11"));
    assert!(!mixed_compute_tests.contains("rustfmt::skip"));
}

#[test]
fn fp_vector_live_issue_locks_task3_o3ps_vector_codec() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let pipeline_source = fs::read_to_string(root.join("src/o3_pipeline.rs")).unwrap();
    let pipeline_tests = fs::read_to_string(root.join("tests/o3_pipeline.rs")).unwrap();
    let compact_source = compact_rust_code(&production_rust_source(&pipeline_source));
    let compact_tests = compact_rust_code(&pipeline_tests);

    assert!(compact_source.contains("constO3_PENDING_STATE_CHECKPOINT_VERSION:u8=2;"));
    assert!(compact_source.contains("constO3_PENDING_STATE_LEGACY_CHECKPOINT_VERSION:u8=1;"));
    assert!(compact_source
        .contains("pubenumO3IssueOpClass{IntAlu,IntMult,Float,Memory,Branch,System,Vector,}"));

    for mapping in [
        "O3IssueOpClass::IntAlu=>0",
        "O3IssueOpClass::IntMult=>1",
        "O3IssueOpClass::Float=>2",
        "O3IssueOpClass::Memory=>3",
        "O3IssueOpClass::Branch=>4",
        "O3IssueOpClass::System=>5",
        "O3IssueOpClass::Vector=>6",
    ] {
        assert!(
            compact_source.contains(mapping),
            "codec mapping changed: {mapping}"
        );
    }

    assert!(compact_source.contains(
        "fndecode_checkpoint_op_class(version:u8,code:u8)->Result<O3IssueOpClass,O3PipelineError>"
    ));
    assert!(compact_source.contains(
        "ifversion!=O3_PENDING_STATE_CHECKPOINT_VERSION&&version!=O3_PENDING_STATE_LEGACY_CHECKPOINT_VERSION"
    ));
    assert!(compact_source
        .contains("6ifversion==O3_PENDING_STATE_CHECKPOINT_VERSION=>Ok(O3IssueOpClass::Vector)"));
    assert!(compact_source.contains("_=>Err(O3PipelineError::InvalidCheckpointOpClassCode{code})"));

    for test_anchor in [
        "fno3_pending_state_checkpoint_payload_round_trips_issue_dependencies_and_writeback(",
        "assert_eq!(encoded[O3_PENDING_CHECKPOINT_VERSION_OFFSET],2);",
        "O3ScopedReadyInstruction::new(23,queue,O3IssueOpClass::Vector)",
        "fno3_pending_state_checkpoint_payload_decodes_legacy_v1_class_codes(",
        "payload[O3_PENDING_CHECKPOINT_VERSION_OFFSET]=1;",
        "fno3_pending_state_checkpoint_payload_rejects_vector_code_in_legacy_v1(",
        "O3ScopedReadyInstruction::new(legacy_sequence,queue,O3IssueOpClass::IntAlu,)",
        "assert_eq!(payload[vector_op_class_offset],0);",
        "payload[vector_op_class_offset]=6;",
        "O3PipelineError::InvalidCheckpointOpClassCode{code:6}",
        "unsupported_version[O3_PENDING_CHECKPOINT_VERSION_OFFSET]=3;",
        "fnpending_ready_op_class_offset(",
    ] {
        assert!(
            compact_tests.contains(test_anchor),
            "missing O3PS boundary test anchor {test_anchor}"
        );
    }
}

#[test]
fn fp_vector_live_issue_locks_task4_queue_compute_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let queue_path = root.join("src/o3_runtime_issue/queue.rs");
    let compute_path = root.join("src/o3_runtime_issue/queue/compute.rs");
    let queue_tests_path = root.join("src/o3_runtime_issue/queue_tests.rs");
    let mixed_tests_path = root.join("src/o3_runtime_issue/queue_tests/mixed_compute.rs");
    let issue_path = root.join("src/o3_runtime_issue.rs");
    let state_path = root.join("src/o3_runtime_issue/state.rs");
    let state_tests_path = root.join("src/o3_runtime_issue/state_tests.rs");

    assert!(compute_path.exists());
    assert!(mixed_tests_path.exists());
    assert!(line_count(&compute_path) <= MAX_LIVE_COMPUTE_QUEUE_LINES);
    assert!(line_count(&queue_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_LINES);
    assert!(line_count(&mixed_tests_path) <= MAX_O3_RUNTIME_ISSUE_QUEUE_MIXED_COMPUTE_TEST_LINES);

    let queue = fs::read_to_string(&queue_path).unwrap();
    let compute = fs::read_to_string(&compute_path).unwrap();
    let queue_tests = fs::read_to_string(&queue_tests_path).unwrap();
    let mixed_tests = fs::read_to_string(&mixed_tests_path).unwrap();
    let issue = fs::read_to_string(&issue_path).unwrap();
    let state = fs::read_to_string(&state_path).unwrap();
    let state_tests = fs::read_to_string(&state_tests_path).unwrap();
    let production_issue = production_rust_source(&issue);
    let compact_queue = compact_rust_code(&production_rust_source(&queue));
    let compact_compute = compact_rust_code(&production_rust_source(&compute));
    let compact_head_recording = compact_rust_code(
        &rust_function_definition(&production_issue, "record_live_issue_head_execution").unwrap(),
    );
    let compact_head_validation = compact_rust_code(
        &rust_function_definition(&production_issue, "live_issue_head_execution_is_valid").unwrap(),
    );
    let compact_state = compact_rust_code(&production_rust_source(&state));
    let compact_state_tests = compact_rust_code(&state_tests);

    assert_eq!(
        path_owned_module_declaration_count(&queue, "queue/compute.rs", "compute"),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &queue_tests,
            "queue_tests/mixed_compute.rs",
            "mixed_compute"
        ),
        1
    );
    assert!(!compact_queue.contains("Scalar(O3RenameMapEntry)"));
    assert!(compact_queue.contains("Compute(O3RenameMapEntry)"));
    assert!(compact_compute.contains("o3_live_compute_operands(instruction)"));
    assert!(compact_compute.contains(
        "typed_destination_matches_rename_entry(operands.destination(),staged_rename_entry)"
    ));
    assert!(!compact_compute.contains("metadata.destination()==staged_rename_entry"));
    assert!(compact_compute.contains("O3LiveComputeClass::ScalarInteger"));
    assert!(compact_compute.contains("O3LiveComputeClass::ScalarFloat=>O3IssueOpClass::Float"));
    assert!(compact_compute.contains("O3LiveComputeClass::VectorToScalar=>O3IssueOpClass::Vector"));
    assert!(compact_queue.contains("O3LiveSpeculativeIssueKind::Compute"));

    for trace_anchor in ["Self::ScalarFloat=>", "Self::VectorToScalar=>"] {
        assert!(
            compact_state.contains(trace_anchor),
            "missing trace variant {trace_anchor}"
        );
    }
    assert!(state.contains("Self::ScalarFloat => \"scalar_float\""));
    assert!(state.contains("Self::VectorToScalar => \"vector_to_scalar\""));
    assert!(compact_state.contains("scalar_float_issued_rows:u64"));
    assert!(compact_state.contains("vector_to_scalar_issued_rows:u64"));
    assert!(compact_state.contains("scalar_float_issued_rows->u64"));
    assert!(compact_state.contains("vector_to_scalar_issued_rows->u64"));
    assert!(compact_state_tests.contains("O3LiveIssueTraceClass::ScalarFloat"));
    assert!(compact_state_tests.contains("O3LiveIssueTraceClass::VectorToScalar"));

    assert!(compact_compute.contains("fnexecution_exactly_writes_compute_destination("));
    assert!(compact_compute.contains("O3RegisterClass::Integer=>{execution.register_writes().len()==1&&execution.float_register_writes().is_empty()&&execution_writes_rename_destination(execution,destination)}"));
    assert!(compact_compute.contains("O3RegisterClass::FloatingPoint=>{execution.register_writes().is_empty()&&execution.float_register_writes().len()==1&&execution_writes_rename_destination(execution,destination)}"));
    assert!(compact_compute.contains(
        "O3RegisterClass::Vector|O3RegisterClass::ConditionCode|O3RegisterClass::Misc=>false"
    ));
    assert!(compact_compute.contains("execution.next_pc()==execution.pc().wrapping_add(u64::from(execution.instruction_bytes()))"));
    assert!(
        compact_head_recording.contains("!live_issue_head_execution_is_valid(entry,&execution)")
    );
    assert!(compact_head_validation.contains("queue::valid_recorded_compute_execution(execution,entry.pc(),execution.instruction(),destination,)"));
    assert!(!compact_head_recording.contains("!execution.float_register_writes().is_empty()"));

    for forbidden in [
        "forwarded_float_register_writes",
        "forwarded_vector_register_writes",
        "FloatRegisterWrite",
        "VectorRegisterWrite",
        "RiscvInstruction::VectorFloat(",
        "RiscvInstruction::VectorAddVv{",
        "RiscvVectorScalarMoveInstruction::MoveFromScalar",
    ] {
        assert!(
            !compact_compute.contains(forbidden),
            "compute queue authority admitted forbidden surface {forbidden}"
        );
    }

    for test_anchor in [
        "fnlive_issue_queue_materializes_mixed_compute_classes(",
        "fnlive_issue_queue_admits_scalar_fp_live_source_producers(",
        "fnlive_issue_queue_uses_typed_source_identities(",
        "fnlive_issue_queue_preserves_integer_producer_forwarding(",
        "fnlive_issue_candidate_result_validation_is_destination_class_exact(",
        "fnlive_issue_head_result_validation_matches_candidate_validation(",
    ] {
        assert!(
            compact_rust_code(&mixed_tests).contains(test_anchor),
            "mixed queue tests missing {test_anchor}"
        );
    }
}

#[test]
fn fp_vector_live_issue_locks_task5_calendar_and_service_proof() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let calendar_path = root.join("src/o3_runtime_issue/calendar.rs");
    let calendar_tests_path = root.join("src/o3_runtime_issue/calendar_tests.rs");
    let mixed_tests_path = root.join("src/o3_runtime_issue/calendar_tests/mixed_compute.rs");
    let service_tests_path = root.join("src/o3_runtime_issue/service_tests.rs");
    let mixed_service_tests_path = root.join("src/o3_runtime_issue/service_tests/mixed_compute.rs");
    assert!(mixed_tests_path.exists());
    assert!(mixed_service_tests_path.exists());
    assert!(
        line_count(&mixed_tests_path) <= MAX_O3_RUNTIME_ISSUE_CALENDAR_MIXED_COMPUTE_TEST_LINES
    );
    assert!(
        line_count(&mixed_service_tests_path)
            <= MAX_O3_RUNTIME_ISSUE_SERVICE_MIXED_COMPUTE_TEST_LINES
    );

    let calendar = fs::read_to_string(&calendar_path).unwrap();
    let calendar_tests = fs::read_to_string(&calendar_tests_path).unwrap();
    let mixed_tests = fs::read_to_string(&mixed_tests_path).unwrap();
    let service_tests = fs::read_to_string(&service_tests_path).unwrap();
    let mixed_service_tests = fs::read_to_string(&mixed_service_tests_path).unwrap();
    let production_calendar = production_rust_source(&calendar);
    let compact_calendar = compact_rust_code(&production_calendar);
    let compact_capacities = compact_rust_code(
        &rust_function_definition(
            &production_calendar,
            "live_issue_capacities_after_reservations",
        )
        .unwrap(),
    );
    let compact_mixed_tests = compact_rust_code(&mixed_tests);
    let compact_service_proof = compact_rust_code(
        &rust_function_definition(
            &mixed_service_tests,
            "mixed_compute_service_coissues_fp_vector_and_retries_second_fp",
        )
        .unwrap(),
    );

    assert_eq!(
        path_owned_module_declaration_count(
            &calendar_tests,
            "calendar_tests/mixed_compute.rs",
            "mixed_compute",
        ),
        1
    );
    assert_eq!(
        path_owned_module_declaration_count(
            &service_tests,
            "service_tests/mixed_compute.rs",
            "mixed_compute",
        ),
        1
    );
    assert!(compact_calendar.contains("float:usize"));
    assert!(compact_calendar.contains("vector:usize"));
    assert!(
        compact_calendar.contains("O3IssueOpClass::Float=>self.float=self.float.saturating_add(1)")
    );
    assert!(compact_calendar
        .contains("O3IssueOpClass::Vector=>self.vector=self.vector.saturating_add(1)"));
    assert!(compact_calendar.contains("O3IssueOpClass::System=>{}"));
    assert!(compact_capacities
        .contains("O3IssueOpClass::Float,1_usize.saturating_sub(reservations.float)"));
    assert!(compact_capacities
        .contains("O3IssueOpClass::Vector,1_usize.saturating_sub(reservations.vector)"));
    assert!(!compact_capacities.contains("O3IssueOpClass::System"));

    for anchor in [
        "fnlive_issue_calendar_width_two_coissues_float_and_vector(",
        "fnlive_issue_calendar_serializes_two_float_rows(",
        "fnlive_issue_calendar_rebuild_reserves_float_without_blocking_vector(",
    ] {
        assert!(compact_mixed_tests.contains(anchor));
    }
    assert!(!mixed_tests.contains("rustfmt::skip"));
    assert!(compact_service_proof.contains("runtime.set_issue_width(2)"));
    assert!(compact_service_proof.contains("assert_eq!(first.issued_rows(),2)"));
    assert!(compact_service_proof.contains("(*first_fp,O3LiveIssueTraceAction::Selected)"));
    assert!(compact_service_proof.contains("(*vector,O3LiveIssueTraceAction::Selected)"));
    assert!(compact_service_proof.contains("(*second_fp,O3LiveIssueTraceAction::RetainedResource)"));
    assert!(compact_service_proof.contains("assert_eq!(first.next_service_tick(),Some(21))"));
    assert!(
        compact_service_proof.contains("assert_eq!(runtime.live_issue_service_tick(),Some(21))")
    );
    assert!(compact_service_proof.contains("assert_eq!(second.issued_rows(),1)"));
}

#[test]
fn fp_vector_live_issue_locks_task8_typed_forwarding_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forwarding_path = root.join("src/o3_runtime_issue/queue/forwarding.rs");
    let queue_tests_path = root.join("src/o3_runtime_issue/queue_tests/typed_forwarding.rs");
    let service_tests_path = root.join("src/o3_runtime_issue/service_tests/typed_forwarding.rs");
    let transaction_tests_path =
        root.join("src/o3_runtime_issue/transaction_tests/typed_forwarding.rs");
    let control_tests_path = root.join("src/o3_runtime_control_window_tests/typed_forwarding.rs");

    for path in [
        &forwarding_path,
        &queue_tests_path,
        &service_tests_path,
        &transaction_tests_path,
        &control_tests_path,
    ] {
        assert!(
            path.is_file(),
            "missing focused typed forwarding owner {}",
            path.display()
        );
    }
    assert!(line_count(&forwarding_path) <= MAX_O3_RUNTIME_ISSUE_FORWARDING_LINES);
    assert!(line_count(&queue_tests_path) <= MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TEST_LINES);
    assert!(
        line_count(&service_tests_path) <= MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_SERVICE_TEST_LINES
    );
    assert!(
        line_count(&transaction_tests_path)
            <= MAX_O3_RUNTIME_ISSUE_TYPED_FORWARDING_TRANSACTION_TEST_LINES
    );
    assert!(line_count(&control_tests_path) <= MAX_O3_RUNTIME_TYPED_FORWARDING_CONTROL_TEST_LINES);

    for (path, tests) in [
        (&queue_tests_path, QUEUE_TYPED_FORWARDING_TESTS),
        (&service_tests_path, SERVICE_TYPED_FORWARDING_TESTS),
        (&transaction_tests_path, TRANSACTION_TYPED_FORWARDING_TESTS),
        (&control_tests_path, CONTROL_TYPED_FORWARDING_TESTS),
    ] {
        let source = fs::read_to_string(path).unwrap();
        assert!(focused_test_definitions_are_unconditional(&source, tests));
        for attribute in [
            "#[cfg(any())]\n",
            "#[cfg_attr(all(), cfg(any()))]\n",
            "#[ignore]\n",
        ] {
            let gated = source.replacen("#[test]", &format!("#[test]\n{attribute}"), 1);
            assert_ne!(gated, source, "test attribute mutation must apply");
            assert!(!focused_test_definitions_are_unconditional(&gated, tests));
        }
    }

    for (owner, relative, module) in [
        (
            "src/o3_runtime_issue/queue.rs",
            "queue/forwarding.rs",
            "forwarding",
        ),
        (
            "src/o3_runtime_issue/queue_tests.rs",
            "queue_tests/typed_forwarding.rs",
            "typed_forwarding",
        ),
        (
            "src/o3_runtime_issue/service_tests.rs",
            "service_tests/typed_forwarding.rs",
            "typed_forwarding",
        ),
        (
            "src/o3_runtime_issue/transaction_tests.rs",
            "transaction_tests/typed_forwarding.rs",
            "typed_forwarding",
        ),
        (
            "src/o3_runtime_control_window_tests.rs",
            "o3_runtime_control_window_tests/typed_forwarding.rs",
            "typed_forwarding",
        ),
    ] {
        let owner_path = root.join(owner);
        let source = fs::read_to_string(&owner_path).unwrap();
        let child = fs::read_to_string(owner_path.parent().unwrap().join(relative)).unwrap();
        assert_eq!(
            active_unconditional_path_owned_module_declaration_count(
                &source, &child, relative, module,
            ),
            1,
            "{owner} must attach {relative} exactly once",
        );
        let attachment = format!("#[path = \"{relative}\"]\nmod {module};");
        for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
            let gated = source.replacen(&attachment, &format!("{conditional}{attachment}"), 1);
            assert_ne!(gated, source, "conditional attachment mutation must apply");
            assert_eq!(
                active_unconditional_path_owned_module_declaration_count(
                    &gated, &child, relative, module,
                ),
                0,
            );
            let inner = conditional.replacen("#[", "#![", 1);
            assert_eq!(
                active_unconditional_path_owned_module_declaration_count(
                    &format!("{inner}{source}"),
                    &child,
                    relative,
                    module,
                ),
                0,
            );
            let gated_child = format!("{inner}{child}");
            assert_eq!(
                active_unconditional_path_owned_module_declaration_count(
                    &source,
                    &gated_child,
                    relative,
                    module,
                ),
                0,
            );
        }
        let nested = source.replacen(
            &attachment,
            &format!("#[cfg(test)]\nmod tests {{\n{attachment}\n}}"),
            1,
        );
        assert_ne!(nested, source, "nested attachment mutation must apply");
        assert_eq!(
            active_unconditional_path_owned_module_declaration_count(
                &nested, &child, relative, module,
            ),
            0,
        );
    }

    let runtime_source = fs::read_to_string(root.join("src/o3_runtime.rs")).unwrap();
    for (relative, module) in [
        ("o3_runtime_issue_tests.rs", "o3_runtime_issue_tests"),
        (
            "o3_runtime_control_window_tests.rs",
            "o3_runtime_control_window_tests",
        ),
    ] {
        let child = fs::read_to_string(root.join("src").join(relative)).unwrap();
        assert_eq!(
            active_cfg_test_path_owned_module_declaration_count(
                &runtime_source,
                &child,
                relative,
                module,
            ),
            1,
        );
        for conditional in ["cfg(any())", "cfg_attr(all(), cfg(any()))"] {
            let gated = runtime_source.replacen(
                &format!("#[cfg(test)]\n#[path = \"{relative}\"]"),
                &format!("#[{conditional}]\n#[path = \"{relative}\"]"),
                1,
            );
            assert_ne!(gated, runtime_source, "cfg-test edge mutation must apply");
            assert_eq!(
                active_cfg_test_path_owned_module_declaration_count(
                    &gated, &child, relative, module,
                ),
                0,
            );
        }
        let inline =
            runtime_source.replacen(&format!("mod {module};"), &format!("mod {module} {{}}"), 1);
        assert_ne!(inline, runtime_source, "inline edge mutation must apply");
        assert_eq!(
            active_cfg_test_path_owned_module_declaration_count(&inline, &child, relative, module,),
            0,
        );
    }

    let issue_tests_source =
        fs::read_to_string(root.join("src/o3_runtime_issue_tests.rs")).unwrap();
    for (relative, module) in [
        ("o3_runtime_issue/queue_tests.rs", "queue"),
        ("o3_runtime_issue/service_tests.rs", "service"),
        ("o3_runtime_issue/transaction_tests.rs", "transaction"),
    ] {
        let child = fs::read_to_string(root.join("src").join(relative)).unwrap();
        assert_eq!(
            active_unconditional_path_owned_module_declaration_count(
                &issue_tests_source,
                &child,
                relative,
                module,
            ),
            1,
        );
    }

    let forwarding = fs::read_to_string(&forwarding_path).unwrap();
    let compute = fs::read_to_string(root.join("src/o3_runtime_issue/queue/compute.rs")).unwrap();
    let combined = compact_rust_code(&format!(
        "{}\n{}",
        production_rust_source(&forwarding),
        production_rust_source(&compute),
    ));
    for anchor in [
        "enumO3LiveIssueForwardedValue{Integer(RegisterWrite),FloatingPoint(FloatRegisterWrite),}",
        "source.register_class()==O3RegisterClass::Vector",
    ] {
        assert!(
            combined.contains(anchor),
            "missing typed forwarding anchor {anchor}"
        );
    }
    for forbidden in ["VectorRegisterWrite", "O3LiveIssueForwardedValue::Vector"] {
        assert!(
            !combined.contains(forbidden),
            "forbidden typed forwarding surface {forbidden}"
        );
    }

    let service = fs::read_to_string(root.join("src/o3_runtime_issue/service.rs")).unwrap();
    let prepare = compact_rust_code(
        &rust_function_definition(
            &production_rust_source(&service),
            "prepare_live_issue_batch",
        )
        .unwrap(),
    );
    let clone = prepare
        .find("letmutspeculative_hart=hart.clone();")
        .expect("typed forwarding must clone the canonical hart");
    let fp_apply = prepare
        .find("speculative_hart.write_float(write.register(),write.value())")
        .expect("typed forwarding must apply FP values to the clone");
    assert!(clone < fp_apply);
    let without_speculative_apply = prepare.replace(
        "speculative_hart.write_float(write.register(),write.value())",
        "",
    );
    assert!(!without_speculative_apply.contains("hart.write_float("));

    for (relative, anchor) in [
        (
            "src/o3_runtime_checkpoint.rs",
            "constO3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS:u8=23;",
        ),
        (
            "src/o3_pipeline.rs",
            "constO3_PENDING_STATE_CHECKPOINT_VERSION:u8=2;",
        ),
        (
            "src/riscv_execution_mode_handoff/codec.rs",
            "pub(super)constVERSION_CURRENT:u8=7;",
        ),
    ] {
        let source = fs::read_to_string(root.join(relative)).unwrap();
        assert!(
            compact_rust_code(&production_rust_source(&source)).contains(anchor),
            "schema owner changed: {relative}",
        );
    }
}

#[test]
fn fp_vector_live_issue_locks_task6_external_telemetry_schema() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let system_cpu =
        fs::read_to_string(root.join("../rem6-system/src/riscv_o3_runtime_stats/cpu.rs")).unwrap();
    let system_snapshot =
        fs::read_to_string(root.join("../rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs"))
            .unwrap();
    let core_summary = fs::read_to_string(root.join("../rem6/src/core_summary_json.rs")).unwrap();
    let stats_output =
        fs::read_to_string(root.join("../rem6/src/stats_output/o3_runtime_issue.rs")).unwrap();
    let debug_output =
        fs::read_to_string(root.join("../rem6/src/debug_output/o3_issue_queue_json.rs")).unwrap();
    let persistent_iq =
        fs::read_to_string(root.join("../rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs"))
            .unwrap();

    let compact_system_cpu = compact_rust_code(&production_rust_source(&system_cpu));
    let compact_core_summary = compact_rust_code(&production_rust_source(&core_summary));
    let compact_debug_output = compact_rust_code(&production_rust_source(&debug_output));
    let compact_persistent_iq = compact_rust_code(&persistent_iq);
    let compact_raw_system_cpu = compact_rust_code(&system_cpu);
    let compact_raw_stats_output = compact_rust_code(&stats_output);
    let compact_delta =
        compact_rust_code(&rust_function_definition(&system_cpu, "increment_delta").unwrap());
    let compact_snapshot =
        compact_rust_code(&rust_function_definition(&system_snapshot, "set_snapshot").unwrap());

    for (class, getter) in [
        ("scalar_float", "scalar_float_issued_rows"),
        ("vector_to_scalar", "vector_to_scalar_issued_rows"),
    ] {
        let stat_path = format!("issue_queue.issued_by_class.{class}");
        assert!(system_cpu.contains(&stat_path));
        assert!(compact_system_cpu.contains(&format!("issue_queue_{getter}:StatId")));
        assert!(compact_raw_system_cpu.contains(&format!("issue_queue_{getter}:register_o3_counter(registry,&prefix,\"issue_queue.issued_by_class.{class}\",\"Count\",)?")));
        assert!(compact_delta.contains(&format!("(self.issue_queue_{getter},previous_live_issue.{getter}(),current_live_issue.{getter}(),)")));
        assert!(compact_snapshot.contains(&format!(
            "(self.issue_queue_{getter},live_issue.{getter}(),)"
        )));
        assert!(core_summary.contains(&format!("\\\"{class}\\\"")));
        assert!(compact_core_summary.contains(&format!("queue.{getter}()")));
        assert!(stats_output.contains(&format!("issued_by_class.{class}")));
        assert!(compact_raw_stats_output
            .contains(&format!("(\"issued_by_class.{class}\",queue.{getter}(),)")));
        assert!(debug_output.contains(&format!("\\\"{class}\\\"")));
        assert!(compact_debug_output.contains(&format!("{getter}:telemetry.{getter}()")));
        assert!(compact_persistent_iq.contains(&format!("issued_by_class/{class}")));
        assert!(compact_persistent_iq.contains(&format!("(\"issued_by_class/{class}\",0)")));
    }

    for (source, control, scalar_float, vector_to_scalar) in [
        (
            &system_cpu,
            "issue_queue.issued_by_class.control",
            "issue_queue.issued_by_class.scalar_float",
            "issue_queue.issued_by_class.vector_to_scalar",
        ),
        (
            &stats_output,
            "issued_by_class.control",
            "issued_by_class.scalar_float",
            "issued_by_class.vector_to_scalar",
        ),
        (
            &persistent_iq,
            "issued_by_class/control",
            "issued_by_class/scalar_float",
            "issued_by_class/vector_to_scalar",
        ),
    ] {
        let control = source.find(control).unwrap();
        let scalar_float = source.find(scalar_float).unwrap();
        let vector_to_scalar = source.find(vector_to_scalar).unwrap();
        assert!(control < scalar_float && scalar_float < vector_to_scalar);
    }
    let appended_json_classes =
        "\\\"control\\\":{},\\\"scalar_float\\\":{},\\\"vector_to_scalar\\\":{}";
    assert!(core_summary.contains(appended_json_classes));
    assert!(debug_output.contains(appended_json_classes));
    assert!(compact_core_summary.contains("queue.control_issued_rows(),queue.scalar_float_issued_rows(),queue.vector_to_scalar_issued_rows(),"));
    assert!(compact_debug_output.contains("telemetry.control_issued_rows,telemetry.scalar_float_issued_rows,telemetry.vector_to_scalar_issued_rows,"));

    for source in [
        &system_cpu,
        &system_snapshot,
        &core_summary,
        &stats_output,
        &debug_output,
    ] {
        assert!(!source.contains("issued_by_class.system"));
        assert!(!source.contains("system_issued_rows"));
    }
    assert!(compact_persistent_iq.contains("constPERSISTENT_IQ_QUEUE_STATS:[(&str,&str);11]"));
}

#[test]
fn fp_vector_live_issue_locks_ledger_scope() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ledger = fs::read_to_string(root.join(MIGRATION_LEDGER)).unwrap();
    let cpu = ledger
        .split_once("### CPU Execution Models - 74% representative")
        .expect("missing CPU execution-model ledger section")
        .1
        .split("\n### ")
        .next()
        .unwrap();

    for evidence in [
        "scalar FP and vector-to-scalar",
        "issued_by_class.scalar_float",
        "issued_by_class.vector_to_scalar",
        "rem6_run_o3_typed_live_forwarding_width_one_direct",
        "rem6_run_o3_typed_live_forwarding_width_two_direct",
        "rem6_run_o3_typed_live_forwarding_width_four_hierarchy",
        "rem6_run_o3_typed_live_forwarding_checkpoint_boundaries",
        "rem6_run_o3_typed_live_forwarding_handoff_rejects_live_state",
        "rem6_run_o3_typed_live_forwarding_drained_restore",
        "rem6_run_timing_suppresses_o3_typed_live_forwarding",
        "00001041",
        "vmul.vv -> vmv.x.s",
    ] {
        assert!(cpu.contains(evidence), "CPU ledger missing `{evidence}`");
    }
    assert!(cpu.contains(
        "true vector-register producers and destinations, vector LMUL/mask/tail/v0/load/VCSR-aware forwarding, FP loads, double precision, conversions, broader or status-sensitive FP chains, arbitrary unbounded mixed dependency graphs, positive system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, checkpoint-restorable live IQ/transport state, and a general O3 engine remain incomplete"
    ));
    let normalized_ledger = normalized_policy_text(&ledger);
    for broad_claim in ["persistent vector arithmetic iq", "system issue support"] {
        assert!(
            !normalized_ledger.contains(broad_claim),
            "migration ledger overclaims `{broad_claim}`",
        );
    }
}

fn normalized_policy_text(source: &str) -> String {
    source
        .chars()
        .map(|character| {
            character
                .is_ascii_alphanumeric()
                .then(|| character.to_ascii_lowercase())
                .unwrap_or(' ')
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
