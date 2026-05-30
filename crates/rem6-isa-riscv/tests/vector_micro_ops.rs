use rem6_isa_riscv::{RiscvInstructionFlags, RiscvVectorMicroOpExpansion};

#[test]
fn vector_micro_ops_inherit_macro_execution_flags() {
    let macro_flags =
        RiscvInstructionFlags::SERIALIZE_AFTER.union(RiscvInstructionFlags::NON_SPECULATIVE);

    let expansion = RiscvVectorMicroOpExpansion::new(3)
        .with_macro_flags(macro_flags)
        .expand()
        .unwrap();

    assert_eq!(expansion.len(), 3);
    assert!(expansion.iter().all(|micro_op| micro_op
        .flags()
        .contains(RiscvInstructionFlags::SERIALIZE_AFTER)));
    assert!(expansion.iter().all(|micro_op| micro_op
        .flags()
        .contains(RiscvInstructionFlags::NON_SPECULATIVE)));
}

#[test]
fn vector_micro_ops_merge_macro_and_micro_local_flags() {
    let macro_flags = RiscvInstructionFlags::NON_SPECULATIVE;
    let local_flags = RiscvInstructionFlags::DELAYED_COMMIT;

    let expansion = RiscvVectorMicroOpExpansion::new(2)
        .with_macro_flags(macro_flags)
        .with_micro_op_flags(local_flags)
        .expand()
        .unwrap();

    assert!(expansion
        .iter()
        .all(|micro_op| micro_op.flags().contains(macro_flags)));
    assert!(expansion
        .iter()
        .all(|micro_op| micro_op.flags().contains(local_flags)));
    assert!(expansion[0].is_first());
    assert!(!expansion[0].is_last());
    assert!(!expansion[1].is_first());
    assert!(expansion[1].is_last());
}
