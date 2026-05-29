use rem6_workload::{
    WorkloadCheckpointChunkSummary, WorkloadCheckpointComponentSummary,
    WorkloadCheckpointManifestSummary,
};

#[test]
fn workload_checkpoint_summary_preserves_chunk_level_payload_evidence() {
    let summary = WorkloadCheckpointManifestSummary::with_component_summaries(
        "warm",
        80,
        [
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "cpu0",
                [
                    WorkloadCheckpointChunkSummary::new("regs", 6),
                    WorkloadCheckpointChunkSummary::new("pc", 2),
                ],
            ),
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "memory0",
                [WorkloadCheckpointChunkSummary::new("lines", 16)],
            ),
        ],
    );

    assert_eq!(summary.component_count(), 2);
    assert_eq!(summary.chunk_count(), 3);
    assert_eq!(summary.payload_bytes(), 24);

    let cpu = summary.component_summary("cpu0").unwrap();
    assert_eq!(cpu.chunk_count(), 2);
    assert_eq!(cpu.payload_bytes(), 8);
    assert_eq!(
        cpu.chunk_summaries(),
        &[
            WorkloadCheckpointChunkSummary::new("pc", 2),
            WorkloadCheckpointChunkSummary::new("regs", 6),
        ]
    );
    assert_eq!(
        cpu.chunk_summary("regs"),
        Some(&WorkloadCheckpointChunkSummary::new("regs", 6))
    );
    assert_eq!(cpu.chunk_summary("xregs"), None);
    assert_eq!(summary.component_summary("gpu0"), None);
}

#[test]
fn workload_checkpoint_summary_canonicalizes_duplicate_chunk_names() {
    let summary = WorkloadCheckpointComponentSummary::with_chunk_summaries(
        "cpu0",
        [
            WorkloadCheckpointChunkSummary::new("regs", 4),
            WorkloadCheckpointChunkSummary::new("pc", 2),
            WorkloadCheckpointChunkSummary::new("regs", 8),
        ],
    );

    assert_eq!(summary.chunk_count(), 2);
    assert_eq!(summary.payload_bytes(), 10);
    assert_eq!(
        summary.chunk_summaries(),
        &[
            WorkloadCheckpointChunkSummary::new("pc", 2),
            WorkloadCheckpointChunkSummary::new("regs", 8),
        ]
    );
}

#[test]
fn workload_checkpoint_manifest_summary_canonicalizes_duplicate_components() {
    let summary = WorkloadCheckpointManifestSummary::with_component_summaries(
        "warm",
        80,
        [
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "cpu0",
                [
                    WorkloadCheckpointChunkSummary::new("pc", 2),
                    WorkloadCheckpointChunkSummary::new("regs", 4),
                ],
            ),
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "memory0",
                [WorkloadCheckpointChunkSummary::new("lines", 16)],
            ),
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "cpu0",
                [
                    WorkloadCheckpointChunkSummary::new("regs", 8),
                    WorkloadCheckpointChunkSummary::new("flags", 1),
                ],
            ),
        ],
    );

    assert_eq!(summary.component_count(), 2);
    assert_eq!(summary.chunk_count(), 4);
    assert_eq!(summary.payload_bytes(), 27);
    assert_eq!(
        summary.component_summaries(),
        &[
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "cpu0",
                [
                    WorkloadCheckpointChunkSummary::new("flags", 1),
                    WorkloadCheckpointChunkSummary::new("pc", 2),
                    WorkloadCheckpointChunkSummary::new("regs", 8),
                ],
            ),
            WorkloadCheckpointComponentSummary::with_chunk_summaries(
                "memory0",
                [WorkloadCheckpointChunkSummary::new("lines", 16)],
            ),
        ]
    );
}

#[test]
fn workload_checkpoint_manifest_summary_keeps_strongest_aggregate_component_totals() {
    let summary = WorkloadCheckpointManifestSummary::with_component_summaries(
        "warm",
        80,
        [
            WorkloadCheckpointComponentSummary::new("cpu0", 2, 8),
            WorkloadCheckpointComponentSummary::new("cpu0", 1, 16),
        ],
    );

    assert_eq!(summary.component_count(), 1);
    assert_eq!(summary.chunk_count(), 2);
    assert_eq!(summary.payload_bytes(), 16);
    let cpu = summary.component_summary("cpu0").unwrap();
    assert_eq!(cpu.chunk_count(), 2);
    assert_eq!(cpu.payload_bytes(), 16);
    assert!(cpu.chunk_summaries().is_empty());
}
