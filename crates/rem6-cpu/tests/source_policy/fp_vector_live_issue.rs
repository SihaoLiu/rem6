use super::*;

const MAX_LIVE_COMPUTE_OPERAND_LINES: usize = 320;
const MAX_LIVE_COMPUTE_OPERAND_TEST_LINES: usize = 320;

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
