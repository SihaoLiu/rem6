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
