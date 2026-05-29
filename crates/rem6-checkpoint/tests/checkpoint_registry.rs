use rem6_checkpoint::{
    CheckpointChunk, CheckpointChunkSummary, CheckpointComponentId, CheckpointComponentSummary,
    CheckpointError, CheckpointManifest, CheckpointRegistry, CheckpointState,
};

#[test]
fn checkpoint_registry_captures_components_in_deterministic_order() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let memory = CheckpointComponentId::new("memory0").unwrap();
    let mut registry = CheckpointRegistry::new();
    registry.register(memory.clone()).unwrap();
    registry.register(cpu.clone()).unwrap();

    registry
        .write_chunk(&memory, "lines", vec![0xaa, 0xbb, 0xcc])
        .unwrap();
    registry
        .write_chunk(&cpu, "regs", vec![0x01, 0x02, 0x03, 0x04])
        .unwrap();
    registry.write_chunk(&cpu, "pc", vec![0x10, 0x00]).unwrap();

    assert_eq!(
        registry.capture("after-boot", 120).unwrap(),
        CheckpointManifest::new(
            "after-boot",
            120,
            vec![
                CheckpointState::new(
                    cpu.clone(),
                    vec![
                        CheckpointChunk::new("pc", vec![0x10, 0x00]),
                        CheckpointChunk::new("regs", vec![0x01, 0x02, 0x03, 0x04]),
                    ],
                ),
                CheckpointState::new(
                    memory.clone(),
                    vec![CheckpointChunk::new("lines", vec![0xaa, 0xbb, 0xcc])],
                ),
            ],
        )
    );
}

#[test]
fn checkpoint_manifest_reports_component_chunk_and_payload_totals() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let memory = CheckpointComponentId::new("memory0").unwrap();
    let manifest = CheckpointManifest::new(
        "audit",
        44,
        vec![
            CheckpointState::new(
                cpu.clone(),
                vec![
                    CheckpointChunk::new("pc", vec![0x10, 0x00]),
                    CheckpointChunk::new("regs", vec![0x01, 0x02, 0x03]),
                ],
            ),
            CheckpointState::new(
                memory.clone(),
                vec![CheckpointChunk::new("lines", vec![0xaa, 0xbb, 0xcc, 0xdd])],
            ),
        ],
    );

    let summary = manifest.summary();

    assert_eq!(summary.component_count(), 2);
    assert_eq!(summary.chunk_count(), 3);
    assert_eq!(summary.payload_bytes(), 9);
    assert_eq!(
        summary.component_summaries(),
        &[
            CheckpointComponentSummary::with_chunk_summaries(
                cpu,
                vec![
                    CheckpointChunkSummary::new("pc", 2),
                    CheckpointChunkSummary::new("regs", 3),
                ],
            ),
            CheckpointComponentSummary::with_chunk_summaries(
                memory,
                vec![CheckpointChunkSummary::new("lines", 4)],
            ),
        ]
    );
}

#[test]
fn checkpoint_manifest_summary_preserves_chunk_level_payload_evidence() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let memory = CheckpointComponentId::new("memory0").unwrap();
    let manifest = CheckpointManifest::new(
        "audit",
        55,
        vec![
            CheckpointState::new(
                cpu.clone(),
                vec![
                    CheckpointChunk::new("regs", vec![0x01, 0x02, 0x03]),
                    CheckpointChunk::new("pc", vec![0x10, 0x00]),
                ],
            ),
            CheckpointState::new(
                memory.clone(),
                vec![CheckpointChunk::new("lines", vec![0xaa, 0xbb, 0xcc, 0xdd])],
            ),
        ],
    );

    let summary = manifest.summary();
    let cpu_summary = summary.component_summary(&cpu).unwrap();
    let memory_summary = summary.component_summary(&memory).unwrap();

    assert_eq!(
        cpu_summary.chunk_summaries(),
        &[
            CheckpointChunkSummary::new("pc", 2),
            CheckpointChunkSummary::new("regs", 3),
        ]
    );
    assert_eq!(
        cpu_summary.chunk_summary("regs"),
        Some(&CheckpointChunkSummary::new("regs", 3))
    );
    assert_eq!(cpu_summary.chunk_summary("xregs"), None);
    assert_eq!(
        memory_summary.chunk_summaries(),
        &[CheckpointChunkSummary::new("lines", 4)]
    );
    assert_eq!(
        summary.component_summary(&CheckpointComponentId::new("gpu0").unwrap()),
        None
    );
}

#[test]
fn checkpoint_registry_restores_manifest_chunks() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let memory = CheckpointComponentId::new("memory0").unwrap();
    let manifest = CheckpointManifest::new(
        "warm",
        88,
        vec![
            CheckpointState::new(
                cpu.clone(),
                vec![CheckpointChunk::new("regs", vec![1, 2, 3])],
            ),
            CheckpointState::new(
                memory.clone(),
                vec![CheckpointChunk::new("lines", vec![4, 5, 6])],
            ),
        ],
    );
    let mut registry = CheckpointRegistry::new();
    registry.register(cpu.clone()).unwrap();
    registry.register(memory.clone()).unwrap();

    registry.restore(&manifest).unwrap();

    assert_eq!(registry.chunk(&cpu, "regs"), Some(&[1, 2, 3][..]));
    assert_eq!(registry.chunk(&memory, "lines"), Some(&[4, 5, 6][..]));
}

#[test]
fn checkpoint_registry_rejects_invalid_components_and_chunks() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let unknown = CheckpointComponentId::new("cpu1").unwrap();
    let mut registry = CheckpointRegistry::new();

    assert_eq!(
        CheckpointComponentId::new("").unwrap_err(),
        CheckpointError::EmptyComponentId
    );
    registry.register(cpu.clone()).unwrap();
    assert_eq!(
        registry.register(cpu.clone()).unwrap_err(),
        CheckpointError::DuplicateComponent {
            component: cpu.clone(),
        }
    );
    assert_eq!(
        registry.write_chunk(&unknown, "regs", vec![1]).unwrap_err(),
        CheckpointError::UnknownComponent { component: unknown }
    );
    assert_eq!(
        registry.write_chunk(&cpu, "", vec![1]).unwrap_err(),
        CheckpointError::EmptyChunkName {
            component: cpu.clone(),
        }
    );
    assert_eq!(
        registry.capture("", 10).unwrap_err(),
        CheckpointError::EmptyLabel
    );
}

#[test]
fn checkpoint_restore_rejects_unknown_or_duplicate_manifest_state() {
    let cpu = CheckpointComponentId::new("cpu0").unwrap();
    let other = CheckpointComponentId::new("cpu1").unwrap();
    let mut registry = CheckpointRegistry::new();
    registry.register(cpu.clone()).unwrap();

    let unknown = CheckpointManifest::new(
        "bad",
        3,
        vec![CheckpointState::new(
            other.clone(),
            vec![CheckpointChunk::new("regs", vec![1])],
        )],
    );
    assert_eq!(
        registry.restore(&unknown).unwrap_err(),
        CheckpointError::UnknownComponent { component: other }
    );

    let duplicate = CheckpointManifest::new(
        "bad",
        4,
        vec![
            CheckpointState::new(cpu.clone(), vec![CheckpointChunk::new("regs", vec![1])]),
            CheckpointState::new(cpu.clone(), vec![CheckpointChunk::new("pc", vec![2])]),
        ],
    );
    assert_eq!(
        registry.restore(&duplicate).unwrap_err(),
        CheckpointError::DuplicateComponent { component: cpu }
    );
}
