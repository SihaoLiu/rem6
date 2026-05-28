use rem6_net::{EthernetBus, EthernetBusPortId, EthernetBusTiming, EthernetPacket, NetworkError};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

fn timed_packet(bytes: &[u8], wire_length_bytes: u64) -> EthernetPacket {
    packet(bytes)
        .with_wire_length_bytes(wire_length_bytes)
        .unwrap()
}

#[test]
fn ethernet_bus_broadcasts_after_serialization_delay() {
    let timing = EthernetBusTiming::new(2).unwrap();
    let mut bus = EthernetBus::new(4, timing).unwrap();

    let transmission = bus
        .transmit(
            EthernetBusPortId::new(1),
            timed_packet(&[0xaa, 0xbb, 0xcc, 0xdd], 4),
            10,
        )
        .unwrap();

    assert_eq!(transmission.sequence(), 0);
    assert_eq!(transmission.sender_port(), EthernetBusPortId::new(1));
    assert_eq!(transmission.request_tick(), 10);
    assert_eq!(transmission.completion_tick(), 19);
    assert_eq!(
        transmission.recipient_ports(),
        &[
            EthernetBusPortId::new(0),
            EthernetBusPortId::new(2),
            EthernetBusPortId::new(3),
        ]
    );
    assert!(bus.is_busy());
    assert_eq!(bus.busy_until_tick(), Some(19));
    assert_eq!(bus.pending_transmission_count(), 1);

    assert!(bus.drain_ready_deliveries(18).is_empty());
    let ready = bus.drain_ready_deliveries(19);
    let recipients: Vec<_> = ready
        .iter()
        .map(|delivery| delivery.recipient_port())
        .collect();
    assert_eq!(
        recipients,
        vec![
            EthernetBusPortId::new(0),
            EthernetBusPortId::new(2),
            EthernetBusPortId::new(3),
        ]
    );
    assert!(ready.iter().all(|delivery| delivery.sequence() == 0));
    assert!(ready
        .iter()
        .all(|delivery| delivery.sender_port() == EthernetBusPortId::new(1)));
    assert!(ready.iter().all(|delivery| delivery.delivery_tick() == 19));
    assert!(ready
        .iter()
        .all(|delivery| delivery.packet().payload() == [0xaa, 0xbb, 0xcc, 0xdd]));
    assert!(!bus.is_busy());
    assert_eq!(bus.pending_transmission_count(), 0);
}

#[test]
fn ethernet_bus_loopback_delivers_to_sender() {
    let timing = EthernetBusTiming::new(1).unwrap();
    let mut bus = EthernetBus::new_with_loopback(3, timing, true).unwrap();

    let transmission = bus
        .transmit(EthernetBusPortId::new(2), timed_packet(&[1, 2], 2), 4)
        .unwrap();

    assert_eq!(
        transmission.recipient_ports(),
        &[
            EthernetBusPortId::new(0),
            EthernetBusPortId::new(1),
            EthernetBusPortId::new(2),
        ]
    );
    let ready = bus.drain_ready_deliveries(7);
    assert_eq!(ready.len(), 3);
    assert_eq!(ready[2].recipient_port(), EthernetBusPortId::new(2));
}

#[test]
fn ethernet_bus_rejects_invalid_ports_busy_state_and_overflow() {
    assert!(matches!(
        EthernetBusTiming::new(0),
        Err(NetworkError::InvalidEthernetBusRate { ticks_per_byte: 0 })
    ));
    assert!(matches!(
        EthernetBus::new(0, EthernetBusTiming::new(1).unwrap()),
        Err(NetworkError::InvalidEthernetBusPortCount { port_count: 0 })
    ));

    let timing = EthernetBusTiming::new(1).unwrap();
    let mut bus = EthernetBus::new(2, timing).unwrap();
    assert!(matches!(
        bus.transmit(EthernetBusPortId::new(2), packet(&[1]), 0),
        Err(NetworkError::UnknownEthernetBusPort {
            port: EthernetBusPortId(2),
            port_count: 2,
        })
    ));

    bus.transmit(EthernetBusPortId::new(0), timed_packet(&[1], 2), 0)
        .unwrap();
    assert!(matches!(
        bus.transmit(EthernetBusPortId::new(1), packet(&[2]), 2),
        Err(NetworkError::EthernetBusBusy {
            sender_port: EthernetBusPortId(1),
            request_tick: 2,
            busy_until_tick: 3,
        })
    ));

    bus.drain_ready_deliveries(3);
    let too_large = packet(&[3]).with_wire_length_bytes(u64::MAX).unwrap();
    assert!(matches!(
        bus.transmit(EthernetBusPortId::new(0), too_large, 0),
        Err(NetworkError::EthernetBusTimingOverflow {
            request_tick: 0,
            wire_length_bytes: u64::MAX,
            ticks_per_byte: 1,
        })
    ));
}

#[test]
fn ethernet_bus_snapshot_restores_pending_transmission_and_sequence() {
    let timing = EthernetBusTiming::new(2).unwrap();
    let mut bus = EthernetBus::new(3, timing).unwrap();

    let first = bus
        .transmit(EthernetBusPortId::new(0), timed_packet(&[1, 2], 3), 5)
        .unwrap();
    let snapshot = bus.snapshot();

    assert_eq!(snapshot.next_sequence(), 1);
    assert_eq!(snapshot.pending_transmission_count(), 1);
    assert_eq!(snapshot.busy_until_tick(), Some(12));

    assert_eq!(bus.drain_ready_deliveries(12).len(), 2);
    bus.transmit(EthernetBusPortId::new(1), packet(&[9]), 13)
        .unwrap();

    bus.restore(&snapshot).unwrap();
    assert_eq!(bus.next_sequence(), 1);
    assert_eq!(bus.pending_transmission_count(), 1);
    assert!(bus.is_busy());

    let restored = bus.drain_ready_deliveries(12);
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].sequence(), first.sequence());
    assert_eq!(restored[0].sender_port(), EthernetBusPortId::new(0));
    assert_eq!(restored[0].packet().payload(), &[1, 2]);

    let next = bus
        .transmit(EthernetBusPortId::new(1), packet(&[3]), 13)
        .unwrap();
    assert_eq!(next.sequence(), 1);
}
