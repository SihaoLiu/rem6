use rem6_net::{EthernetPacket, EthernetPacketFifo, EthernetPacketHandle, NetworkError};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn push(fifo: &mut EthernetPacketFifo, bytes: &[u8]) -> EthernetPacketHandle {
    fifo.push(packet(bytes)).unwrap()
}

#[test]
fn packet_fifo_tracks_capacity_reservation_and_wire_length() {
    let timed = EthernetPacket::new(vec![0xaa, 0xbb, 0xcc, 0xdd])
        .unwrap()
        .with_wire_length_bytes(8)
        .unwrap();
    assert_eq!(timed.payload_len(), 4);
    assert_eq!(timed.wire_length_bytes(), 8);

    let mut fifo = EthernetPacketFifo::new(10).unwrap();
    fifo.reserve(2).unwrap();
    assert_eq!(fifo.reserved_bytes(), 2);
    assert_eq!(fifo.available_bytes(), 8);

    let handle = fifo.push(timed).unwrap();
    assert_eq!(fifo.front_handle(), Some(handle));
    assert_eq!(fifo.packet_count(), 1);
    assert_eq!(fifo.queued_payload_bytes(), 4);
    assert_eq!(fifo.occupied_bytes(), 4);
    assert_eq!(fifo.reserved_bytes(), 0);
    assert_eq!(fifo.available_bytes(), 6);

    fifo.reserve(5).unwrap();
    let before = fifo.snapshot();
    assert!(matches!(
        fifo.push(packet(&[1, 2, 3, 4])),
        Err(NetworkError::ReservationExceedsPacket {
            reserved_bytes: 5,
            packet_bytes: 4,
        })
    ));
    assert_eq!(fifo.snapshot(), before);

    assert!(matches!(
        EthernetPacketFifo::new(0),
        Err(NetworkError::ZeroFifoCapacity)
    ));
    assert!(matches!(
        EthernetPacket::new(Vec::new()),
        Err(NetworkError::EmptyPacket)
    ));
    assert!(matches!(
        EthernetPacket::new(vec![1])
            .unwrap()
            .with_wire_length_bytes(0),
        Err(NetworkError::InvalidWireLength {
            payload_bytes: 1,
            wire_bytes: 0,
        })
    ));
}

#[test]
fn packet_fifo_removal_preserves_order_and_explicit_slack() {
    let mut fifo = EthernetPacketFifo::new(12).unwrap();
    let first = push(&mut fifo, &[1, 2, 3, 4]);
    let middle = push(&mut fifo, &[5, 6, 7]);
    let last = push(&mut fifo, &[8, 9]);

    assert_eq!(fifo.occupied_bytes(), 9);
    assert_eq!(fifo.queued_payload_bytes(), 9);
    assert_eq!(fifo.available_bytes(), 3);
    assert_eq!(fifo.count_packets_before(last).unwrap(), 2);
    assert_eq!(fifo.count_packets_after(first).unwrap(), 2);

    let removed = fifo.remove(middle).unwrap();
    assert_eq!(removed.payload(), &[5, 6, 7]);
    assert_eq!(fifo.packet_count(), 2);
    assert_eq!(fifo.queued_payload_bytes(), 6);
    assert_eq!(fifo.occupied_bytes(), 9);
    assert_eq!(fifo.entry_slack_bytes(first).unwrap(), 3);
    assert_eq!(fifo.available_bytes(), 3);
    assert_eq!(fifo.count_packets_before(last).unwrap(), 1);
    assert_eq!(fifo.count_packets_after(first).unwrap(), 1);

    let popped = fifo.pop().unwrap();
    assert_eq!(popped.payload(), &[1, 2, 3, 4]);
    assert_eq!(fifo.front_handle(), Some(last));
    assert_eq!(fifo.occupied_bytes(), 2);
    assert_eq!(fifo.queued_payload_bytes(), 2);
    assert_eq!(fifo.available_bytes(), 10);

    assert_eq!(fifo.pop().unwrap().payload(), &[8, 9]);
    assert!(fifo.pop().is_none());
    assert_eq!(fifo.occupied_bytes(), 0);
}

#[test]
fn packet_fifo_snapshot_restores_payload_order_and_counters() {
    let mut fifo = EthernetPacketFifo::new(16).unwrap();
    let first = push(&mut fifo, &[1, 2, 3]);
    let second = push(&mut fifo, &[4, 5]);
    fifo.reserve(4).unwrap();

    assert_eq!(fifo.copy_payload_range(1, 4).unwrap(), vec![2, 3, 4, 5]);
    assert!(matches!(
        fifo.copy_payload_range(4, 2),
        Err(NetworkError::PayloadRangeOutOfBounds {
            offset: 4,
            len: 2,
            queued_payload_bytes: 5,
        })
    ));

    let snapshot = fifo.snapshot();
    assert_eq!(snapshot.next_sequence(), 2);
    assert_eq!(snapshot.reserved_bytes(), 4);

    assert_eq!(fifo.pop().unwrap().payload(), &[1, 2, 3]);
    assert_eq!(fifo.front_handle(), Some(second));
    fifo.clear();
    assert!(fifo.is_empty());

    fifo.restore(&snapshot).unwrap();
    assert_eq!(fifo.front_handle(), Some(first));
    assert_eq!(fifo.packet_count(), 2);
    assert_eq!(fifo.queued_payload_bytes(), 5);
    assert_eq!(fifo.occupied_bytes(), 5);
    assert_eq!(fifo.reserved_bytes(), 4);
    assert_eq!(fifo.available_bytes(), 7);
    assert_eq!(fifo.copy_payload_range(0, 5).unwrap(), vec![1, 2, 3, 4, 5]);
}
