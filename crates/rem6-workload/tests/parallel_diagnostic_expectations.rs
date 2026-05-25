use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadExpectedCleanParallelDiagnostics, WorkloadId,
    WorkloadParallelDiagnosticScope, WorkloadParallelExecutionSummary, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceId, WorkloadResourceKind, WorkloadResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
}

fn kernel_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        "sha256:kernel",
        "resources/kernel.elf",
    )
    .unwrap()
}

fn expected_clean(
    scope: WorkloadParallelDiagnosticScope,
) -> WorkloadExpectedCleanParallelDiagnostics {
    WorkloadExpectedCleanParallelDiagnostics::new(scope)
}

#[test]
fn workload_manifest_records_clean_parallel_diagnostic_expectations() {
    let resource = expected_clean(WorkloadParallelDiagnosticScope::Resource);
    let data_cache = expected_clean(WorkloadParallelDiagnosticScope::DataCache);
    let compute = expected_clean(WorkloadParallelDiagnosticScope::Compute);
    let dma = expected_clean(WorkloadParallelDiagnosticScope::Dma);
    let full_system = expected_clean(WorkloadParallelDiagnosticScope::FullSystem);
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("manifest-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(full_system)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(dma)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(resource)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(compute)
            .unwrap()
            .add_expected_clean_parallel_diagnostics(data_cache)
            .unwrap()
            .build()
            .unwrap();

    assert_eq!(
        manifest.expected_clean_parallel_diagnostics(),
        &[resource, data_cache, compute, dma, full_system],
    );
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    assert_eq!(
        plan.expected_clean_parallel_diagnostics(),
        manifest.expected_clean_parallel_diagnostics(),
    );
    assert_eq!(
        WorkloadParallelDiagnosticScope::Resource.as_str(),
        "resource"
    );
    assert_eq!(
        WorkloadParallelDiagnosticScope::DataCache.as_str(),
        "data-cache"
    );
    assert_eq!(WorkloadParallelDiagnosticScope::Compute.as_str(), "compute");
    assert_eq!(WorkloadParallelDiagnosticScope::Dma.as_str(), "dma");
    assert_eq!(
        WorkloadParallelDiagnosticScope::FullSystem.as_str(),
        "full-system",
    );

    let result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(WorkloadParallelExecutionSummary::default());
    plan.verify_result(&result).unwrap();
}

#[test]
fn workload_manifest_identity_changes_with_clean_parallel_diagnostics() {
    let base =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let resource =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::Resource,
            ))
            .unwrap()
            .build()
            .unwrap();
    let full_system =
        rem6_workload::WorkloadManifest::builder(id("identity-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::FullSystem,
            ))
            .unwrap()
            .build()
            .unwrap();

    assert_ne!(base.identity(), resource.identity());
    assert_ne!(resource.identity(), full_system.identity());
}

#[test]
fn workload_replay_plan_rejects_missing_or_dirty_parallel_diagnostics() {
    let manifest = rem6_workload::WorkloadManifest::builder(id("dirty-diagnostics"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest)
        .unwrap()
        .add_expected_clean_parallel_diagnostics(expected_clean(
            WorkloadParallelDiagnosticScope::FullSystem,
        ))
        .unwrap();

    let missing_summary = WorkloadResult::new(plan.manifest_identity(), 32);
    assert_eq!(
        plan.verify_result(&missing_summary).unwrap_err(),
        WorkloadError::MissingParallelDiagnosticSummary {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
        },
    );

    let dirty_summary = WorkloadParallelExecutionSummary::default()
        .with_data_cache_diagnostics(2, 0)
        .with_resource_diagnostics(0, 1, 0, 0)
        .with_gpu_compute_diagnostics(3, 0)
        .with_accelerator_dma_diagnostics(0, 4);
    let dirty_result = WorkloadResult::new(plan.manifest_identity(), 32)
        .with_parallel_execution_summary(dirty_summary);
    assert_eq!(
        plan.verify_result(&dirty_result).unwrap_err(),
        WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
            scope: WorkloadParallelDiagnosticScope::FullSystem,
            wait_for_edge_count: 5,
            deadlock_diagnostic_count: 5,
            livelock_diagnostic_count: 0,
        },
    );
}

#[test]
fn workload_replay_plan_rejects_duplicate_clean_parallel_diagnostics() {
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("duplicate-clean-diagnostics"), boot_image())
            .add_resource(kernel_resource())
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::DataCache,
            ))
            .unwrap()
            .add_expected_clean_parallel_diagnostics(expected_clean(
                WorkloadParallelDiagnosticScope::DataCache,
            ))
            .unwrap_err();
    assert_eq!(
        manifest,
        WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
            scope: WorkloadParallelDiagnosticScope::DataCache,
        },
    );
}
