use rem6_virtio::{
    VirtioBlockConfigSpec, VirtioBlockDecodedRequest, VirtioBlockDevice, VirtioBlockMemoryBackend,
    VirtioBlockRequestId, VirtioBlockRequestKind, VirtioQueueIndex, VirtioSplitDescriptor,
    VirtioSplitDescriptorChain, VirtioSplitUsedElement, VirtioSplitUsedRing,
    VIRTIO_BLOCK_SECTOR_SIZE, VIRTIO_BLOCK_S_OK, VIRTIO_BLOCK_T_FLUSH, VIRTIO_BLOCK_T_GET_ID,
    VIRTIO_BLOCK_T_IN, VIRTIO_BLOCK_T_OUT,
};

fn queue(index: u16) -> VirtioQueueIndex {
    VirtioQueueIndex::new(index).unwrap()
}

fn header(raw_type: u32, sector: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(raw_type.to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(sector.to_le_bytes());
    bytes
}

fn sector(byte: u8) -> Vec<u8> {
    vec![byte; VIRTIO_BLOCK_SECTOR_SIZE as usize]
}

fn decoded(chain: VirtioSplitDescriptorChain) -> VirtioBlockDecodedRequest {
    chain.decode_block_request(queue(3)).unwrap()
}

#[test]
fn virtio_split_descriptor_chain_decodes_block_requests() {
    let write = decoded(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_OUT, 7), Some(1)),
                VirtioSplitDescriptor::device_readable(1, sector(0xab), Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(write.request().id(), VirtioBlockRequestId::new(0));
    assert_eq!(write.request().queue(), queue(3));
    assert_eq!(write.request().sector(), 7);
    assert_eq!(write.status_descriptor(), 2);
    assert_eq!(write.writable_data_bytes(), 0);
    assert_eq!(
        write.request().kind(),
        &VirtioBlockRequestKind::Write { data: sector(0xab) }
    );

    let read = decoded(
        VirtioSplitDescriptorChain::new(
            4,
            vec![
                VirtioSplitDescriptor::device_readable(4, header(VIRTIO_BLOCK_T_IN, 9), Some(5)),
                VirtioSplitDescriptor::device_writable(5, VIRTIO_BLOCK_SECTOR_SIZE as u32, Some(6)),
                VirtioSplitDescriptor::device_writable(6, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(read.request().id(), VirtioBlockRequestId::new(4));
    assert_eq!(read.request().sector(), 9);
    assert_eq!(read.status_descriptor(), 6);
    assert_eq!(read.writable_data_bytes(), VIRTIO_BLOCK_SECTOR_SIZE);
    assert_eq!(
        read.request().kind(),
        &VirtioBlockRequestKind::Read {
            bytes: VIRTIO_BLOCK_SECTOR_SIZE
        }
    );

    let flush = decoded(
        VirtioSplitDescriptorChain::new(
            8,
            vec![
                VirtioSplitDescriptor::device_readable(8, header(VIRTIO_BLOCK_T_FLUSH, 0), Some(9)),
                VirtioSplitDescriptor::device_writable(9, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(flush.request().kind(), &VirtioBlockRequestKind::Flush);
    assert_eq!(flush.status_descriptor(), 9);
    assert_eq!(flush.writable_data_bytes(), 0);

    let get_id = decoded(
        VirtioSplitDescriptorChain::new(
            11,
            vec![
                VirtioSplitDescriptor::device_readable(
                    11,
                    header(VIRTIO_BLOCK_T_GET_ID, 0),
                    Some(12),
                ),
                VirtioSplitDescriptor::device_writable(12, 20, Some(13)),
                VirtioSplitDescriptor::device_writable(13, 1, None),
            ],
        )
        .unwrap(),
    );
    assert_eq!(get_id.request().kind(), &VirtioBlockRequestKind::GetId);
    assert_eq!(get_id.status_descriptor(), 13);
    assert_eq!(get_id.writable_data_bytes(), 20);
}

#[test]
fn virtio_split_descriptor_chain_rejects_bad_block_shapes() {
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_IN, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, 1, Some(0)),
            ],
        ),
        Err(error) if error.to_string().contains("loop")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![VirtioSplitDescriptor::device_readable(0, vec![0; 8], None)],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("header")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![VirtioSplitDescriptor::device_readable(
                0,
                header(VIRTIO_BLOCK_T_IN, 0),
                None,
            )],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("status")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_IN, 0), Some(1)),
                VirtioSplitDescriptor::device_readable(1, sector(0x11), Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("writable")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_OUT, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, VIRTIO_BLOCK_SECTOR_SIZE as u32, Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("readable")
    ));
    assert!(matches!(
        VirtioSplitDescriptorChain::new(
            0,
            vec![
                VirtioSplitDescriptor::device_readable(0, header(VIRTIO_BLOCK_T_GET_ID, 0), Some(1)),
                VirtioSplitDescriptor::device_writable(1, 19, Some(2)),
                VirtioSplitDescriptor::device_writable(2, 1, None),
            ],
        )
        .unwrap()
        .decode_block_request(queue(0)),
        Err(error) if error.to_string().contains("device id")
    ));
}

#[test]
fn virtio_split_used_ring_records_block_completion_writeback() {
    let data = sector(0x44);
    let backend = VirtioBlockMemoryBackend::from_bytes(data.clone()).unwrap();
    let device = VirtioBlockDevice::new(VirtioBlockConfigSpec::new(1), backend.clone()).unwrap();
    let decoded = decoded(
        VirtioSplitDescriptorChain::new(
            14,
            vec![
                VirtioSplitDescriptor::device_readable(14, header(VIRTIO_BLOCK_T_IN, 0), Some(15)),
                VirtioSplitDescriptor::device_writable(15, 300, Some(16)),
                VirtioSplitDescriptor::device_writable(16, 212, Some(17)),
                VirtioSplitDescriptor::device_writable(17, 1, None),
            ],
        )
        .unwrap(),
    );
    let completion = device.execute_at(41, decoded.request().clone()).unwrap();

    let mut used_ring = VirtioSplitUsedRing::new(4, 0xfffe).unwrap();
    let writeback = used_ring
        .complete_block_request(&decoded, &completion)
        .unwrap();

    assert_eq!(writeback.status_write().descriptor(), 17);
    assert_eq!(writeback.status_write().offset(), 0);
    assert_eq!(writeback.status_write().bytes(), &[VIRTIO_BLOCK_S_OK]);
    assert_eq!(writeback.data_writes().len(), 2);
    assert_eq!(writeback.data_writes()[0].descriptor(), 15);
    assert_eq!(writeback.data_writes()[0].bytes(), &data[..300]);
    assert_eq!(writeback.data_writes()[1].descriptor(), 16);
    assert_eq!(writeback.data_writes()[1].bytes(), &data[300..]);
    assert_eq!(writeback.used_slot(), 2);
    assert_eq!(writeback.used_index(), 0xffff);
    assert_eq!(
        writeback.used_element(),
        VirtioSplitUsedElement::new(14, 529)
    );
    assert_eq!(used_ring.index(), 0xffff);
    assert_eq!(
        used_ring.entry(2),
        Some(&VirtioSplitUsedElement::new(14, 529))
    );
    assert_eq!(
        writeback.used_element().to_le_bytes(),
        [14, 0, 0, 0, 17, 2, 0, 0]
    );
}
