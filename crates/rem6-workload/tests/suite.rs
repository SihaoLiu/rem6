use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadError, WorkloadId, WorkloadManifest, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResult, WorkloadSuite, WorkloadSuiteDispatchPlan,
    WorkloadSuiteId, WorkloadSuiteReplayPlan, WorkloadSuiteResult,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn suite_id(value: &str) -> WorkloadSuiteId {
    WorkloadSuiteId::new(value).unwrap()
}

fn resource_id(value: &str) -> WorkloadResourceId {
    WorkloadResourceId::new(value).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
}

fn kernel_resource(digest: &str) -> WorkloadResource {
    WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        digest,
        "resources/kernel.elf",
    )
    .unwrap()
}

fn manifest(workload: &str, digest: &str) -> WorkloadManifest {
    WorkloadManifest::builder(id(workload), boot_image())
        .add_resource(kernel_resource(digest))
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap()
}

#[test]
fn workload_suite_orders_manifests_and_preserves_identity() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let suite = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();
    let reordered = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(suite.identity(), reordered.identity());
    assert_eq!(suite.entries()[0].workload_id(), alpha.id());
    assert_eq!(suite.entries()[1].workload_id(), beta.id());

    let plan = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    assert_eq!(plan.suite_identity(), suite.identity());
    assert_eq!(plan.plans().len(), 2);
    assert_eq!(plan.plans()[0].manifest_identity(), alpha.identity());
    assert_eq!(plan.plans()[1].manifest_identity(), beta.identity());
}

#[test]
fn workload_suite_rejects_duplicate_workload_ids() {
    let first = manifest("dup", "sha256:first");
    let second = manifest("dup", "sha256:second");
    let error = WorkloadSuite::builder(suite_id("dups"))
        .add_manifest(first)
        .unwrap()
        .add_manifest(second)
        .unwrap_err();

    assert!(matches!(
        error,
        WorkloadError::DuplicateSuiteWorkload { workload } if workload == id("dup")
    ));
}

#[test]
fn workload_suite_result_verifies_manifest_identities() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("riscv-mix"))
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .build()
        .unwrap();

    let result = WorkloadSuiteResult::new(suite.identity())
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 20))
        .unwrap()
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap();
    assert_eq!(result.results()[0].workload_id(), alpha.id());
    result.verify_against(&suite).unwrap();

    let missing = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        missing,
        WorkloadError::MissingSuiteWorkloadResult { workload } if workload == *beta.id()
    ));

    let unexpected = WorkloadSuiteResult::new(suite.identity())
        .add_result(
            alpha.id().clone(),
            WorkloadResult::new(alpha.identity(), 10),
        )
        .unwrap()
        .add_result(
            gamma.id().clone(),
            WorkloadResult::new(gamma.identity(), 30),
        )
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        unexpected,
        WorkloadError::UnexpectedSuiteWorkloadResult { workload } if workload == *gamma.id()
    ));

    let drifted = WorkloadSuiteResult::new(suite.identity())
        .add_result(alpha.id().clone(), WorkloadResult::new(beta.identity(), 10))
        .unwrap()
        .add_result(beta.id().clone(), WorkloadResult::new(beta.identity(), 20))
        .unwrap()
        .verify_against(&suite)
        .unwrap_err();
    assert!(matches!(
        drifted,
        WorkloadError::SuiteWorkloadResultManifestMismatch { workload, expected, actual }
            if workload == *alpha.id()
                && expected == alpha.identity()
                && actual == beta.identity()
    ));
}

#[test]
fn workload_suite_dispatch_plan_assigns_sorted_manifests_to_workers() {
    let alpha = manifest("alpha", "sha256:alpha");
    let beta = manifest("beta", "sha256:beta");
    let gamma = manifest("gamma", "sha256:gamma");
    let suite = WorkloadSuite::builder(suite_id("dispatch"))
        .add_manifest(gamma.clone())
        .unwrap()
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .build()
        .unwrap();
    let reordered = WorkloadSuite::builder(suite_id("dispatch"))
        .add_manifest(alpha.clone())
        .unwrap()
        .add_manifest(beta.clone())
        .unwrap()
        .add_manifest(gamma.clone())
        .unwrap()
        .build()
        .unwrap();

    let plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        2,
    )
    .unwrap();
    let reordered_plan = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&reordered).unwrap(),
        2,
    )
    .unwrap();

    assert_eq!(plan, reordered_plan);
    assert_eq!(plan.suite_identity(), suite.identity());
    assert_eq!(plan.worker_count(), 2);
    assert_eq!(plan.active_worker_count(), 2);
    assert_eq!(plan.records().len(), 3);
    assert_eq!(plan.records()[0].workload_id(), alpha.id());
    assert_eq!(plan.records()[0].worker_index(), 0);
    assert_eq!(plan.records()[0].dispatch_order(), 0);
    assert_eq!(plan.records()[0].manifest_identity(), alpha.identity());
    assert_eq!(plan.records()[1].workload_id(), beta.id());
    assert_eq!(plan.records()[1].worker_index(), 1);
    assert_eq!(plan.records()[1].dispatch_order(), 1);
    assert_eq!(plan.records()[2].workload_id(), gamma.id());
    assert_eq!(plan.records()[2].worker_index(), 0);
    assert_eq!(plan.records()[2].dispatch_order(), 2);
}

#[test]
fn workload_suite_dispatch_plan_rejects_zero_workers() {
    let suite = WorkloadSuite::builder(suite_id("zero-workers"))
        .add_manifest(manifest("alpha", "sha256:alpha"))
        .unwrap()
        .build()
        .unwrap();
    let error = WorkloadSuiteDispatchPlan::from_replay_plan(
        &WorkloadSuiteReplayPlan::from_suite(&suite).unwrap(),
        0,
    )
    .unwrap_err();

    assert!(matches!(error, WorkloadError::ZeroWorkloadSuiteWorkers));
}
