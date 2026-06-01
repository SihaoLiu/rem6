use rem6_memory::{
    AccessSize, Address, AddressDecoder, AddressDecoderCheckpointPayload, AddressInterleave,
    AddressMapRegion, AddressRange, AgentId, CacheLineLayout, MemoryError, MemoryRequest,
    MemoryRequestId, MemoryTargetId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(9), sequence)
}

fn read(sequence: u64, address: u64, bytes: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn address_decoder_maps_requests_to_ordered_regions() {
    let mut decoder = AddressDecoder::new();
    decoder
        .insert(
            MemoryTargetId::new(1),
            Address::new(0x0000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    decoder
        .insert(
            MemoryTargetId::new(2),
            Address::new(0x8000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();

    assert_eq!(decoder.region_count(), 2);
    assert_eq!(
        decoder.decode(Address::new(0x0000)).unwrap(),
        MemoryTargetId::new(1)
    );
    assert_eq!(
        decoder.decode(Address::new(0x8fff)).unwrap(),
        MemoryTargetId::new(2)
    );
    assert_eq!(
        decoder.decode_request(&read(1, 0x8080, 16)).unwrap(),
        MemoryTargetId::new(2)
    );
    assert_eq!(
        decoder.regions(),
        &[
            (
                MemoryTargetId::new(1),
                AddressMapRegion::new(
                    AddressRange::new(Address::new(0x0000), AccessSize::new(0x1000).unwrap())
                        .unwrap()
                ),
            ),
            (
                MemoryTargetId::new(2),
                AddressMapRegion::new(
                    AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
                        .unwrap()
                ),
            ),
        ]
    );
}

#[test]
fn address_decoder_rejects_overlapping_regions() {
    let mut decoder = AddressDecoder::new();
    decoder
        .insert(
            MemoryTargetId::new(1),
            Address::new(0x1000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();

    assert_eq!(
        decoder
            .insert(
                MemoryTargetId::new(2),
                Address::new(0x1800),
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap_err(),
        MemoryError::OverlappingAddressRegion {
            existing: AddressRange::new(Address::new(0x1000), AccessSize::new(0x1000).unwrap())
                .unwrap(),
            requested: AddressRange::new(Address::new(0x1800), AccessSize::new(0x100).unwrap())
                .unwrap(),
        }
    );
    assert_eq!(decoder.region_count(), 1);
}

#[test]
fn address_decoder_rejects_unmapped_and_cross_region_requests() {
    let mut decoder = AddressDecoder::new();
    decoder
        .insert(
            MemoryTargetId::new(1),
            Address::new(0x4000),
            AccessSize::new(0x100).unwrap(),
        )
        .unwrap();

    assert_eq!(
        decoder.decode(Address::new(0x5000)).unwrap_err(),
        MemoryError::UnmappedAddress {
            address: Address::new(0x5000),
        }
    );
    assert_eq!(
        decoder.decode_request(&read(2, 0x40f8, 16)).unwrap_err(),
        MemoryError::RequestCrossesAddressRegion {
            request: request_id(2),
            range: AddressRange::new(Address::new(0x40f8), AccessSize::new(16).unwrap()).unwrap(),
        }
    );
}

#[test]
fn address_decoder_routes_modulo_interleaved_three_channel_regions_with_offsets() {
    let base = AddressRange::new(Address::new(0x0000), AccessSize::new(0x300).unwrap()).unwrap();
    let granularity = AccessSize::new(64).unwrap();
    let mut decoder = AddressDecoder::new();

    for channel in 0..3 {
        let interleave = AddressInterleave::modulo(granularity, 3, channel).unwrap();
        decoder
            .insert_region(
                MemoryTargetId::new(channel + 1),
                AddressMapRegion::new(base)
                    .with_interleave(interleave)
                    .unwrap(),
            )
            .unwrap();
    }

    let first = decoder.decode_detail(Address::new(0x0000)).unwrap();
    assert_eq!(first.target(), MemoryTargetId::new(1));
    assert_eq!(first.offset(), 0);

    let second = decoder.decode_detail(Address::new(0x0040)).unwrap();
    assert_eq!(second.target(), MemoryTargetId::new(2));
    assert_eq!(second.offset(), 0);

    let third = decoder.decode_detail(Address::new(0x0080)).unwrap();
    assert_eq!(third.target(), MemoryTargetId::new(3));
    assert_eq!(third.offset(), 0);

    let next_first = decoder.decode_detail(Address::new(0x00c0)).unwrap();
    assert_eq!(next_first.target(), MemoryTargetId::new(1));
    assert_eq!(next_first.offset(), 64);

    assert_eq!(
        decoder.decode_request(&read(9, 0x00c8, 8)).unwrap(),
        MemoryTargetId::new(1)
    );
    assert_eq!(
        decoder.decode_request(&read(10, 0x0038, 16)).unwrap_err(),
        MemoryError::RequestCrossesAddressRegion {
            request: request_id(10),
            range: AddressRange::new(Address::new(0x0038), AccessSize::new(16).unwrap()).unwrap(),
        }
    );
}

#[test]
fn address_decoder_skips_sparse_holes_and_packs_offsets() {
    let base = AddressRange::new(Address::new(0x0000), AccessSize::new(0x500).unwrap()).unwrap();
    let hole = AddressRange::new(Address::new(0x0300), AccessSize::new(0x100).unwrap()).unwrap();
    let target = MemoryTargetId::new(8);
    let mut decoder = AddressDecoder::new();
    decoder
        .insert_region(
            target,
            AddressMapRegion::new(base).with_holes(vec![hole]).unwrap(),
        )
        .unwrap();

    let before_hole = decoder.decode_detail(Address::new(0x02ff)).unwrap();
    assert_eq!(before_hole.target(), target);
    assert_eq!(before_hole.offset(), 0x02ff);

    assert_eq!(
        decoder.decode(Address::new(0x0300)).unwrap_err(),
        MemoryError::UnmappedAddress {
            address: Address::new(0x0300),
        }
    );

    let after_hole = decoder.decode_detail(Address::new(0x0400)).unwrap();
    assert_eq!(after_hole.target(), target);
    assert_eq!(after_hole.offset(), 0x0300);

    assert_eq!(
        decoder.decode_request(&read(11, 0x02f8, 16)).unwrap_err(),
        MemoryError::RequestCrossesAddressRegion {
            request: request_id(11),
            range: AddressRange::new(Address::new(0x02f8), AccessSize::new(16).unwrap()).unwrap(),
        }
    );
}

#[test]
fn address_decoder_rejects_ambiguous_mapped_region_overlap() {
    let base = AddressRange::new(Address::new(0x0000), AccessSize::new(0x300).unwrap()).unwrap();
    let mut decoder = AddressDecoder::new();
    decoder
        .insert_region(
            MemoryTargetId::new(1),
            AddressMapRegion::new(base)
                .with_interleave(
                    AddressInterleave::modulo(AccessSize::new(64).unwrap(), 3, 0).unwrap(),
                )
                .unwrap(),
        )
        .unwrap();

    assert!(matches!(
        decoder
            .insert_region(
                MemoryTargetId::new(2),
                AddressMapRegion::new(base)
                    .with_interleave(
                        AddressInterleave::modulo(AccessSize::new(128).unwrap(), 3, 0).unwrap()
                    )
                    .unwrap(),
            )
            .unwrap_err(),
        MemoryError::OverlappingAddressRegion { .. }
    ));
}

#[test]
fn address_decoder_checkpoint_payload_round_trips_sparse_and_interleaved_regions() {
    let sparse_target = MemoryTargetId::new(8);
    let sparse_base =
        AddressRange::new(Address::new(0x0000), AccessSize::new(0x500).unwrap()).unwrap();
    let sparse_hole =
        AddressRange::new(Address::new(0x0300), AccessSize::new(0x100).unwrap()).unwrap();
    let interleaved_base =
        AddressRange::new(Address::new(0x1000), AccessSize::new(0x300).unwrap()).unwrap();
    let mut decoder = AddressDecoder::new();
    decoder
        .insert_region(
            sparse_target,
            AddressMapRegion::new(sparse_base)
                .with_holes(vec![sparse_hole])
                .unwrap(),
        )
        .unwrap();
    for channel in 0..3 {
        decoder
            .insert_region(
                MemoryTargetId::new(20 + channel),
                AddressMapRegion::new(interleaved_base)
                    .with_interleave(
                        AddressInterleave::modulo(AccessSize::new(64).unwrap(), 3, channel)
                            .unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
    }
    let snapshot = decoder.snapshot();
    let payload = AddressDecoderCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded = AddressDecoderCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = AddressDecoder::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        restored
            .decode_detail(Address::new(0x0400))
            .unwrap()
            .offset(),
        0x0300
    );
    assert_eq!(
        restored
            .decode_detail(Address::new(0x10c0))
            .unwrap()
            .target(),
        MemoryTargetId::new(20)
    );
    assert_eq!(
        restored
            .decode_detail(Address::new(0x1100))
            .unwrap()
            .target(),
        MemoryTargetId::new(21)
    );
}

#[test]
fn address_decoder_checkpoint_payload_rejects_overlapping_region_records() {
    let mut decoder = AddressDecoder::new();
    decoder
        .insert(
            MemoryTargetId::new(1),
            Address::new(0x1000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    let payload = AddressDecoderCheckpointPayload::from_snapshot(decoder.snapshot())
        .unwrap()
        .encode();
    let duplicate_payload = duplicate_first_decoder_checkpoint_region(payload);

    assert_eq!(
        AddressDecoderCheckpointPayload::decode(&duplicate_payload).unwrap_err(),
        MemoryError::OverlappingAddressRegion {
            existing: AddressRange::new(Address::new(0x1000), AccessSize::new(0x1000).unwrap())
                .unwrap(),
            requested: AddressRange::new(Address::new(0x1000), AccessSize::new(0x1000).unwrap())
                .unwrap(),
        }
    );
}

fn duplicate_first_decoder_checkpoint_region(mut payload: Vec<u8>) -> Vec<u8> {
    let region_count_offset = 8;
    payload[region_count_offset..region_count_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    let first_region_offset = 20;
    let region_record_bytes = 32;
    let first_region =
        payload[first_region_offset..first_region_offset + region_record_bytes].to_vec();
    payload.splice(first_region_offset..first_region_offset, first_region);
    payload
}
