use rem6_virtio::{
    VirtioConsoleConfig, VirtioConsoleDevice, VirtioError, VirtioQueueIndex, VirtioSplitDescriptor,
    VirtioSplitDescriptorChain, VirtioSplitUsedElement, VirtioSplitUsedRing,
    VIRTIO_CONSOLE_CONFIG_SIZE, VIRTIO_CONSOLE_DEVICE_ID, VIRTIO_CONSOLE_F_SIZE,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

#[test]
fn virtio_console_reports_gem5_identity_size_feature_and_default_config() {
    let device = VirtioConsoleDevice::new();

    assert_eq!(VIRTIO_CONSOLE_DEVICE_ID, 3);
    assert_eq!(VIRTIO_CONSOLE_CONFIG_SIZE, 4);
    assert_eq!(VIRTIO_CONSOLE_F_SIZE, 1);
    assert_eq!(device.feature_pages(), vec![(0, VIRTIO_CONSOLE_F_SIZE)]);
    assert_eq!(device.config_size(), VIRTIO_CONSOLE_CONFIG_SIZE);
    assert_eq!(device.config(), VirtioConsoleConfig::new(80, 24).unwrap());
    assert_eq!(device.config_bytes(), [80, 0, 24, 0]);
}

#[test]
fn virtio_console_transmit_chain_copies_guest_bytes_to_terminal_and_uses_zero_length() {
    let device = VirtioConsoleDevice::new();
    let chain = VirtioSplitDescriptorChain::new(
        5,
        [
            VirtioSplitDescriptor::device_readable(5, b"hel".to_vec(), Some(6)),
            VirtioSplitDescriptor::device_readable(6, b"lo".to_vec(), None),
        ],
    )
    .unwrap();
    let decoded = chain.decode_console_transmit_request(queue(1)).unwrap();

    assert_eq!(decoded.request().queue(), queue(1));
    assert_eq!(decoded.request().data(), b"hello");

    let completion = device.transmit_at(33, decoded.request().clone()).unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(8, 4).unwrap();
    let writeback = used_ring
        .complete_console_transmit(&decoded, &completion)
        .unwrap();

    assert_eq!(device.guest_output(), b"hello");
    assert_eq!(completion.tick(), 33);
    assert_eq!(completion.used_length(), 0);
    assert!(writeback.data_writes().is_empty());
    assert_eq!(writeback.used_slot(), 4);
    assert_eq!(writeback.used_index(), 5);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(5, 0));
}

#[test]
fn virtio_console_receive_chain_scatters_host_bytes_and_reports_actual_length() {
    let device = VirtioConsoleDevice::new();
    device.push_host_input(b"abcdef".to_vec());
    let chain = VirtioSplitDescriptorChain::new(
        2,
        [
            VirtioSplitDescriptor::device_writable(2, 3, Some(3)),
            VirtioSplitDescriptor::device_writable(3, 4, None),
        ],
    )
    .unwrap();
    let decoded = chain.decode_console_receive_request(queue(0)).unwrap();

    assert_eq!(decoded.request().queue(), queue(0));
    assert_eq!(decoded.request().capacity(), 7);

    let completion = device.receive_at(44, decoded.request().clone()).unwrap();
    let mut used_ring = VirtioSplitUsedRing::new(8, 7).unwrap();
    let writeback = used_ring
        .complete_console_receive(&decoded, &completion)
        .unwrap();

    assert_eq!(completion.bytes(), b"abcdef");
    assert_eq!(completion.used_length(), 6);
    assert!(device.pending_host_input().is_empty());
    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 2);
    assert_eq!(writeback.data_writes()[0].bytes(), b"abc");
    assert_eq!(writeback.data_writes()[1].descriptor(), 3);
    assert_eq!(writeback.data_writes()[1].bytes(), b"def");
    assert_eq!(writeback.used_slot(), 7);
    assert_eq!(writeback.used_index(), 8);
    assert_eq!(writeback.used_element(), VirtioSplitUsedElement::new(2, 6));
}

#[test]
fn virtio_console_rejects_gem5_panic_paths_as_typed_errors() {
    assert!(matches!(
        VirtioConsoleConfig::new(0, 24),
        Err(VirtioError::InvalidConsoleSize { cols: 0, rows: 24 })
    ));

    let receive_readable = VirtioSplitDescriptorChain::new(
        0,
        [VirtioSplitDescriptor::device_readable(
            0,
            b"x".to_vec(),
            None,
        )],
    )
    .unwrap();
    assert!(matches!(
        receive_readable.decode_console_receive_request(queue(0)),
        Err(VirtioError::InvalidVirtioConsoleReceiveDescriptor { index: 0 })
    ));

    let receive_empty =
        VirtioSplitDescriptorChain::new(0, [VirtioSplitDescriptor::device_writable(0, 0, None)])
            .unwrap();
    assert!(matches!(
        receive_empty.decode_console_receive_request(queue(0)),
        Err(VirtioError::MissingVirtioConsoleReceiveDescriptor)
    ));

    let transmit_writable =
        VirtioSplitDescriptorChain::new(4, [VirtioSplitDescriptor::device_writable(4, 2, None)])
            .unwrap();
    assert!(matches!(
        transmit_writable.decode_console_transmit_request(queue(1)),
        Err(VirtioError::InvalidVirtioConsoleTransmitDescriptor { index: 4 })
    ));
}
