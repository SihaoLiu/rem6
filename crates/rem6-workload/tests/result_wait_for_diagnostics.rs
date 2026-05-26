use rem6_kernel::WaitForNode;
use rem6_workload::{WorkloadParallelExecutionSummary, WorkloadWaitForBlockedNodeWindow};

fn wait_resource(value: &str) -> WaitForNode {
    WaitForNode::resource(value).unwrap()
}

#[test]
fn workload_result_preserves_wait_for_blocked_node_tick_windows() {
    let cpu_core = wait_resource("cpu.core.0");
    let fabric_port = wait_resource("fabric.port.0");
    let dram_controller = wait_resource("dram.controller.0");
    let gpu_wave = wait_resource("gpu.wave.0");
    let accelerator_cmd = wait_resource("accelerator.cmd.0");
    let gpu_dma_engine = wait_resource("gpu.dma.engine.0");
    let accelerator_dma_engine = wait_resource("accelerator.dma.engine.0");
    let summary =
        WorkloadParallelExecutionSummary::default()
            .with_data_cache_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
                cpu_core.clone(),
                2,
                4,
                9,
            )])
            .with_resource_wait_for_blocked_node_windows(
                [WorkloadWaitForBlockedNodeWindow::new(
                    fabric_port.clone(),
                    3,
                    5,
                    11,
                )],
                [WorkloadWaitForBlockedNodeWindow::new(
                    dram_controller.clone(),
                    2,
                    3,
                    13,
                )],
            )
            .with_gpu_compute_wait_for_blocked_node_windows([
                WorkloadWaitForBlockedNodeWindow::new(gpu_wave.clone(), 5, 2, 14),
            ])
            .with_accelerator_compute_wait_for_blocked_node_windows([
                WorkloadWaitForBlockedNodeWindow::new(accelerator_cmd.clone(), 7, 6, 18),
            ])
            .with_gpu_dma_wait_for_blocked_node_windows([WorkloadWaitForBlockedNodeWindow::new(
                gpu_dma_engine.clone(),
                11,
                8,
                21,
            )])
            .with_accelerator_dma_wait_for_blocked_node_windows([
                WorkloadWaitForBlockedNodeWindow::new(accelerator_dma_engine.clone(), 13, 10, 22),
            ]);

    assert_eq!(
        summary.data_cache_wait_for_blocked_node_window(&cpu_core),
        Some(WorkloadWaitForBlockedNodeWindow::new(
            cpu_core.clone(),
            2,
            4,
            9,
        )),
    );
    assert_eq!(
        summary.resource_wait_for_blocked_node_window(&dram_controller),
        Some(WorkloadWaitForBlockedNodeWindow::new(
            dram_controller.clone(),
            2,
            3,
            13,
        )),
    );
    assert_eq!(
        summary.full_system_wait_for_blocked_node_window(&accelerator_dma_engine),
        Some(WorkloadWaitForBlockedNodeWindow::new(
            accelerator_dma_engine.clone(),
            13,
            10,
            22,
        )),
    );
    assert_eq!(summary.full_system_wait_for_edge_count(), 43);
    assert_eq!(
        summary.full_system_wait_for_blocked_node_windows(),
        vec![
            WorkloadWaitForBlockedNodeWindow::new(accelerator_cmd, 7, 6, 18),
            WorkloadWaitForBlockedNodeWindow::new(accelerator_dma_engine, 13, 10, 22),
            WorkloadWaitForBlockedNodeWindow::new(cpu_core, 2, 4, 9),
            WorkloadWaitForBlockedNodeWindow::new(dram_controller, 2, 3, 13),
            WorkloadWaitForBlockedNodeWindow::new(fabric_port, 3, 5, 11),
            WorkloadWaitForBlockedNodeWindow::new(gpu_dma_engine, 11, 8, 21),
            WorkloadWaitForBlockedNodeWindow::new(gpu_wave, 5, 2, 14),
        ],
    );
}
