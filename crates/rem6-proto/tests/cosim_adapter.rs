use rem6_memory::Address;
use rem6_proto::{
    CoSimAdapterBoundary, CoSimAdapterKind, CoSimEndpoint, CoSimEndpointId, CoSimEvent,
    CoSimEventKind, ProtoError,
};

fn endpoint(name: &str, kind: CoSimAdapterKind) -> CoSimEndpoint {
    CoSimEndpoint::new(CoSimEndpointId::new(name).unwrap(), kind, 1_000_000_000).unwrap()
}

#[test]
fn cosim_adapter_boundary_hands_off_events_and_records_acknowledgements() {
    let mut boundary = CoSimAdapterBoundary::new();
    let systemc = endpoint("systemc.memory", CoSimAdapterKind::SystemC);
    boundary.register_endpoint(systemc.clone()).unwrap();

    let event = CoSimEvent::new(1, systemc.id().clone(), 42, CoSimEventKind::TlmTransaction)
        .with_address(Address::new(0x8000))
        .with_size(2)
        .with_payload(vec![0xaa, 0xbb]);
    boundary.handoff_event(event).unwrap();

    assert_eq!(boundary.pending_events().len(), 1);
    assert_eq!(boundary.pending_events()[0].endpoint(), systemc.id());
    assert_eq!(
        boundary.pending_events()[0].address(),
        Some(Address::new(0x8000))
    );

    let receipt = boundary.acknowledge_event(1, 45).unwrap();
    assert_eq!(receipt.sequence(), 1);
    assert_eq!(receipt.endpoint(), systemc.id());
    assert_eq!(receipt.accepted_tick(), 45);
    assert!(boundary.pending_events().is_empty());
    assert_eq!(boundary.completed_events().len(), 1);
}

#[test]
fn cosim_adapter_checkpoint_rejects_inflight_events_and_restores_clean_boundary() {
    let mut boundary = CoSimAdapterBoundary::new();
    let tlm = endpoint("tlm.fabric", CoSimAdapterKind::Tlm);
    boundary.register_endpoint(tlm.clone()).unwrap();
    boundary
        .handoff_event(CoSimEvent::new(
            7,
            tlm.id().clone(),
            100,
            CoSimEventKind::Interrupt,
        ))
        .unwrap();

    assert_eq!(
        boundary.snapshot().unwrap_err(),
        ProtoError::CoSimCheckpointHasPendingEvents { pending: 1 },
    );

    boundary.acknowledge_event(7, 101).unwrap();
    let snapshot = boundary.snapshot().unwrap();
    assert_eq!(snapshot.endpoints().len(), 1);
    assert_eq!(snapshot.completed_events().len(), 1);

    let mut restored = CoSimAdapterBoundary::restore(snapshot).unwrap();
    assert_eq!(
        restored.endpoint(tlm.id()).unwrap().kind(),
        CoSimAdapterKind::Tlm
    );
    assert_eq!(restored.completed_events()[0].sequence(), 7);

    restored
        .handoff_event(CoSimEvent::new(
            8,
            tlm.id().clone(),
            110,
            CoSimEventKind::ClockAdvance,
        ))
        .unwrap();
    assert_eq!(restored.pending_events()[0].sequence(), 8);
}

#[test]
fn cosim_adapter_rejects_ambiguous_external_handoff() {
    assert_eq!(
        CoSimEndpointId::new("").unwrap_err(),
        ProtoError::EmptyCoSimEndpoint,
    );
    assert_eq!(
        CoSimEndpoint::new(
            CoSimEndpointId::new("bad").unwrap(),
            CoSimAdapterKind::Sst,
            0
        )
        .unwrap_err(),
        ProtoError::ZeroCoSimEndpointTickFrequency,
    );

    let mut boundary = CoSimAdapterBoundary::new();
    let sst = endpoint("sst.link0", CoSimAdapterKind::Sst);
    boundary.register_endpoint(sst.clone()).unwrap();
    assert_eq!(
        boundary
            .register_endpoint(
                CoSimEndpoint::new(sst.id().clone(), CoSimAdapterKind::SystemC, 500_000_000,)
                    .unwrap()
            )
            .unwrap_err(),
        ProtoError::DuplicateCoSimEndpoint {
            endpoint: "sst.link0".to_string(),
        },
    );
    assert_eq!(
        boundary.endpoint(sst.id()).unwrap().kind(),
        CoSimAdapterKind::Sst,
    );
    assert_eq!(
        boundary
            .handoff_event(CoSimEvent::new(
                0,
                sst.id().clone(),
                1,
                CoSimEventKind::TrafficPacket,
            ))
            .unwrap_err(),
        ProtoError::ZeroCoSimEventSequence,
    );
    assert_eq!(
        boundary
            .handoff_event(CoSimEvent::new(
                1,
                CoSimEndpointId::new("missing").unwrap(),
                1,
                CoSimEventKind::TrafficPacket,
            ))
            .unwrap_err(),
        ProtoError::UnknownCoSimEndpoint {
            endpoint: "missing".to_string(),
        },
    );

    assert_eq!(
        boundary
            .handoff_event(CoSimEvent::new(
                2,
                sst.id().clone(),
                2,
                CoSimEventKind::TrafficPacket,
            ))
            .unwrap_err(),
        ProtoError::MissingCoSimEventShape {
            sequence: 2,
            kind: CoSimEventKind::TrafficPacket,
        },
    );
    assert_eq!(
        boundary
            .handoff_event(
                CoSimEvent::new(3, sst.id().clone(), 2, CoSimEventKind::TlmTransaction)
                    .with_address(Address::new(0x1000)),
            )
            .unwrap_err(),
        ProtoError::MissingCoSimEventShape {
            sequence: 3,
            kind: CoSimEventKind::TlmTransaction,
        },
    );
    boundary
        .handoff_event(
            CoSimEvent::new(1, sst.id().clone(), 1, CoSimEventKind::TrafficPacket)
                .with_address(Address::new(0x2000))
                .with_size(2)
                .with_payload(vec![0xcc, 0xdd]),
        )
        .unwrap();
    assert_eq!(
        boundary
            .handoff_event(
                CoSimEvent::new(1, sst.id().clone(), 2, CoSimEventKind::TrafficPacket)
                    .with_address(Address::new(0x2000))
                    .with_size(2)
                    .with_payload(vec![0xcc, 0xdd]),
            )
            .unwrap_err(),
        ProtoError::DuplicateCoSimEvent { sequence: 1 },
    );
    assert_eq!(
        boundary.acknowledge_event(99, 3).unwrap_err(),
        ProtoError::UnknownCoSimEvent { sequence: 99 },
    );
}

#[test]
fn cosim_adapter_rejects_data_event_payload_size_mismatch() {
    let mut boundary = CoSimAdapterBoundary::new();
    let tlm = endpoint("tlm.memory", CoSimAdapterKind::Tlm);
    boundary.register_endpoint(tlm.clone()).unwrap();

    let error = boundary
        .handoff_event(
            CoSimEvent::new(1, tlm.id().clone(), 10, CoSimEventKind::TlmTransaction)
                .with_address(Address::new(0x4000))
                .with_size(4)
                .with_payload(vec![0xaa, 0xbb]),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ProtoError::CoSimEventPayloadSizeMismatch {
            sequence: 1,
            expected_bytes: 4,
            actual_bytes: 2,
        }
    );

    let error = boundary
        .handoff_event(
            CoSimEvent::new(2, tlm.id().clone(), 11, CoSimEventKind::TrafficPacket)
                .with_address(Address::new(0x5000))
                .with_size(1)
                .with_payload(Vec::new()),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ProtoError::CoSimEventPayloadSizeMismatch {
            sequence: 2,
            expected_bytes: 1,
            actual_bytes: 0,
        }
    );
}
