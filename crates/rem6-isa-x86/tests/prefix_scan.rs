use rem6_isa_x86::{
    X86DecodeError, X86IgnoredRexReason, X86InstructionMode, X86OpcodeMap, X86PrefixScan,
    X86SegmentOverride,
};

#[test]
fn x86_64_prefix_scan_ignores_rex_interrupted_by_legacy_prefix() {
    let scan =
        X86PrefixScan::scan(X86InstructionMode::Long64, &[0x49, 0x26, 0x83, 0xc0, 0x0a]).unwrap();

    assert_eq!(scan.opcode_start(), 2);
    assert_eq!(scan.opcode_map(), X86OpcodeMap::OneByte);
    assert_eq!(scan.opcode(), 0x83);
    assert_eq!(
        scan.legacy_prefixes().segment(),
        Some(X86SegmentOverride::ES)
    );
    assert_eq!(scan.rex(), None);
    assert_eq!(scan.ignored_rex_prefixes().len(), 1);
    assert_eq!(scan.ignored_rex_prefixes()[0].byte(), 0x49);
    assert_eq!(
        scan.ignored_rex_prefixes()[0].reason(),
        X86IgnoredRexReason::InterruptedByLegacyPrefix
    );
}

#[test]
fn x86_64_prefix_scan_applies_rex_immediately_before_opcode() {
    let scan =
        X86PrefixScan::scan(X86InstructionMode::Long64, &[0x26, 0x49, 0x83, 0xc0, 0x0a]).unwrap();
    let rex = scan.rex().unwrap();

    assert_eq!(scan.opcode_start(), 2);
    assert_eq!(scan.opcode(), 0x83);
    assert_eq!(
        scan.legacy_prefixes().segment(),
        Some(X86SegmentOverride::ES)
    );
    assert_eq!(rex.byte(), 0x49);
    assert!(rex.w());
    assert!(!rex.r());
    assert!(!rex.x());
    assert!(rex.b());
    assert!(scan.ignored_rex_prefixes().is_empty());
}

#[test]
fn x86_64_prefix_scan_applies_rex_before_escape_opcode() {
    let scan =
        X86PrefixScan::scan(X86InstructionMode::Long64, &[0x66, 0x48, 0x0f, 0xaf, 0xc1]).unwrap();

    assert_eq!(scan.opcode_start(), 2);
    assert_eq!(scan.opcode_map(), X86OpcodeMap::TwoByte);
    assert_eq!(scan.opcode(), 0xaf);
    assert!(scan.legacy_prefixes().operand_size_override());
    assert_eq!(scan.rex().unwrap().byte(), 0x48);
}

#[test]
fn x86_64_prefix_scan_uses_last_contiguous_rex_prefix() {
    let scan = X86PrefixScan::scan(X86InstructionMode::Long64, &[0x48, 0x49, 0x89, 0xc0]).unwrap();

    assert_eq!(scan.opcode_start(), 2);
    assert_eq!(scan.opcode(), 0x89);
    assert_eq!(scan.rex().unwrap().byte(), 0x49);
    assert_eq!(scan.ignored_rex_prefixes().len(), 1);
    assert_eq!(scan.ignored_rex_prefixes()[0].byte(), 0x48);
    assert_eq!(
        scan.ignored_rex_prefixes()[0].reason(),
        X86IgnoredRexReason::SupersededByLaterRex
    );
}

#[test]
fn legacy_mode_treats_rex_byte_as_opcode() {
    let scan =
        X86PrefixScan::scan(X86InstructionMode::Protected32, &[0x49, 0x83, 0xc0, 0x0a]).unwrap();

    assert_eq!(scan.opcode_start(), 0);
    assert_eq!(scan.opcode_map(), X86OpcodeMap::OneByte);
    assert_eq!(scan.opcode(), 0x49);
    assert_eq!(scan.rex(), None);
    assert!(scan.ignored_rex_prefixes().is_empty());
}

#[test]
fn prefix_scan_rejects_empty_instruction_bytes() {
    assert_eq!(
        X86PrefixScan::scan(X86InstructionMode::Long64, &[]).unwrap_err(),
        X86DecodeError::EmptyInstruction
    );
}
