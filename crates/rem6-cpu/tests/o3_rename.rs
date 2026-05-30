use rem6_cpu::{
    O3PhysicalRegisterId, O3RegisterClass, O3SourceRegister, O3SourceRenamePlan,
    O3SourceRenameReason,
};

#[test]
fn o3_source_rename_marks_invalid_register_class_ready_without_scoreboard_lookup() {
    let plan = O3SourceRenamePlan::for_sources([
        O3SourceRegister::invalid(),
        O3SourceRegister::mapped(
            O3RegisterClass::Integer,
            O3PhysicalRegisterId::new(17),
            false,
        ),
        O3SourceRegister::mapped(
            O3RegisterClass::FloatingPoint,
            O3PhysicalRegisterId::new(33),
            true,
        ),
    ]);

    assert_eq!(plan.scoreboard_lookup_count(), 2);
    assert!(plan.has_ready_source());
    assert!(plan.has_blocked_source());

    let invalid = &plan.decisions()[0];
    assert_eq!(invalid.source_index(), 0);
    assert_eq!(invalid.register_class(), None);
    assert_eq!(invalid.physical(), O3PhysicalRegisterId::invalid());
    assert!(!invalid.consults_scoreboard());
    assert!(invalid.mark_ready());
    assert_eq!(
        invalid.reason(),
        O3SourceRenameReason::InvalidRegisterClassReady
    );

    let blocked = &plan.decisions()[1];
    assert_eq!(blocked.source_index(), 1);
    assert_eq!(blocked.register_class(), Some(O3RegisterClass::Integer));
    assert_eq!(blocked.physical(), O3PhysicalRegisterId::new(17));
    assert!(blocked.consults_scoreboard());
    assert!(!blocked.mark_ready());
    assert_eq!(blocked.reason(), O3SourceRenameReason::ScoreboardNotReady);

    let ready = &plan.decisions()[2];
    assert_eq!(ready.source_index(), 2);
    assert_eq!(ready.register_class(), Some(O3RegisterClass::FloatingPoint));
    assert_eq!(ready.physical(), O3PhysicalRegisterId::new(33));
    assert!(ready.consults_scoreboard());
    assert!(ready.mark_ready());
    assert_eq!(ready.reason(), O3SourceRenameReason::ScoreboardReady);
}
