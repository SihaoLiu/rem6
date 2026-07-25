use super::*;

const MAX_LIVE_COMPUTE_OPERAND_LINES: usize = 320;
const MAX_LIVE_COMPUTE_OPERAND_TEST_LINES: usize = 320;
const MAX_O3_RUNTIME_LIVE_WINDOW_MIXED_COMPUTE_TEST_LINES: usize = 120;
const MAX_LIVE_COMPUTE_QUEUE_LINES: usize = 320;
const MAX_O3_RUNTIME_ISSUE_QUEUE_LINES: usize = 600;
const MAX_O3_RUNTIME_ISSUE_QUEUE_MIXED_COMPUTE_TEST_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_CALENDAR_MIXED_COMPUTE_TEST_LINES: usize = 120;
const MAX_O3_RUNTIME_ISSUE_SERVICE_MIXED_COMPUTE_TEST_LINES: usize = 120;

#[test]
fn fp_vector_live_issue_uses_one_focused_operand_authority() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let operand_path = root.join("src/o3_live_compute_operands.rs");
    let test_path = root.join("src/o3_live_compute_operands_tests.rs");
    let runtime_path = root.join("src/o3_runtime.rs");
    assert!(operand_path.exists());
    assert!(test_path.exists());
    assert!(line_count(&operand_path) <= MAX_LIVE_COMPUTE_OPERAND_LINES);
    assert!(line_count(&test_path) <= MAX_LIVE_COMPUTE_OPERAND_TEST_LINES);

    let source = fs::read_to_string(&operand_path).unwrap();
    let tests = fs::read_to_string(&test_path).unwrap();
    let runtime = fs::read_to_string(&runtime_path).unwrap();
    let compact = compact_rust_code(&fs::read_to_string(&operand_path).unwrap());
    let compact_tests = compact_rust_code(&tests);
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

    for positive_pattern in [
        "RiscvInstruction::FloatAddS{",
        "RiscvInstruction::FloatSubS{",
        "RiscvInstruction::FloatMulS{",
        "RiscvInstruction::FloatMultiplyAddS{",
        "RiscvInstruction::FloatMultiplySubtractS{",
        "RiscvInstruction::FloatNegativeMultiplySubtractS{",
        "RiscvInstruction::FloatNegativeMultiplyAddS{",
        "RiscvInstruction::FloatDivS{",
        "RiscvInstruction::FloatSqrtS{",
        "RiscvVectorScalarMoveInstruction::MoveToScalar{",
        "RiscvVectorMaskReductionInstruction::PopCount{",
        "RiscvVectorMaskReductionInstruction::FirstSet{",
    ] {
        assert_eq!(compact.matches(positive_pattern).count(), 1);
    }

    for unsupported_pattern in [
        "RiscvInstruction::FloatAddD{",
        "RiscvInstruction::FloatSubD{",
        "RiscvInstruction::FloatMulD{",
        "RiscvInstruction::FloatDivD{",
        "RiscvInstruction::FloatMultiplyAddD{",
        "RiscvInstruction::FloatMultiplySubtractD{",
        "RiscvInstruction::FloatNegativeMultiplySubtractD{",
        "RiscvInstruction::FloatNegativeMultiplyAddD{",
        "RiscvInstruction::FloatSqrtD{",
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
        "FloatAddD",
        "FloatSubD",
        "FloatMulD",
        "FloatDivD",
        "FloatMultiplyAddD",
        "FloatMultiplySubtractD",
        "FloatNegativeMultiplySubtractD",
        "FloatNegativeMultiplyAddD",
        "FloatSqrtD",
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
    assert!(compact_window.contains("source.register_class()!=O3RegisterClass::Integer"));
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
        "fnlive_issue_queue_rejects_non_integer_live_source_producers(",
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
