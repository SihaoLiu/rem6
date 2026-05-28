use rem6_workload::WorkloadParallelExecutionSummary;

#[test]
fn workload_summary_marks_active_device_counts_as_heterogeneous_activity() {
    let gpu_compute =
        WorkloadParallelExecutionSummary::default().with_gpu_compute_counts(0, 0, 0, 1);
    assert!(gpu_compute.has_gpu_compute_activity());

    let gpu_dma = WorkloadParallelExecutionSummary::default().with_gpu_dma_counts(0, 0, 1);
    assert!(gpu_dma.has_gpu_dma_activity());

    let accelerator_compute =
        WorkloadParallelExecutionSummary::default().with_accelerator_compute_counts(0, 0, 0, 1);
    assert!(accelerator_compute.has_accelerator_compute_activity());
    assert!(!accelerator_compute.has_accelerator_npu_activity());

    let accelerator_dma =
        WorkloadParallelExecutionSummary::default().with_accelerator_dma_counts(0, 0, 1);
    assert!(accelerator_dma.has_accelerator_dma_activity());
}
