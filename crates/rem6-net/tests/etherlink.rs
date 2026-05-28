use rem6_net::{
    EthernetFullDuplexLink, EthernetLinkDelayVariation, EthernetLinkDirection, EthernetLinkTiming,
    EthernetPacket, NetworkError,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn timed_packet(bytes: &[u8], wire_length_bytes: u64) -> EthernetPacket {
    packet(bytes)
        .with_wire_length_bytes(wire_length_bytes)
        .unwrap()
}

#[test]
fn ethernet_link_uses_wire_length_for_serialization_and_delivery() {
    let timing = EthernetLinkTiming::new(2, 5).unwrap();
    let mut link = EthernetFullDuplexLink::new(timing);

    let transmission = link
        .transmit(
            EthernetLinkDirection::LeftToRight,
            timed_packet(&[0xaa, 0xbb, 0xcc, 0xdd], 8),
            10,
        )
        .unwrap();

    assert_eq!(transmission.sequence(), 0);
    assert_eq!(transmission.direction(), EthernetLinkDirection::LeftToRight);
    assert_eq!(transmission.request_tick(), 10);
    assert_eq!(transmission.serialization_done_tick(), 27);
    assert_eq!(transmission.delivery_tick(), 32);
    assert!(link.is_busy(EthernetLinkDirection::LeftToRight, 26));
    assert!(!link.is_busy(EthernetLinkDirection::LeftToRight, 27));
    assert_eq!(link.pending_delivery_count(), 1);

    assert!(link.drain_ready_deliveries(31).is_empty());
    let ready = link.drain_ready_deliveries(32);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].sequence(), 0);
    assert_eq!(ready[0].packet().payload(), &[0xaa, 0xbb, 0xcc, 0xdd]);
    assert_eq!(link.pending_delivery_count(), 0);
}

#[test]
fn full_duplex_link_keeps_directions_independent_and_reports_busy() {
    let timing = EthernetLinkTiming::new(3, 2).unwrap();
    let mut link = EthernetFullDuplexLink::new(timing);

    let left_to_right = link
        .transmit(
            EthernetLinkDirection::LeftToRight,
            timed_packet(&[1, 2, 3, 4], 4),
            0,
        )
        .unwrap();
    let right_to_left = link
        .transmit(
            EthernetLinkDirection::RightToLeft,
            timed_packet(&[5, 6], 2),
            0,
        )
        .unwrap();

    assert_eq!(left_to_right.serialization_done_tick(), 13);
    assert_eq!(left_to_right.delivery_tick(), 15);
    assert_eq!(right_to_left.serialization_done_tick(), 7);
    assert_eq!(right_to_left.delivery_tick(), 9);

    assert!(matches!(
        link.transmit(EthernetLinkDirection::LeftToRight, packet(&[7]), 12),
        Err(NetworkError::EthernetLinkBusy {
            direction: EthernetLinkDirection::LeftToRight,
            request_tick: 12,
            busy_until_tick: 13,
        })
    ));

    let later = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[8]), 13)
        .unwrap();
    assert_eq!(later.sequence(), 2);

    let ready = link.drain_ready_deliveries(15);
    let sequences: Vec<u64> = ready.iter().map(|delivery| delivery.sequence()).collect();
    assert_eq!(sequences, vec![1, 0]);
    assert_eq!(ready[0].direction(), EthernetLinkDirection::RightToLeft);
    assert_eq!(ready[1].direction(), EthernetLinkDirection::LeftToRight);
    assert_eq!(link.pending_delivery_count(), 1);
}

#[test]
fn ethernet_link_applies_reproducible_delay_variation_per_direction() {
    let timing = EthernetLinkTiming::new(1, 4).unwrap();
    let variation = EthernetLinkDelayVariation::new(3, vec![2, 0]).unwrap();
    let mut link = EthernetFullDuplexLink::new_with_delay_variation(timing, variation);

    let left_first = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[1, 2, 3]), 0)
        .unwrap();
    let right_first = link
        .transmit(EthernetLinkDirection::RightToLeft, packet(&[4, 5]), 0)
        .unwrap();

    assert_eq!(left_first.serialization_done_tick(), 4);
    assert_eq!(left_first.delay_variation_ticks(), 2);
    assert_eq!(left_first.transmit_done_tick(), 6);
    assert_eq!(left_first.delivery_tick(), 10);
    assert_eq!(right_first.serialization_done_tick(), 3);
    assert_eq!(right_first.delay_variation_ticks(), 2);
    assert_eq!(right_first.transmit_done_tick(), 5);
    assert_eq!(right_first.delivery_tick(), 9);

    assert!(link.is_busy(EthernetLinkDirection::LeftToRight, 5));
    assert!(!link.is_busy(EthernetLinkDirection::LeftToRight, 6));

    let left_second = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[6]), 6)
        .unwrap();
    assert_eq!(left_second.sequence(), 2);
    assert_eq!(left_second.serialization_done_tick(), 8);
    assert_eq!(left_second.delay_variation_ticks(), 0);
    assert_eq!(left_second.transmit_done_tick(), 8);
    assert_eq!(left_second.delivery_tick(), 12);
}

#[test]
fn ethernet_link_snapshot_restores_delay_variation_position() {
    let timing = EthernetLinkTiming::new(1, 1).unwrap();
    let variation = EthernetLinkDelayVariation::new(3, vec![1, 3]).unwrap();
    let mut link = EthernetFullDuplexLink::new_with_delay_variation(timing, variation);

    let first = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[1]), 0)
        .unwrap();
    assert_eq!(first.delay_variation_ticks(), 1);
    let snapshot = link.snapshot();

    let after_snapshot = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[2]), 3)
        .unwrap();
    assert_eq!(after_snapshot.delay_variation_ticks(), 3);
    assert_eq!(after_snapshot.transmit_done_tick(), 8);

    link.restore(&snapshot).unwrap();
    let restored = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[3]), 3)
        .unwrap();
    assert_eq!(restored.sequence(), after_snapshot.sequence());
    assert_eq!(
        restored.delay_variation_ticks(),
        after_snapshot.delay_variation_ticks()
    );
    assert_eq!(
        restored.transmit_done_tick(),
        after_snapshot.transmit_done_tick()
    );
}

#[test]
fn ethernet_link_snapshot_restores_pending_deliveries_and_counters() {
    let timing = EthernetLinkTiming::new(1, 4).unwrap();
    let mut link = EthernetFullDuplexLink::new(timing);

    let first = link
        .transmit(EthernetLinkDirection::LeftToRight, packet(&[1, 2, 3]), 3)
        .unwrap();
    let second = link
        .transmit(EthernetLinkDirection::RightToLeft, packet(&[4, 5]), 3)
        .unwrap();
    let snapshot = link.snapshot();

    assert_eq!(snapshot.next_sequence(), 2);
    assert_eq!(snapshot.pending_delivery_count(), 2);
    assert_eq!(
        snapshot.busy_until_tick(EthernetLinkDirection::LeftToRight),
        Some(first.serialization_done_tick())
    );
    assert_eq!(
        snapshot.busy_until_tick(EthernetLinkDirection::RightToLeft),
        Some(second.serialization_done_tick())
    );

    assert_eq!(link.drain_ready_deliveries(11).len(), 2);
    link.transmit(EthernetLinkDirection::LeftToRight, packet(&[9]), 12)
        .unwrap();

    link.restore(&snapshot).unwrap();
    assert_eq!(link.pending_delivery_count(), 2);
    assert_eq!(link.next_sequence(), 2);
    assert!(link.is_busy(EthernetLinkDirection::LeftToRight, 6));

    let restored = link.drain_ready_deliveries(11);
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].sequence(), second.sequence());
    assert_eq!(restored[0].packet().payload(), &[4, 5]);
    assert_eq!(restored[1].sequence(), first.sequence());
    assert_eq!(restored[1].packet().payload(), &[1, 2, 3]);
}

#[test]
fn ethernet_link_rejects_invalid_timing_and_overflow() {
    assert!(matches!(
        EthernetLinkTiming::new(0, 1),
        Err(NetworkError::InvalidEthernetLinkRate { ticks_per_byte: 0 })
    ));
    assert!(matches!(
        EthernetLinkDelayVariation::new(2, vec![0, 3]),
        Err(NetworkError::InvalidEthernetLinkDelayVariation {
            max_delay_ticks: 2,
            delay_ticks: 3,
        })
    ));

    let timing = EthernetLinkTiming::new(2, 1).unwrap();
    let mut link = EthernetFullDuplexLink::new(timing);
    let too_large = packet(&[1]).with_wire_length_bytes(u64::MAX).unwrap();

    assert!(matches!(
        link.transmit(EthernetLinkDirection::LeftToRight, too_large, 0),
        Err(NetworkError::EthernetLinkTimingOverflow {
            request_tick: 0,
            wire_length_bytes: u64::MAX,
            ticks_per_byte: 2,
            link_delay_ticks: 1,
        })
    ));
}
