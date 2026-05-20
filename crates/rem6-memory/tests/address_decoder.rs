use rem6_memory::{
    AccessSize, Address, AddressDecoder, AddressRange, AgentId, CacheLineLayout, MemoryError,
    MemoryRequest, MemoryRequestId, MemoryTargetId,
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
                AddressRange::new(Address::new(0x0000), AccessSize::new(0x1000).unwrap()).unwrap(),
            ),
            (
                MemoryTargetId::new(2),
                AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap()).unwrap(),
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
