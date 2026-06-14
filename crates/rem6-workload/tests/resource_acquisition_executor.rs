use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadId, WorkloadInMemoryResourceAcquisitionExecutor, WorkloadReplayPlan,
    WorkloadResolvedResources, WorkloadResource, WorkloadResourceAcquisition,
    WorkloadResourceAcquisitionError, WorkloadResourceAcquisitionKind, WorkloadResourceArtifact,
    WorkloadResourceId, WorkloadResourceKind, WorkloadSuite, WorkloadSuiteId,
    WorkloadSuiteReplayPlan,
};

const KERNEL_BYTES: &[u8] = &[0x13, 0x05, 0x00, 0x00];
const KERNEL_DIGEST: &str = "fnv64:181a4a0d6fe792ed";
const KERNEL_SHA_DIGEST: &str =
    "sha256:8eb2ca3a315d85c5c4f3e7c51143103fb77732f50784133ae2fda66ccde3e4b2";
const TRACE_BYTES: &[u8] = b"trace-data";
const TRACE_DIGEST: &str = "fnv64:76d3b162d75e3109";

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
        .add_segment(Address::new(0x8000), KERNEL_BYTES.to_vec())
        .unwrap()
}

fn preloaded(locator: &str, revision: &str) -> WorkloadResourceAcquisition {
    WorkloadResourceAcquisition::new(WorkloadResourceAcquisitionKind::Preloaded, locator)
        .unwrap()
        .with_tool("memory-artifact-catalog")
        .unwrap()
        .with_revision(revision)
        .unwrap()
}

fn resource(
    name: &str,
    kind: WorkloadResourceKind,
    digest: &str,
    locator: &str,
    acquisition: WorkloadResourceAcquisition,
) -> WorkloadResource {
    WorkloadResource::new(resource_id(name), kind, digest, locator)
        .unwrap()
        .with_acquisition(acquisition)
}

#[test]
fn in_memory_executor_acquires_required_resources_with_payload_provenance() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
        acquisition.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("local-artifact"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition.clone(),
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let records = executor.acquire_replay_plan(&plan).unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].resource(), &resource_id("kernel"));
    assert_eq!(records[0].kind(), WorkloadResourceKind::Kernel);
    assert_eq!(records[0].digest(), KERNEL_DIGEST);
    assert_eq!(records[0].size_bytes(), KERNEL_BYTES.len());
    assert_eq!(records[0].acquisition(), &acquisition);
    assert_eq!(records[0].payload().data(), KERNEL_BYTES);

    let resolved = WorkloadResolvedResources::from_manifest(
        &manifest,
        records.into_iter().map(|record| record.into_payload()),
    )
    .unwrap();
    assert_eq!(
        resolved.payload_data(&resource_id("kernel")).unwrap(),
        KERNEL_BYTES
    );
}

#[test]
fn in_memory_executor_rejects_digest_mismatch() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        "fnv64:bad",
        "resources/kernel.elf",
        acquisition.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("bad-digest"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition,
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let error = executor.acquire_replay_plan(&plan).unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::DigestMismatch {
            resource: resource_id("kernel"),
            expected: "fnv64:bad".to_string(),
            actual: KERNEL_DIGEST.to_string(),
        }
    );
}

#[test]
fn in_memory_executor_accepts_matching_opaque_digest_schemes() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_SHA_DIGEST,
        "resources/kernel.elf",
        acquisition.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("opaque-digest"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition,
            KERNEL_SHA_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let records = executor.acquire_replay_plan(&plan).unwrap();

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].digest(), KERNEL_SHA_DIGEST);
    assert_eq!(records[0].payload().digest(), KERNEL_SHA_DIGEST);
    assert_eq!(records[0].payload().data(), KERNEL_BYTES);
}

#[test]
fn in_memory_executor_rejects_missing_resource_acquisition() {
    let kernel = WorkloadResource::new(
        resource_id("kernel"),
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
    )
    .unwrap();
    let manifest =
        rem6_workload::WorkloadManifest::builder(id("missing-acquisition"), boot_image())
            .add_resource(kernel)
            .unwrap()
            .add_required_resource(resource_id("kernel"))
            .build()
            .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let error = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .acquire_replay_plan(&plan)
        .unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::MissingAcquisition {
            resource: resource_id("kernel"),
        }
    );
}

#[test]
fn in_memory_executor_rejects_missing_artifact() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
        acquisition,
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("missing-artifact"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();

    let error = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .acquire_replay_plan(&plan)
        .unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::MissingArtifact {
            resource: resource_id("kernel"),
            locator: "mem://kernel".to_string(),
        }
    );
}

#[test]
fn in_memory_executor_rejects_provenance_mismatch() {
    let expected = preloaded("mem://kernel", "bundle:v1");
    let actual = preloaded("mem://kernel", "bundle:v2");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
        expected.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("bad-provenance"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            actual.clone(),
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let error = executor.acquire_replay_plan(&plan).unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::ProvenanceMismatch {
            resource: resource_id("kernel"),
            expected: Box::new(expected),
            actual: Box::new(actual),
        }
    );
}

#[test]
fn in_memory_executor_rejects_duplicate_artifact_locators() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");

    let error = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition.clone(),
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition,
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::DuplicateArtifact {
            locator: "mem://kernel".to_string(),
        }
    );
}

#[test]
fn in_memory_executor_rejects_catalog_size_mismatch() {
    let acquisition = preloaded("mem://kernel", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
        acquisition.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("bad-size"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            acquisition,
            KERNEL_DIGEST,
            KERNEL_BYTES.len() + 1,
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let error = executor.acquire_replay_plan(&plan).unwrap_err();

    assert_eq!(
        error,
        WorkloadResourceAcquisitionError::SizeMismatch {
            resource: resource_id("kernel"),
            expected_bytes: KERNEL_BYTES.len() + 1,
            actual_bytes: KERNEL_BYTES.len(),
        }
    );
}

#[test]
fn in_memory_executor_preserves_suite_resource_provenance() {
    let kernel_acquisition = preloaded("mem://kernel", "bundle:v1");
    let trace_acquisition = preloaded("mem://trace", "bundle:v1");
    let kernel = resource(
        "kernel",
        WorkloadResourceKind::Kernel,
        KERNEL_DIGEST,
        "resources/kernel.elf",
        kernel_acquisition.clone(),
    );
    let trace = resource(
        "trace",
        WorkloadResourceKind::Input,
        TRACE_DIGEST,
        "resources/trace.bin",
        trace_acquisition.clone(),
    );
    let manifest = rem6_workload::WorkloadManifest::builder(id("suite-workload"), boot_image())
        .add_resource(kernel)
        .unwrap()
        .add_resource(trace)
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .add_required_resource(resource_id("trace"))
        .build()
        .unwrap();
    let suite = WorkloadSuite::builder(suite_id("artifact-suite"))
        .add_manifest(manifest.clone())
        .unwrap()
        .build()
        .unwrap();
    let plan = WorkloadSuiteReplayPlan::from_suite(&suite).unwrap();
    let executor = WorkloadInMemoryResourceAcquisitionExecutor::new()
        .add_artifact(WorkloadResourceArtifact::new(
            trace_acquisition,
            TRACE_DIGEST,
            TRACE_BYTES.len(),
            TRACE_BYTES.to_vec(),
        ))
        .unwrap()
        .add_artifact(WorkloadResourceArtifact::new(
            kernel_acquisition,
            KERNEL_DIGEST,
            KERNEL_BYTES.len(),
            KERNEL_BYTES.to_vec(),
        ))
        .unwrap();

    let records = executor.acquire_suite_replay_plan(&plan).unwrap();

    assert_eq!(records.len(), 2);
    assert_eq!(records[0].workload_id(), manifest.id());
    assert_eq!(records[0].manifest_identity(), manifest.identity());
    assert_eq!(records[0].acquired().resource(), &resource_id("kernel"));
    assert_eq!(records[1].workload_id(), manifest.id());
    assert_eq!(records[1].manifest_identity(), manifest.identity());
    assert_eq!(records[1].acquired().resource(), &resource_id("trace"));
}
