use rem6_virtio::{
    VirtioPciBarIndex, VirtioPciCapabilityEntry, VirtioPciCapabilityKind,
    VirtioPciCapabilityOffset, VirtioPciNotifyCapabilityEntry,
};

fn bar(index: u8) -> VirtioPciBarIndex {
    VirtioPciBarIndex::new(index).unwrap()
}

#[test]
fn virtio_pci_capability_entry_exports_standard_vendor_bytes() {
    let common = VirtioPciCapabilityEntry::new(
        VirtioPciCapabilityOffset::new(0x70).unwrap(),
        Some(VirtioPciCapabilityOffset::new(0x80).unwrap()),
        VirtioPciCapabilityKind::CommonConfig,
        bar(2),
        0,
        0x1000_0004,
        0x40,
    )
    .unwrap();

    assert_eq!(VirtioPciCapabilityKind::CommonConfig.cfg_type(), 1);
    assert_eq!(
        common.offset(),
        VirtioPciCapabilityOffset::new(0x70).unwrap()
    );
    assert_eq!(
        common.next(),
        Some(VirtioPciCapabilityOffset::new(0x80).unwrap())
    );
    assert_eq!(common.kind(), VirtioPciCapabilityKind::CommonConfig);
    assert_eq!(common.bar(), bar(2));
    assert_eq!(common.id(), 0);
    assert_eq!(common.region_offset(), 0x1000_0004);
    assert_eq!(common.length(), 0x40);
    assert_eq!(
        common.bytes(),
        [
            0x09, 0x80, 0x10, 0x01, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x10, 0x40, 0x00,
            0x00, 0x00,
        ]
    );
}

#[test]
fn virtio_pci_notify_capability_entry_exports_multiplier_extension() {
    let notify = VirtioPciNotifyCapabilityEntry::new(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0x80).unwrap(),
            None,
            VirtioPciCapabilityKind::NotifyConfig,
            bar(4),
            2,
            0x200,
            0x100,
        )
        .unwrap(),
        0x20,
    )
    .unwrap();

    assert_eq!(notify.base().kind(), VirtioPciCapabilityKind::NotifyConfig);
    assert_eq!(notify.notify_off_multiplier(), 0x20);
    assert_eq!(
        notify.bytes(),
        [
            0x09, 0x00, 0x14, 0x02, 0x04, 0x02, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
        ]
    );
}

#[test]
fn virtio_pci_capability_entries_reject_invalid_layouts() {
    assert!(matches!(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0xf4).unwrap(),
            None,
            VirtioPciCapabilityKind::DeviceConfig,
            bar(1),
            0,
            0,
            4,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));
    assert!(matches!(
        VirtioPciCapabilityEntry::new(
            VirtioPciCapabilityOffset::new(0x40).unwrap(),
            None,
            VirtioPciCapabilityKind::DeviceConfig,
            bar(1),
            0,
            0,
            0,
        ),
        Err(error) if error.to_string().contains("zero length")
    ));
    assert!(matches!(
        VirtioPciNotifyCapabilityEntry::new(
            VirtioPciCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0x40).unwrap(),
                None,
                VirtioPciCapabilityKind::DeviceConfig,
                bar(1),
                0,
                0,
                4,
            )
            .unwrap(),
            0,
        ),
        Err(error) if error.to_string().contains("notify")
    ));
    assert!(matches!(
        VirtioPciNotifyCapabilityEntry::new(
            VirtioPciCapabilityEntry::new(
                VirtioPciCapabilityOffset::new(0xf0).unwrap(),
                None,
                VirtioPciCapabilityKind::NotifyConfig,
                bar(1),
                0,
                0,
                4,
            )
            .unwrap(),
            0,
        ),
        Err(error) if error.to_string().contains("configuration space")
    ));
}
