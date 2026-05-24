use rem6_boot::BootImage;
use rem6_memory::{AccessSize, Address};
use rem6_workload::{
    WorkloadError, WorkloadId, WorkloadLinuxBootHandoff, WorkloadLinuxInitrd, WorkloadManifest,
    WorkloadReplayPlan, WorkloadResolvedResources, WorkloadResource, WorkloadResourceId,
    WorkloadResourceKind, WorkloadResourcePayload,
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
        .add_segment(Address::new(0x8010), vec![0x73, 0x00, 0x00, 0x00])
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

fn disk_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("disk"),
        WorkloadResourceKind::DiskImage,
        "sha256:disk",
        "resources/rootfs.img",
    )
    .unwrap()
}

fn initrd_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("initrd"),
        WorkloadResourceKind::Initrd,
        "sha256:initrd",
        "resources/initrd.cpio",
    )
    .unwrap()
}

fn device_tree_resource() -> WorkloadResource {
    WorkloadResource::new(
        resource_id("dtb"),
        WorkloadResourceKind::DeviceTree,
        "sha256:dtb",
        "resources/riscv.dtb",
    )
    .unwrap()
}

fn linux_handoff() -> WorkloadLinuxBootHandoff {
    WorkloadLinuxBootHandoff::new(Address::new(0x87e0_0000))
        .with_device_tree_resource(resource_id("dtb"))
        .with_bootargs("console=ttyS0 root=/dev/vda")
        .with_initrd(
            WorkloadLinuxInitrd::new(
                resource_id("initrd"),
                Address::new(0x8800_0000),
                AccessSize::new(8).unwrap(),
            )
            .unwrap(),
        )
}

fn linux_manifest() -> WorkloadManifest {
    WorkloadManifest::builder(id("linux-handoff"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_linux_boot_handoff(linux_handoff())
        .build()
        .unwrap()
}

#[test]
fn workload_manifest_records_linux_boot_handoff_resources() {
    let manifest = linux_manifest();
    let plan = WorkloadReplayPlan::from_manifest(&manifest).unwrap();
    let handoff = manifest.linux_boot_handoff().unwrap();
    let initrd = handoff.initrd().unwrap();

    assert_eq!(handoff.dtb_addr(), Address::new(0x87e0_0000));
    assert_eq!(handoff.device_tree_resource(), Some(&resource_id("dtb")));
    assert_eq!(handoff.bootargs(), Some("console=ttyS0 root=/dev/vda"));
    assert_eq!(initrd.resource(), &resource_id("initrd"));
    assert_eq!(initrd.start(), Address::new(0x8800_0000));
    assert_eq!(initrd.end(), Address::new(0x8800_0008));
    assert_eq!(
        manifest.required_resources(),
        &[
            resource_id("dtb"),
            resource_id("initrd"),
            resource_id("kernel")
        ]
    );
    assert!(plan
        .required_resources()
        .iter()
        .any(|resource| resource.id() == &resource_id("initrd")
            && resource.kind() == WorkloadResourceKind::Initrd));
    assert!(plan
        .required_resources()
        .iter()
        .any(|resource| resource.id() == &resource_id("dtb")
            && resource.kind() == WorkloadResourceKind::DeviceTree));
    assert_eq!(plan.linux_boot_handoff(), Some(handoff));
}

#[test]
fn workload_resolved_resources_validate_linux_initrd_payload() {
    let manifest = linux_manifest();
    let resources = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13, 0x00])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0, 0xd1])
                .unwrap(),
            WorkloadResourcePayload::new(
                resource_id("initrd"),
                "sha256:initrd",
                vec![0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7],
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let handoff = manifest.linux_boot_handoff().unwrap();

    assert_eq!(resources.manifest_identity(), manifest.identity());
    assert_eq!(
        resources.payload_data(&resource_id("initrd")).unwrap(),
        &[0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7]
    );
    assert_eq!(
        resources.linux_device_tree_data(handoff).unwrap(),
        &[0xd0, 0xd1]
    );
    assert_eq!(
        resources.linux_initrd_data(handoff).unwrap(),
        &[0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7]
    );
}

#[test]
fn workload_resolved_resources_reject_bad_required_payloads() {
    let manifest = linux_manifest();

    let missing_initrd = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
        ],
    )
    .unwrap_err();
    assert_eq!(
        missing_initrd,
        WorkloadError::MissingResourcePayload {
            resource: resource_id("initrd"),
        }
    );

    let wrong_digest = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:other", vec![0; 8])
                .unwrap(),
        ],
    )
    .unwrap_err();
    assert_eq!(
        wrong_digest,
        WorkloadError::ResourcePayloadDigestMismatch {
            resource: resource_id("initrd"),
            expected: "sha256:initrd".to_string(),
            actual: "sha256:other".to_string(),
        }
    );

    let wrong_size = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![0; 7])
                .unwrap(),
        ],
    )
    .unwrap_err();
    assert_eq!(
        wrong_size,
        WorkloadError::ResourcePayloadSizeMismatch {
            resource: resource_id("initrd"),
            expected_bytes: 8,
            actual_bytes: 7,
        }
    );

    let unexpected = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![0; 8])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("extra"), "sha256:extra", vec![0xee]).unwrap(),
        ],
    )
    .unwrap_err();
    assert_eq!(
        unexpected,
        WorkloadError::UnexpectedResourcePayload {
            resource: resource_id("extra"),
        }
    );

    let duplicate = WorkloadResolvedResources::from_manifest(
        &manifest,
        [
            WorkloadResourcePayload::new(resource_id("kernel"), "sha256:kernel", vec![0x13])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("dtb"), "sha256:dtb", vec![0xd0]).unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![0; 8])
                .unwrap(),
            WorkloadResourcePayload::new(resource_id("initrd"), "sha256:initrd", vec![1; 8])
                .unwrap(),
        ],
    )
    .unwrap_err();
    assert_eq!(
        duplicate,
        WorkloadError::DuplicateResourcePayload {
            resource: resource_id("initrd"),
        }
    );
}

#[test]
fn workload_manifest_identity_changes_with_linux_boot_handoff() {
    let plain = WorkloadManifest::builder(id("linux-handoff-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .build()
        .unwrap();
    let with_handoff = WorkloadManifest::builder(id("linux-handoff-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_linux_boot_handoff(linux_handoff())
        .build()
        .unwrap();
    let different_bootargs = WorkloadManifest::builder(id("linux-handoff-identity"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(initrd_resource())
        .unwrap()
        .add_required_resource(resource_id("kernel"))
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x87e0_0000))
                .with_device_tree_resource(resource_id("dtb"))
                .with_bootargs("console=ttyS1 root=/dev/vda")
                .with_initrd(
                    WorkloadLinuxInitrd::new(
                        resource_id("initrd"),
                        Address::new(0x8800_0000),
                        AccessSize::new(0x2000).unwrap(),
                    )
                    .unwrap(),
                ),
        )
        .build()
        .unwrap();

    assert_ne!(plain.identity(), with_handoff.identity());
    assert_ne!(with_handoff.identity(), different_bootargs.identity());
}

#[test]
fn workload_linux_boot_handoff_rejects_missing_or_wrong_initrd_resource() {
    let missing = WorkloadManifest::builder(id("missing-initrd"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .with_linux_boot_handoff(linux_handoff())
        .build()
        .unwrap_err();
    assert_eq!(
        missing,
        WorkloadError::MissingRequiredResource {
            resource: resource_id("initrd"),
        }
    );

    let wrong_kind = WorkloadManifest::builder(id("wrong-initrd-kind"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(device_tree_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x87e0_0000)).with_initrd(
                WorkloadLinuxInitrd::new(
                    resource_id("disk"),
                    Address::new(0x8800_0000),
                    AccessSize::new(0x2000).unwrap(),
                )
                .unwrap(),
            ),
        )
        .build()
        .unwrap_err();
    assert_eq!(
        wrong_kind,
        WorkloadError::ResourceKindMismatch {
            resource: resource_id("disk"),
            expected: WorkloadResourceKind::Initrd,
            actual: WorkloadResourceKind::DiskImage,
        }
    );
}

#[test]
fn workload_linux_boot_handoff_rejects_missing_or_wrong_device_tree_resource() {
    let missing = WorkloadManifest::builder(id("missing-dtb"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x87e0_0000))
                .with_device_tree_resource(resource_id("dtb")),
        )
        .build()
        .unwrap_err();
    assert_eq!(
        missing,
        WorkloadError::MissingRequiredResource {
            resource: resource_id("dtb"),
        }
    );

    let wrong_kind = WorkloadManifest::builder(id("wrong-dtb-kind"), boot_image())
        .add_resource(kernel_resource())
        .unwrap()
        .add_resource(disk_resource())
        .unwrap()
        .with_linux_boot_handoff(
            WorkloadLinuxBootHandoff::new(Address::new(0x87e0_0000))
                .with_device_tree_resource(resource_id("disk")),
        )
        .build()
        .unwrap_err();
    assert_eq!(
        wrong_kind,
        WorkloadError::ResourceKindMismatch {
            resource: resource_id("disk"),
            expected: WorkloadResourceKind::DeviceTree,
            actual: WorkloadResourceKind::DiskImage,
        }
    );
}
