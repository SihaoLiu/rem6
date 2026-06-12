use rem6_isa_riscv::{
    RiscvCounterBank, RiscvCounterCsr, RiscvCounterCsrWord, RiscvCounterSnapshot, RiscvCsrError,
};

#[test]
fn rv32_counter_words_decode_low_and_high_aliases() {
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc00).unwrap(),
        RiscvCounterCsrWord::CycleLow
    );
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc80).unwrap(),
        RiscvCounterCsrWord::CycleHigh
    );
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc01).unwrap(),
        RiscvCounterCsrWord::TimeLow
    );
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc81).unwrap(),
        RiscvCounterCsrWord::TimeHigh
    );
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc02).unwrap(),
        RiscvCounterCsrWord::InstretLow
    );
    assert_eq!(
        RiscvCounterCsrWord::from_user_address(0xc82).unwrap(),
        RiscvCounterCsrWord::InstretHigh
    );
    assert_eq!(
        RiscvCounterCsrWord::from_machine_address(0xb00).unwrap(),
        RiscvCounterCsrWord::CycleLow
    );
    assert_eq!(
        RiscvCounterCsrWord::from_machine_address(0xb80).unwrap(),
        RiscvCounterCsrWord::CycleHigh
    );
    assert_eq!(
        RiscvCounterCsrWord::from_machine_address(0xb02).unwrap(),
        RiscvCounterCsrWord::InstretLow
    );
    assert_eq!(
        RiscvCounterCsrWord::from_machine_address(0xb82).unwrap(),
        RiscvCounterCsrWord::InstretHigh
    );
    assert_eq!(RiscvCounterCsrWord::CycleLow.machine_address(), Some(0xb00));
    assert_eq!(RiscvCounterCsrWord::TimeLow.machine_address(), None);
    assert_eq!(RiscvCounterCsrWord::TimeHigh.machine_address(), None);
}

#[test]
fn rv32_counter_word_reads_and_machine_writes_preserve_other_half() {
    let mut counters = RiscvCounterBank::new();
    counters
        .write_machine(RiscvCounterCsr::Cycle, 0x1234_5678_9abc_def0)
        .unwrap();
    counters
        .write_machine(RiscvCounterCsr::Instret, 0x0102_0304_0506_0708)
        .unwrap();

    assert_eq!(
        counters.read_user_word(RiscvCounterCsrWord::CycleLow),
        0x9abc_def0
    );
    assert_eq!(
        counters.read_user_word(RiscvCounterCsrWord::CycleHigh),
        0x1234_5678
    );
    assert_eq!(
        counters.read_user_word(RiscvCounterCsrWord::TimeLow),
        0x9abc_def0
    );
    assert_eq!(
        counters.read_user_word(RiscvCounterCsrWord::TimeHigh),
        0x1234_5678
    );
    assert_eq!(
        counters.read_machine_word(RiscvCounterCsrWord::InstretLow),
        0x0506_0708
    );
    assert_eq!(
        counters.read_machine_word(RiscvCounterCsrWord::InstretHigh),
        0x0102_0304
    );

    counters
        .write_machine_word(RiscvCounterCsrWord::CycleLow, 0x1111_2222)
        .unwrap();
    counters
        .write_machine_word(RiscvCounterCsrWord::InstretHigh, 0xaabb_ccdd)
        .unwrap();

    assert_eq!(
        counters.snapshot(),
        RiscvCounterSnapshot::new(0x1234_5678_1111_2222, 0xaabb_ccdd_0506_0708)
    );
}

#[test]
fn rv32_counter_word_user_writes_are_read_only_shadow_errors() {
    let mut counters = RiscvCounterBank::new();

    assert_eq!(
        counters
            .write_user_word(RiscvCounterCsrWord::CycleHigh, 0)
            .unwrap_err(),
        RiscvCsrError::ReadOnlyCounterWordAlias {
            csr: RiscvCounterCsrWord::CycleHigh
        }
    );
    assert_eq!(
        counters
            .write_user_word(RiscvCounterCsrWord::TimeHigh, 0)
            .unwrap_err(),
        RiscvCsrError::ReadOnlyCounterWordAlias {
            csr: RiscvCounterCsrWord::TimeHigh
        }
    );
    assert_eq!(
        counters
            .write_machine(RiscvCounterCsr::Time, 0)
            .unwrap_err(),
        RiscvCsrError::ReadOnlyCounterAlias {
            csr: RiscvCounterCsr::Time
        }
    );
    assert_eq!(
        counters
            .write_machine_word(RiscvCounterCsrWord::TimeLow, 0)
            .unwrap_err(),
        RiscvCsrError::ReadOnlyCounterWordAlias {
            csr: RiscvCounterCsrWord::TimeLow
        }
    );
    assert_eq!(
        counters
            .write_machine_word(RiscvCounterCsrWord::TimeHigh, 0)
            .unwrap_err(),
        RiscvCsrError::ReadOnlyCounterWordAlias {
            csr: RiscvCounterCsrWord::TimeHigh
        }
    );
}
