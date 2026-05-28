use rem6_net::{
    EthernetMacAddress, EthernetPacket, EthernetSwitch, EthernetSwitchPortId, NetworkError,
};

fn mac(bytes: [u8; 6]) -> EthernetMacAddress {
    EthernetMacAddress::new(bytes)
}

fn frame(dst: [u8; 6], src: [u8; 6], payload: &[u8]) -> EthernetPacket {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&dst);
    bytes.extend_from_slice(&src);
    bytes.extend_from_slice(&[0x08, 0x00]);
    bytes.extend_from_slice(payload);
    EthernetPacket::new(bytes).unwrap()
}

#[test]
fn learning_switch_floods_unknown_and_learns_source_port() {
    let mut switch = EthernetSwitch::new(4, 100).unwrap();
    let source = mac([0x02, 0, 0, 0, 0, 1]);
    let unknown = mac([0x02, 0, 0, 0, 0, 2]);

    let decision = switch
        .receive(
            EthernetSwitchPortId::new(1),
            frame(unknown.bytes(), source.bytes(), &[1, 2, 3]),
            10,
        )
        .unwrap();

    assert_eq!(decision.ingress_port(), EthernetSwitchPortId::new(1));
    assert_eq!(decision.destination_mac(), unknown);
    assert_eq!(decision.source_mac(), source);
    assert!(decision.flooded());
    assert_eq!(
        decision.output_ports(),
        &[
            EthernetSwitchPortId::new(0),
            EthernetSwitchPortId::new(2),
            EthernetSwitchPortId::new(3),
        ]
    );
    assert_eq!(
        switch.lookup_port(source, 10).unwrap(),
        Some(EthernetSwitchPortId::new(1))
    );
    assert_eq!(switch.forwarding_table_len(), 1);
}

#[test]
fn learning_switch_unicasts_known_destinations_and_updates_last_use() {
    let mut switch = EthernetSwitch::new_with_ttl(3, 100, 50).unwrap();
    let a = mac([0x02, 0, 0, 0, 0, 0x0a]);
    let b = mac([0x02, 0, 0, 0, 0, 0x0b]);

    switch
        .receive(
            EthernetSwitchPortId::new(1),
            frame(a.bytes(), b.bytes(), &[9]),
            20,
        )
        .unwrap();
    assert_eq!(
        switch.lookup_port(b, 70).unwrap(),
        Some(EthernetSwitchPortId::new(1))
    );

    let decision = switch
        .receive(
            EthernetSwitchPortId::new(2),
            frame(b.bytes(), a.bytes(), &[1, 2]),
            30,
        )
        .unwrap();

    assert!(!decision.flooded());
    assert_eq!(decision.output_ports(), &[EthernetSwitchPortId::new(1)]);
    assert_eq!(
        switch.lookup_port(a, 30).unwrap(),
        Some(EthernetSwitchPortId::new(2))
    );
    assert_eq!(switch.forwarding_table_len(), 2);
}

#[test]
fn learning_switch_expires_stale_entries_and_floods_multicast() {
    let mut switch = EthernetSwitch::new_with_ttl(3, 100, 5).unwrap();
    let a = mac([0x02, 0, 0, 0, 0, 1]);
    let b = mac([0x02, 0, 0, 0, 0, 2]);
    let multicast = mac([0x01, 0, 0x5e, 0, 0, 1]);

    switch
        .receive(
            EthernetSwitchPortId::new(1),
            frame(a.bytes(), b.bytes(), &[1]),
            0,
        )
        .unwrap();
    assert_eq!(
        switch.lookup_port(b, 5).unwrap(),
        Some(EthernetSwitchPortId::new(1))
    );
    assert_eq!(switch.lookup_port(b, 6).unwrap(), None);

    let decision = switch
        .receive(
            EthernetSwitchPortId::new(0),
            frame(multicast.bytes(), a.bytes(), &[2]),
            7,
        )
        .unwrap();
    assert!(decision.flooded());
    assert_eq!(
        decision.output_ports(),
        &[EthernetSwitchPortId::new(1), EthernetSwitchPortId::new(2)]
    );
}

#[test]
fn learning_switch_snapshot_restores_forwarding_table_and_queues() {
    let mut switch = EthernetSwitch::new(3, 100).unwrap();
    let a = mac([0x02, 0, 0, 0, 0, 1]);
    let b = mac([0x02, 0, 0, 0, 0, 2]);

    switch
        .receive(
            EthernetSwitchPortId::new(1),
            frame(a.bytes(), b.bytes(), &[1]),
            0,
        )
        .unwrap();
    let snapshot = switch.snapshot();

    switch
        .receive(
            EthernetSwitchPortId::new(2),
            frame(b.bytes(), a.bytes(), &[2]),
            1,
        )
        .unwrap();
    assert_eq!(switch.forwarding_table_len(), 2);
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        1
    );

    switch.restore(&snapshot).unwrap();
    assert_eq!(switch.forwarding_table_len(), 1);
    assert_eq!(
        switch.lookup_port(b, 0).unwrap(),
        Some(EthernetSwitchPortId::new(1))
    );
    assert_eq!(switch.lookup_port(a, 1).unwrap(), None);
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        0
    );
}

#[test]
fn learning_switch_records_output_queue_drops_when_capacity_is_exceeded() {
    let mut switch = EthernetSwitch::new(2, 20).unwrap();
    let a = mac([0x02, 0, 0, 0, 0, 1]);
    let b = mac([0x02, 0, 0, 0, 0, 2]);
    let c = mac([0x02, 0, 0, 0, 0, 3]);

    let first = switch
        .receive(
            EthernetSwitchPortId::new(0),
            frame(a.bytes(), b.bytes(), &[1, 2, 3, 4]),
            0,
        )
        .unwrap();
    assert!(first.dropped_ports().is_empty());
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        1
    );

    let second = switch
        .receive(
            EthernetSwitchPortId::new(0),
            frame(a.bytes(), c.bytes(), &[5, 6, 7, 8]),
            1,
        )
        .unwrap();
    assert_eq!(second.dropped_ports(), &[EthernetSwitchPortId::new(1)]);
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        1
    );
}

#[test]
fn learning_switch_rejects_invalid_ports_and_short_frames() {
    assert!(matches!(
        EthernetSwitch::new(0, 100),
        Err(NetworkError::InvalidEthernetSwitchPortCount { port_count: 0 })
    ));

    let mut switch = EthernetSwitch::new(2, 100).unwrap();
    assert!(matches!(
        switch.receive(EthernetSwitchPortId::new(2), frame([0; 6], [1; 6], &[1]), 0,),
        Err(NetworkError::UnknownEthernetSwitchPort {
            port: EthernetSwitchPortId(2),
            port_count: 2,
        })
    ));
    assert!(matches!(
        switch.receive(
            EthernetSwitchPortId::new(0),
            EthernetPacket::new(vec![1, 2, 3]).unwrap(),
            0,
        ),
        Err(NetworkError::EthernetFrameTooShort { payload_bytes: 3 })
    ));
}
