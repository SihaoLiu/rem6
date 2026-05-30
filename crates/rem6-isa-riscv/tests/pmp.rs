use rem6_isa_riscv::{
    RiscvPmpAccessKind, RiscvPmpAddressMode, RiscvPmpConfig, RiscvPmpError, RiscvPmpRange,
    RiscvPmpTable, RiscvPrivilegeMode,
};

#[test]
fn pmp_decodes_ranges_and_uses_lowest_matching_entry() {
    let mut pmp = RiscvPmpTable::new(4).unwrap();

    pmp.write_addr(0, 0x1000 >> 2).unwrap();
    pmp.write_config(
        0,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Tor).with_read(true),
    )
    .unwrap();
    pmp.write_addr(1, 0x2ff).unwrap();
    pmp.write_config(
        1,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Napot)
            .with_read(true)
            .with_write(true),
    )
    .unwrap();

    assert_eq!(pmp.active_rule_count(), 2);
    assert_eq!(
        pmp.entry(0).unwrap().range(),
        Some(RiscvPmpRange::new(0, 0x1000).unwrap())
    );
    assert_eq!(
        pmp.entry(1).unwrap().range(),
        Some(RiscvPmpRange::new(0x800, 0x1000).unwrap())
    );
    assert_eq!(
        pmp.check_access(
            0x900,
            8,
            RiscvPmpAccessKind::Read,
            RiscvPrivilegeMode::Supervisor,
        ),
        Ok(())
    );
    assert_eq!(
        pmp.check_access(
            0x900,
            8,
            RiscvPmpAccessKind::Write,
            RiscvPrivilegeMode::Supervisor,
        ),
        Err(RiscvPmpError::AccessDenied {
            address: 0x900,
            size: 8,
            kind: RiscvPmpAccessKind::Write,
            privilege: RiscvPrivilegeMode::Supervisor,
            matched_entry: Some(0),
        })
    );
}

#[test]
fn pmp_locked_entries_gate_machine_access_and_reject_later_writes() {
    let mut pmp = RiscvPmpTable::new(2).unwrap();

    pmp.write_addr(0, 0x4000 >> 2).unwrap();
    pmp.write_config(
        0,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Na4)
            .with_read(true)
            .with_locked(true),
    )
    .unwrap();

    assert_eq!(
        pmp.check_access(
            0x4000,
            4,
            RiscvPmpAccessKind::Write,
            RiscvPrivilegeMode::Machine,
        ),
        Err(RiscvPmpError::AccessDenied {
            address: 0x4000,
            size: 4,
            kind: RiscvPmpAccessKind::Write,
            privilege: RiscvPrivilegeMode::Machine,
            matched_entry: Some(0),
        })
    );
    assert_eq!(
        pmp.write_config(
            0,
            RiscvPmpConfig::new(RiscvPmpAddressMode::Na4).with_read(true),
        )
        .unwrap_err(),
        RiscvPmpError::EntryLocked { index: 0 }
    );
    assert_eq!(
        pmp.write_addr(0, 0x5000 >> 2).unwrap_err(),
        RiscvPmpError::EntryLocked { index: 0 }
    );
}

#[test]
fn pmp_locked_tor_entry_rejects_lower_bound_address_updates() {
    let mut pmp = RiscvPmpTable::new(2).unwrap();

    pmp.write_addr(0, 0x1000 >> 2).unwrap();
    pmp.write_addr(1, 0x2000 >> 2).unwrap();
    pmp.write_config(
        1,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
            .with_read(true)
            .with_locked(true),
    )
    .unwrap();

    assert_eq!(
        pmp.entry(1).unwrap().range(),
        Some(RiscvPmpRange::new(0x1000, 0x2000).unwrap())
    );
    assert_eq!(
        pmp.write_addr(0, 0x1800 >> 2).unwrap_err(),
        RiscvPmpError::NextTorEntryLocked { index: 0, next: 1 }
    );
    assert_eq!(
        pmp.entry(1).unwrap().range(),
        Some(RiscvPmpRange::new(0x1000, 0x2000).unwrap())
    );
}

#[test]
fn pmp_accepts_configuration_before_address_and_materializes_rule_later() {
    let mut pmp = RiscvPmpTable::new(1).unwrap();

    pmp.write_config(
        0,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Tor).with_read(true),
    )
    .unwrap();

    assert_eq!(pmp.active_rule_count(), 1);
    assert_eq!(pmp.entry(0).unwrap().range(), None);
    assert_eq!(
        pmp.check_access(
            0x80,
            4,
            RiscvPmpAccessKind::Read,
            RiscvPrivilegeMode::Supervisor,
        ),
        Err(RiscvPmpError::AccessDenied {
            address: 0x80,
            size: 4,
            kind: RiscvPmpAccessKind::Read,
            privilege: RiscvPrivilegeMode::Supervisor,
            matched_entry: None,
        })
    );

    pmp.write_addr(0, 0x1000 >> 2).unwrap();

    assert_eq!(
        pmp.entry(0).unwrap().range(),
        Some(RiscvPmpRange::new(0, 0x1000).unwrap())
    );
    assert_eq!(
        pmp.check_access(
            0x80,
            4,
            RiscvPmpAccessKind::Read,
            RiscvPrivilegeMode::Supervisor,
        ),
        Ok(())
    );
}

#[test]
fn pmp_snapshot_restore_round_trips_without_partial_mutation() {
    let mut pmp = RiscvPmpTable::new(2).unwrap();
    pmp.write_addr(0, 0x1000 >> 2).unwrap();
    pmp.write_config(
        0,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
            .with_read(true)
            .with_write(true),
    )
    .unwrap();
    pmp.write_addr(1, 0x2000 >> 2).unwrap();
    pmp.write_config(
        1,
        RiscvPmpConfig::new(RiscvPmpAddressMode::Tor)
            .with_read(true)
            .with_execute(true)
            .with_locked(true),
    )
    .unwrap();

    let snapshot = pmp.snapshot();
    let mut restored = RiscvPmpTable::new(2).unwrap();
    restored.restore(&snapshot).unwrap();

    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.entry(1).unwrap().range(),
        Some(RiscvPmpRange::new(0x1000, 0x2000).unwrap())
    );
    assert_eq!(
        restored.check_access(
            0x1800,
            4,
            RiscvPmpAccessKind::Execute,
            RiscvPrivilegeMode::User,
        ),
        Ok(())
    );

    let mut wrong_size = RiscvPmpTable::new(1).unwrap();
    let before = wrong_size.snapshot();
    assert_eq!(
        wrong_size.restore(&snapshot).unwrap_err(),
        RiscvPmpError::SnapshotEntryCountMismatch {
            expected: 1,
            actual: 2,
        }
    );
    assert_eq!(wrong_size.snapshot(), before);
}

#[test]
fn pmp_default_access_policy_distinguishes_absent_and_inactive_tables() {
    let empty = RiscvPmpTable::new(0).unwrap();
    assert_eq!(
        empty.check_access(
            0x8000,
            8,
            RiscvPmpAccessKind::Read,
            RiscvPrivilegeMode::User,
        ),
        Ok(())
    );

    let inactive = RiscvPmpTable::new(1).unwrap();
    assert_eq!(
        inactive.check_access(
            0x8000,
            8,
            RiscvPmpAccessKind::Read,
            RiscvPrivilegeMode::User,
        ),
        Err(RiscvPmpError::AccessDenied {
            address: 0x8000,
            size: 8,
            kind: RiscvPmpAccessKind::Read,
            privilege: RiscvPrivilegeMode::User,
            matched_entry: None,
        })
    );
    assert_eq!(
        inactive.check_access(
            0x8000,
            8,
            RiscvPmpAccessKind::Write,
            RiscvPrivilegeMode::Machine,
        ),
        Ok(())
    );
}
