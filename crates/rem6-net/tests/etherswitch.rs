use rem6_net::{
    EthernetLinkDelayVariation, EthernetMacAddress, EthernetPacket, EthernetSwitch,
    EthernetSwitchPortId, EthernetSwitchTiming, NetworkError,
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

fn timed_frame(
    dst: [u8; 6],
    src: [u8; 6],
    payload: &[u8],
    wire_length_bytes: u64,
) -> EthernetPacket {
    frame(dst, src, payload)
        .with_wire_length_bytes(wire_length_bytes)
        .unwrap()
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
fn learning_switch_drops_lowest_priority_tail_when_capacity_is_exceeded() {
    let mut switch = EthernetSwitch::new(2, 18).unwrap();
    let dst = mac([0x02, 0, 0, 0, 0, 1]);
    let old_src = mac([0x02, 0, 0, 0, 0, 2]);
    let new_src = mac([0x02, 0, 0, 0, 0, 3]);

    switch
        .receive(
            EthernetSwitchPortId::new(0),
            frame(dst.bytes(), old_src.bytes(), &[1, 1, 1, 1]),
            10,
        )
        .unwrap();
    let decision = switch
        .receive(
            EthernetSwitchPortId::new(0),
            frame(dst.bytes(), new_src.bytes(), &[2, 2, 2, 2]),
            5,
        )
        .unwrap();

    assert_eq!(decision.output_ports(), &[EthernetSwitchPortId::new(1)]);
    assert!(decision.dropped_ports().is_empty());

    let ready = switch.drain_ready_outputs(24);
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].receive_tick(), 5);
    assert_eq!(ready[0].packet().payload()[14], 2);
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        0
    );
}

#[test]
fn learning_switch_schedules_output_ready_ticks_from_wire_length() {
    let timing = EthernetSwitchTiming::new(2, 5).unwrap();
    let mut switch = EthernetSwitch::new_with_timing(3, 100, timing).unwrap();
    let a = mac([0x02, 0, 0, 0, 0, 1]);
    let b = mac([0x02, 0, 0, 0, 0, 2]);

    switch
        .receive(
            EthernetSwitchPortId::new(0),
            timed_frame(a.bytes(), b.bytes(), &[1, 2, 3], 4),
            10,
        )
        .unwrap();

    assert!(switch.drain_ready_outputs(23).is_empty());
    let ready = switch.drain_ready_outputs(24);
    assert_eq!(ready.len(), 2);
    assert_eq!(ready[0].ready_tick(), 24);
    assert_eq!(ready[0].egress_port(), EthernetSwitchPortId::new(1));
    assert_eq!(ready[0].ingress_port(), EthernetSwitchPortId::new(0));
    assert_eq!(ready[0].packet().wire_length_bytes(), 4);
    assert_eq!(ready[1].egress_port(), EthernetSwitchPortId::new(2));
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(1)).unwrap(),
        0
    );
    assert_eq!(
        switch.port_queue_len(EthernetSwitchPortId::new(2)).unwrap(),
        0
    );
}

#[test]
fn learning_switch_orders_same_tick_output_queue_by_source_port() {
    let timing = EthernetSwitchTiming::new(1, 1).unwrap();
    let mut switch = EthernetSwitch::new_with_timing(3, 100, timing).unwrap();
    let dst = mac([0x02, 0, 0, 0, 0, 0xaa]);
    let src0 = mac([0x02, 0, 0, 0, 0, 1]);
    let src1 = mac([0x02, 0, 0, 0, 0, 2]);

    switch
        .receive(
            EthernetSwitchPortId::new(2),
            timed_frame(src0.bytes(), dst.bytes(), &[0], 1),
            0,
        )
        .unwrap();
    assert_eq!(switch.drain_ready_outputs(3).len(), 2);

    switch
        .receive(
            EthernetSwitchPortId::new(1),
            timed_frame(dst.bytes(), src1.bytes(), &[1], 2),
            0,
        )
        .unwrap();
    switch
        .receive(
            EthernetSwitchPortId::new(0),
            timed_frame(dst.bytes(), src0.bytes(), &[2], 2),
            0,
        )
        .unwrap();

    let first = switch.drain_ready_outputs(8);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].ingress_port(), EthernetSwitchPortId::new(0));
    assert_eq!(first[0].egress_port(), EthernetSwitchPortId::new(2));
    assert_eq!(first[0].packet().payload()[14], 2);
    assert_eq!(first[1].ingress_port(), EthernetSwitchPortId::new(1));
    assert_eq!(first[1].egress_port(), EthernetSwitchPortId::new(2));
    assert_eq!(first[1].ready_tick(), 8);
}

#[test]
fn learning_switch_applies_output_delay_variation_and_restores_it() {
    let timing = EthernetSwitchTiming::new(1, 1)
        .unwrap()
        .with_delay_variation(EthernetLinkDelayVariation::new(3, vec![2, 0]).unwrap());
    let mut switch = EthernetSwitch::new_with_timing(2, 100, timing).unwrap();
    let dst = mac([0x02, 0, 0, 0, 0, 1]);
    let src = mac([0x02, 0, 0, 0, 0, 2]);

    switch
        .receive(
            EthernetSwitchPortId::new(0),
            timed_frame(dst.bytes(), src.bytes(), &[1], 2),
            0,
        )
        .unwrap();
    let snapshot = switch.snapshot();
    let first = switch.drain_ready_outputs(6);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].delay_variation_ticks(), 2);
    assert_eq!(first[0].ready_tick(), 6);

    switch.restore(&snapshot).unwrap();
    let restored = switch.drain_ready_outputs(6);
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].delay_variation_ticks(), 2);
    assert_eq!(restored[0].ready_tick(), 6);
}

#[test]
fn learning_switch_rejects_invalid_ports_and_short_frames() {
    assert!(matches!(
        EthernetSwitch::new(0, 100),
        Err(NetworkError::InvalidEthernetSwitchPortCount { port_count: 0 })
    ));
    assert!(matches!(
        EthernetSwitchTiming::new(0, 1),
        Err(NetworkError::InvalidEthernetSwitchRate { ticks_per_byte: 0 })
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
