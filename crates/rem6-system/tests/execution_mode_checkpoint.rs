use rem6_checkpoint::{
    CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointRegistry, CheckpointState,
};
use rem6_kernel::PartitionId;
use rem6_stats::StatsRegistry;
use rem6_system::{
    ExecutionMode, ExecutionModeTarget, GuestEventId, GuestSourceId, HostAction, HostActionRecord,
    SystemActionExecutor, SystemActionOutcome,
};

fn action(tick: u64, event: u64, action: HostAction) -> HostActionRecord {
    HostActionRecord::new(
        tick,
        PartitionId::new(0),
        PartitionId::new(1),
        GuestEventId::new(event),
        GuestSourceId::new(7),
        action,
    )
}

fn capture(
    executor: &mut SystemActionExecutor,
    tick: u64,
    event: u64,
    label: &str,
) -> CheckpointManifest {
    match executor
        .apply(&action(
            tick,
            event,
            HostAction::Checkpoint {
                label: label.to_string(),
            },
        ))
        .unwrap()
    {
        SystemActionOutcome::Checkpoint { manifest, .. } => manifest,
        other => panic!("unexpected checkpoint outcome: {other:?}"),
    }
}

fn has_execution_mode_component(manifest: &CheckpointManifest) -> bool {
    manifest
        .states()
        .iter()
        .any(|state| state.component().as_str() == "host.execution_modes")
}

#[test]
fn empty_execution_mode_restore_prunes_owned_component_with_extra_chunks() {
    let component = CheckpointComponentId::new("host.execution_modes").unwrap();
    let target = ExecutionModeTarget::new("cpu0");
    let mut executor = SystemActionExecutor::new(StatsRegistry::new());

    executor
        .apply(&action(
            1,
            1,
            HostAction::SwitchExecutionMode {
                target: target.clone(),
                mode: ExecutionMode::Detailed,
            },
        ))
        .unwrap();
    let with_authority = capture(&mut executor, 2, 2, "with-authority");
    assert!(has_execution_mode_component(&with_authority));

    let empty_with_extra = CheckpointManifest::new(
        "empty-with-extra",
        3,
        vec![CheckpointState::new(
            component.clone(),
            vec![
                CheckpointChunk::new("modes", 0_u64.to_le_bytes().to_vec()),
                CheckpointChunk::new("legacy-extra", vec![1]),
            ],
        )],
    );
    executor
        .apply(&action(
            3,
            3,
            HostAction::RestoreCheckpoint {
                manifest: empty_with_extra.clone(),
            },
        ))
        .unwrap();

    assert_eq!(executor.execution_mode(&target), None);
    assert!(!executor.checkpoints().contains_component(&component));
    assert!(!has_execution_mode_component(&capture(
        &mut executor,
        4,
        4,
        "after-empty-restore"
    )));

    executor
        .apply(&action(
            5,
            5,
            HostAction::RestoreCheckpoint {
                manifest: with_authority,
            },
        ))
        .unwrap();
    assert_eq!(
        executor.execution_mode(&target),
        Some(ExecutionMode::Detailed)
    );
    assert!(executor.checkpoints().contains_component(&component));

    executor
        .apply(&action(
            6,
            6,
            HostAction::RestoreCheckpoint {
                manifest: empty_with_extra,
            },
        ))
        .unwrap();
    assert_eq!(executor.execution_mode(&target), None);
    assert!(!executor.checkpoints().contains_component(&component));
}

#[test]
fn empty_execution_modes_prune_prepopulated_checkpoint_component() {
    let component = CheckpointComponentId::new("host.execution_modes").unwrap();
    let mut checkpoints = CheckpointRegistry::new();
    checkpoints.register(component.clone()).unwrap();
    checkpoints
        .write_chunk(&component, "modes", 0_u64.to_le_bytes().to_vec())
        .unwrap();
    checkpoints
        .write_chunk(&component, "legacy-extra", vec![1])
        .unwrap();
    let mut executor = SystemActionExecutor::with_checkpoint(StatsRegistry::new(), checkpoints);

    assert!(!has_execution_mode_component(&capture(
        &mut executor,
        1,
        1,
        "empty-live-authority"
    )));
}
