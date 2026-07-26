use super::*;

#[test]
fn double_precision_live_compute_operands_match_single_precision_inventory() {
    let cases = [
        (fp2!(FloatAddD, 1, 2, 3), freg(1), vec![freg(2), freg(3)]),
        (fp2!(FloatSubD, 4, 5, 6), freg(4), vec![freg(5), freg(6)]),
        (fp2!(FloatMulD, 7, 8, 9), freg(7), vec![freg(8), freg(9)]),
        (
            fp2!(FloatDivD, 10, 11, 12),
            freg(10),
            vec![freg(11), freg(12)],
        ),
        (
            fp3!(FloatMultiplyAddD, 13, 14, 15, 16),
            freg(13),
            vec![freg(14), freg(15), freg(16)],
        ),
        (
            fp3!(FloatMultiplySubtractD, 17, 18, 19, 20),
            freg(17),
            vec![freg(18), freg(19), freg(20)],
        ),
        (
            fp3!(FloatNegativeMultiplySubtractD, 21, 22, 23, 24),
            freg(21),
            vec![freg(22), freg(23), freg(24)],
        ),
        (
            fp3!(FloatNegativeMultiplyAddD, 25, 26, 27, 28),
            freg(25),
            vec![freg(26), freg(27), freg(28)],
        ),
        (
            RiscvInstruction::FloatSqrtD {
                rd: f(29),
                rs1: f(30),
                rounding_mode: rm(),
            },
            freg(29),
            vec![freg(30)],
        ),
    ];

    for (instruction, destination, sources) in cases {
        expect_operands(
            instruction,
            O3LiveComputeClass::ScalarFloat,
            destination,
            &sources,
        );
    }
}

#[test]
fn double_precision_live_compute_operands_deduplicate_sources_without_reordering() {
    let cases = [
        (fp2!(FloatAddD, 1, 2, 2), freg(1), vec![freg(2)]),
        (
            fp3!(FloatMultiplyAddD, 3, 4, 4, 5),
            freg(3),
            vec![freg(4), freg(5)],
        ),
        (
            fp3!(FloatMultiplySubtractD, 6, 7, 8, 8),
            freg(6),
            vec![freg(7), freg(8)],
        ),
        (
            fp3!(FloatNegativeMultiplyAddD, 9, 10, 11, 10),
            freg(9),
            vec![freg(10), freg(11)],
        ),
    ];

    for (instruction, destination, sources) in cases {
        expect_operands(
            instruction,
            O3LiveComputeClass::ScalarFloat,
            destination,
            &sources,
        );
    }
}
