use super::*;

const MAX_LIVE_COMPUTE_OPERAND_LINES: usize = 320;
const MAX_LIVE_COMPUTE_OPERAND_TEST_LINES: usize = 320;
const MAX_O3_RUNTIME_LIVE_WINDOW_MIXED_COMPUTE_TEST_LINES: usize = 120;

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
