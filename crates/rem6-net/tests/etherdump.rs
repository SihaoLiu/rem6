use rem6_net::{EthernetPacket, EthernetPcapDump, NetworkError};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

fn le_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes.try_into().unwrap())
}

fn le_u16(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes.try_into().unwrap())
}

#[test]
fn ethernet_pcap_dump_writes_global_header_and_capped_packet_records() {
    let mut dump = EthernetPcapDump::new(4, 1_000_000).unwrap();

    let record = dump
        .capture(&packet(&[0, 1, 2, 3, 4, 5]), 1_234_567)
        .unwrap();

    assert_eq!(record.sequence(), 0);
    assert_eq!(record.tick(), 1_234_567);
    assert_eq!(record.timestamp_seconds(), 1);
    assert_eq!(record.timestamp_microseconds(), 234_567);
    assert_eq!(record.captured_len(), 4);
    assert_eq!(record.original_len(), 6);
    assert_eq!(record.captured_payload(), &[0, 1, 2, 3]);

    let image = dump.pcap_bytes();
    assert_eq!(image.len(), 24 + 16 + 4);
    assert_eq!(&image[0..4], &[0xd4, 0xc3, 0xb2, 0xa1]);
    assert_eq!(le_u16(&image[4..6]), 2);
    assert_eq!(le_u16(&image[6..8]), 4);
    assert_eq!(le_i32(&image[8..12]), 0);
    assert_eq!(le_u32(&image[12..16]), 0);
    assert_eq!(le_u32(&image[16..20]), 4);
    assert_eq!(le_u32(&image[20..24]), 1);
    assert_eq!(le_u32(&image[24..28]), 1);
    assert_eq!(le_u32(&image[28..32]), 234_567);
    assert_eq!(le_u32(&image[32..36]), 4);
    assert_eq!(le_u32(&image[36..40]), 6);
    assert_eq!(&image[40..44], &[0, 1, 2, 3]);
}

#[test]
fn ethernet_pcap_dump_keeps_record_order_and_full_packet_lengths() {
    let mut dump = EthernetPcapDump::new(96, 1_000_000).unwrap();

    let first = dump.capture(&packet(&[0xaa, 0xbb]), 2).unwrap();
    let second = dump.capture(&packet(&[0xcc, 0xdd, 0xee]), 3).unwrap();

    assert_eq!(first.sequence(), 0);
    assert_eq!(second.sequence(), 1);
    assert_eq!(dump.record_count(), 2);
    assert_eq!(dump.records()[0].captured_payload(), &[0xaa, 0xbb]);
    assert_eq!(dump.records()[1].captured_payload(), &[0xcc, 0xdd, 0xee]);
    assert_eq!(dump.records()[1].original_len(), 3);
}

#[test]
fn ethernet_pcap_dump_snapshot_restores_records_and_sequence() {
    let mut dump = EthernetPcapDump::new(8, 1_000_000).unwrap();
    dump.capture(&packet(&[1, 2]), 10).unwrap();
    let snapshot = dump.snapshot();

    dump.capture(&packet(&[9, 9, 9]), 20).unwrap();
    assert_eq!(dump.record_count(), 2);

    dump.restore(&snapshot).unwrap();
    assert_eq!(dump.record_count(), 1);
    let restored = dump.capture(&packet(&[3]), 30).unwrap();
    assert_eq!(restored.sequence(), 1);

    let image = dump.pcap_bytes();
    assert_eq!(le_u32(&image[24..28]), 0);
    assert_eq!(le_u32(&image[28..32]), 10);
    assert_eq!(le_u32(&image[32..36]), 2);
    assert_eq!(le_u32(&image[36..40]), 2);
    assert_eq!(&image[40..42], &[1, 2]);
    assert_eq!(le_u32(&image[42..46]), 0);
    assert_eq!(le_u32(&image[46..50]), 30);
    assert_eq!(le_u32(&image[50..54]), 1);
    assert_eq!(le_u32(&image[54..58]), 1);
    assert_eq!(&image[58..59], &[3]);
}

#[test]
fn ethernet_pcap_dump_rejects_invalid_config_and_unrepresentable_timestamps() {
    assert!(matches!(
        EthernetPcapDump::new(0, 1_000_000),
        Err(NetworkError::InvalidEthernetPcapMaxCaptureBytes {
            max_capture_bytes: 0
        })
    ));
    assert!(matches!(
        EthernetPcapDump::new(96, 0),
        Err(NetworkError::InvalidEthernetPcapClock {
            ticks_per_second: 0
        })
    ));

    let mut dump = EthernetPcapDump::new(96, 1).unwrap();
    assert!(matches!(
        dump.capture(&packet(&[1]), (u32::MAX as u64) + 1),
        Err(NetworkError::EthernetPcapTimestampOverflow {
            tick,
            ticks_per_second: 1,
        }) if tick == (u32::MAX as u64) + 1
    ));
}
