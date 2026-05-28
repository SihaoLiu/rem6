use rem6_net::{
    EthernetInterfaceEventKind, EthernetInterfaceId, EthernetInterfaceRegistry, EthernetPacket,
    NetworkError,
};

fn packet(bytes: &[u8]) -> EthernetPacket {
    EthernetPacket::new(bytes.to_vec()).unwrap()
}

#[test]
fn ethernet_interfaces_bind_peers_and_route_packets_as_records() {
    let mut registry = EthernetInterfaceRegistry::new();
    let device = registry.register("device.tx").unwrap();
    let link = registry.register("link.rx").unwrap();

    let binding = registry.bind_pair(device, link).unwrap();
    assert_eq!(binding.local(), device);
    assert_eq!(binding.peer(), link);
    assert_eq!(registry.peer_of(device).unwrap(), Some(link));
    assert_eq!(registry.peer_of(link).unwrap(), Some(device));
    assert!(registry.is_connected(device).unwrap());

    let send = registry
        .send_packet(device, packet(&[0xaa, 0xbb]), 17)
        .unwrap();
    assert_eq!(send.source(), device);
    assert_eq!(send.peer(), Some(link));
    assert_eq!(send.tick(), 17);
    assert_eq!(send.packet().payload(), &[0xaa, 0xbb]);
    assert!(send.accepted());
    assert_eq!(registry.receive_count(link).unwrap(), 1);
    assert_eq!(registry.last_receive_tick(link).unwrap(), Some(17));
}

#[test]
fn ethernet_interfaces_keep_disconnected_send_success_explicit() {
    let mut registry = EthernetInterfaceRegistry::new();
    let lone = registry.register("tap.unplugged").unwrap();

    let send = registry.send_packet(lone, packet(&[1, 2, 3]), 5).unwrap();

    assert_eq!(send.source(), lone);
    assert_eq!(send.peer(), None);
    assert!(send.accepted());
    assert_eq!(registry.receive_count(lone).unwrap(), 0);
    assert!(!registry.is_connected(lone).unwrap());
}

#[test]
fn ethernet_interfaces_report_peer_busy_and_recv_done_as_typed_events() {
    let mut registry = EthernetInterfaceRegistry::new();
    let nic = registry.register("nic.interface").unwrap();
    let link = registry.register("link.interface").unwrap();
    registry.bind_pair(nic, link).unwrap();

    registry.set_busy(link, true).unwrap();
    assert!(registry.ask_busy(nic).unwrap());
    assert!(!registry.ask_busy(link).unwrap());

    let event = registry.recv_done(nic, 33).unwrap();
    assert_eq!(event.interface(), link);
    assert_eq!(event.peer(), nic);
    assert_eq!(event.tick(), 33);
    assert_eq!(event.kind(), EthernetInterfaceEventKind::SendDone);
    assert_eq!(registry.send_done_count(link).unwrap(), 1);
    assert_eq!(registry.last_send_done_tick(link).unwrap(), Some(33));
}

#[test]
fn ethernet_interfaces_reject_duplicate_names_bad_ids_and_peer_rebinding() {
    let mut registry = EthernetInterfaceRegistry::new();
    let first = registry.register("port0").unwrap();
    let second = registry.register("port1").unwrap();
    let third = registry.register("port2").unwrap();

    assert!(matches!(
        registry.register("port0"),
        Err(NetworkError::DuplicateEthernetInterfaceName { name }) if name == "port0"
    ));
    assert!(matches!(
        registry.bind_pair(first, first),
        Err(NetworkError::EthernetInterfaceSelfBinding { interface }) if interface == first
    ));
    assert!(matches!(
        registry.peer_of(EthernetInterfaceId::new(99)),
        Err(NetworkError::UnknownEthernetInterface {
            interface: EthernetInterfaceId(99),
            interface_count: 3,
        })
    ));

    registry.bind_pair(first, second).unwrap();
    assert!(matches!(
        registry.bind_pair(first, third),
        Err(NetworkError::EthernetInterfacePeerAlreadyBound {
            interface,
            current_peer,
            requested_peer,
        }) if interface == first && current_peer == second && requested_peer == third
    ));
    assert_eq!(registry.peer_of(first).unwrap(), Some(second));
    assert_eq!(registry.peer_of(third).unwrap(), None);
}

#[test]
fn ethernet_interfaces_unbind_and_restore_snapshot_state() {
    let mut registry = EthernetInterfaceRegistry::new();
    let left = registry.register("left").unwrap();
    let right = registry.register("right").unwrap();
    registry.bind_pair(left, right).unwrap();
    registry.set_busy(right, true).unwrap();
    registry.send_packet(left, packet(&[1]), 10).unwrap();
    let snapshot = registry.snapshot();

    registry.unbind(left).unwrap();
    assert_eq!(registry.peer_of(left).unwrap(), None);
    assert_eq!(registry.peer_of(right).unwrap(), None);
    registry.set_busy(right, false).unwrap();

    registry.restore(&snapshot).unwrap();
    assert_eq!(registry.peer_of(left).unwrap(), Some(right));
    assert!(registry.ask_busy(left).unwrap());
    assert_eq!(registry.receive_count(right).unwrap(), 1);
    assert_eq!(registry.last_receive_tick(right).unwrap(), Some(10));
}
