use rem6_isa_x86::{
    X86ControlRegister4, X86InterruptFlagError, X86InterruptFlagOperation, X86InterruptFlagOutcome,
    X86PrivilegeLevel, X86Rflags,
};

#[test]
fn cli_user_mode_iopl_zero_faults_even_when_carry_and_reserved_bit_are_set() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1 | X86Rflags::CARRY | X86Rflags::IF);

    assert_eq!(rflags.iopl(), 0);
    assert_eq!(
        X86InterruptFlagOperation::Cli.apply_protected(
            X86PrivilegeLevel::ring3(),
            X86ControlRegister4::new(0),
            rflags,
        ),
        Err(X86InterruptFlagError::GeneralProtection { code: 0 })
    );
}

#[test]
fn cli_user_mode_iopl_three_clears_interrupt_flag() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1 | X86Rflags::IF).with_iopl(3);
    let outcome = X86InterruptFlagOperation::Cli
        .apply_protected(
            X86PrivilegeLevel::ring3(),
            X86ControlRegister4::new(0),
            rflags,
        )
        .unwrap();

    assert!(matches!(
        outcome,
        X86InterruptFlagOutcome::InterruptFlagCleared { .. }
    ));
    assert!(!outcome.rflags().interrupt_flag());
    assert_eq!(outcome.rflags().iopl(), 3);
}

#[test]
fn sti_kernel_mode_iopl_zero_sets_interrupt_flag() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1);
    let outcome = X86InterruptFlagOperation::Sti
        .apply_protected(
            X86PrivilegeLevel::ring0(),
            X86ControlRegister4::new(0),
            rflags,
        )
        .unwrap();

    assert!(matches!(
        outcome,
        X86InterruptFlagOutcome::InterruptFlagSet { .. }
    ));
    assert!(outcome.rflags().interrupt_flag());
    assert!(!outcome.rflags().virtual_interrupt_flag());
}

#[test]
fn sti_user_mode_with_pvi_sets_virtual_interrupt_flag_when_vip_is_clear() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1);
    let outcome = X86InterruptFlagOperation::Sti
        .apply_protected(
            X86PrivilegeLevel::ring3(),
            X86ControlRegister4::new(0).with_pvi(true),
            rflags,
        )
        .unwrap();

    assert!(matches!(
        outcome,
        X86InterruptFlagOutcome::VirtualInterruptFlagSet { .. }
    ));
    assert!(!outcome.rflags().interrupt_flag());
    assert!(outcome.rflags().virtual_interrupt_flag());
}

#[test]
fn sti_user_mode_with_pvi_faults_when_vip_is_set() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1).with_virtual_interrupt_pending(true);

    assert_eq!(
        X86InterruptFlagOperation::Sti.apply_protected(
            X86PrivilegeLevel::ring3(),
            X86ControlRegister4::new(0).with_pvi(true),
            rflags,
        ),
        Err(X86InterruptFlagError::GeneralProtection { code: 0 })
    );
}

#[test]
fn cli_user_mode_with_pvi_clears_virtual_interrupt_flag() {
    let rflags = X86Rflags::new(X86Rflags::RESERVED_BIT_1).with_virtual_interrupt_flag(true);
    let outcome = X86InterruptFlagOperation::Cli
        .apply_protected(
            X86PrivilegeLevel::ring3(),
            X86ControlRegister4::new(0).with_pvi(true),
            rflags,
        )
        .unwrap();

    assert!(matches!(
        outcome,
        X86InterruptFlagOutcome::VirtualInterruptFlagCleared { .. }
    ));
    assert!(!outcome.rflags().interrupt_flag());
    assert!(!outcome.rflags().virtual_interrupt_flag());
}
