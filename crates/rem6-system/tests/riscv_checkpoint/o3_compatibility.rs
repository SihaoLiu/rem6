use super::*;

const O3_PENDING_STATE_CHUNK: &str = "o3-pending-state";
const O3_RUNTIME_STATE_CHUNK: &str = "o3-runtime-state";

fn simple_pending_payload(scope: u64, sequence: u64) -> O3PendingStateCheckpointPayload {
    O3PendingStateCheckpointPayload::from_snapshot(
        O3PendingStateSnapshot::new(
            [O3DependencyScopeId::new(scope)],
            [O3ScopedReadyInstruction::new(
                sequence,
                O3IssueQueueId::new(0),
                O3IssueOpClass::IntAlu,
            )],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [O3WritebackCompletion::new(sequence + 1)],
            ),
        )
        .unwrap(),
    )
    .unwrap()
}

fn registry_without_chunks(
    registry: &CheckpointRegistry,
    component: &CheckpointComponentId,
    omitted: &[&str],
) -> CheckpointRegistry {
    let manifest = registry.capture("filtered-o3-checkpoint", 0).unwrap();
    let state = manifest
        .states()
        .iter()
        .find(|state| state.component() == component)
        .unwrap();
    let mut filtered = CheckpointRegistry::new();
    filtered.register(component.clone()).unwrap();
    for chunk in state.chunks() {
        if !omitted.contains(&chunk.name()) {
            filtered
                .write_chunk(component, chunk.name(), chunk.payload().to_vec())
                .unwrap();
        }
    }
    filtered
}

#[test]
fn riscv_core_checkpoint_restores_runtime_only_and_no_o3_chunk_variants() {
    let runtime_payload = runtime_payload_with_pending(simple_pending_payload(0x606, 61));
    let default_payload = RiscvCore::default_o3_runtime_checkpoint_payload();
    let sentinel_payload = runtime_payload_with_pending(simple_pending_payload(0x707, 71));
    let cases: [(&[&str], O3RuntimeCheckpointPayload); 2] = [
        (&[O3_PENDING_STATE_CHUNK], runtime_payload.clone()),
        (
            &[O3_PENDING_STATE_CHUNK, O3_RUNTIME_STATE_CHUNK],
            default_payload,
        ),
    ];

    for (omitted, expected) in cases {
        let component = CheckpointComponentId::new("cpu0").unwrap();
        let mut registry = CheckpointRegistry::new();
        let core = riscv_core();
        let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());

        core.restore_o3_runtime_checkpoint_payload(runtime_payload.clone())
            .unwrap();
        port.register(&mut registry).unwrap();
        port.capture_into(&mut registry).unwrap();
        let filtered = registry_without_chunks(&registry, &component, omitted);
        core.restore_o3_runtime_checkpoint_payload(sentinel_payload.clone())
            .unwrap();

        let restored = port.restore_from(&filtered).unwrap();

        assert_eq!(restored.o3_runtime_payload(), &expected);
        assert_eq!(core.o3_runtime_checkpoint_payload(), expected);
        for chunk in omitted {
            assert!(filtered.chunk(&component, chunk).is_none());
        }
    }
}

#[derive(Clone, Copy)]
enum MalformedO3Chunk {
    Runtime,
    Pending { runtime_present: bool },
}

#[test]
fn riscv_core_checkpoint_rejects_malformed_o3_chunks_without_partial_restore() {
    let captured_payload = runtime_payload_with_pending(simple_pending_payload(0x808, 81));
    let sentinel_payload = runtime_payload_with_pending(simple_pending_payload(0x909, 91));

    for malformed in [
        MalformedO3Chunk::Runtime,
        MalformedO3Chunk::Pending {
            runtime_present: true,
        },
        MalformedO3Chunk::Pending {
            runtime_present: false,
        },
    ] {
        let component = CheckpointComponentId::new("cpu0").unwrap();
        let mut registry = CheckpointRegistry::new();
        let core = riscv_core();
        let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());

        core.restore_o3_runtime_checkpoint_payload(captured_payload.clone())
            .unwrap();
        port.register(&mut registry).unwrap();
        port.capture_into(&mut registry).unwrap();
        let omitted = match malformed {
            MalformedO3Chunk::Pending {
                runtime_present: false,
            } => &[O3_RUNTIME_STATE_CHUNK][..],
            MalformedO3Chunk::Runtime
            | MalformedO3Chunk::Pending {
                runtime_present: true,
            } => &[][..],
        };
        let mut malformed_registry = registry_without_chunks(&registry, &component, omitted);
        let chunk = match malformed {
            MalformedO3Chunk::Runtime => O3_RUNTIME_STATE_CHUNK,
            MalformedO3Chunk::Pending { .. } => O3_PENDING_STATE_CHUNK,
        };
        malformed_registry
            .write_chunk(&component, chunk, vec![0xff])
            .unwrap();

        core.restore_o3_runtime_checkpoint_payload(sentinel_payload.clone())
            .unwrap();
        core.redirect_pc(Address::new(0x9000));
        core.write_register(reg(1), 0xdead_beef);

        let error = port.restore_from(&malformed_registry).unwrap_err();

        match (malformed, error) {
            (
                MalformedO3Chunk::Runtime,
                rem6_system::RiscvCoreCheckpointError::InvalidO3RuntimeSnapshot {
                    component: actual,
                    ..
                },
            ) => assert_eq!(actual, component),
            (
                MalformedO3Chunk::Pending { .. },
                rem6_system::RiscvCoreCheckpointError::InvalidO3PendingStateSnapshot {
                    component: actual,
                    ..
                },
            ) => assert_eq!(actual, component),
            (_, error) => panic!("unexpected malformed O3 checkpoint error: {error}"),
        }
        assert_eq!(core.o3_runtime_checkpoint_payload(), sentinel_payload);
        assert_eq!(core.pc(), Address::new(0x9000));
        assert_eq!(core.read_register(reg(1)), 0xdead_beef);
    }
}
