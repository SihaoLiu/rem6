use rem6_fabric::{
    FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath, FabricPathHop,
    FabricQosRequest, QosError, QosFixedPriorityPolicy, QosPriority, QosProportionalFairPolicy,
    QosQueueArbiter, QosQueuePolicyKind, QosQueuedRequest, QosRequestId, QosRequestorId,
    VirtualNetworkId,
};

fn request(id: u64, requestor: u32, priority: u8, bytes: u64, order: u64) -> QosQueuedRequest {
    QosQueuedRequest::new(
        QosRequestId::new(id),
        QosRequestorId::new(requestor),
        QosPriority::new(priority),
        bytes,
        order,
    )
    .unwrap()
}

fn packet(id: u64) -> FabricPacket {
    FabricPacket::new(FabricPacketId::new(id), 8, VirtualNetworkId::new(0)).unwrap()
}

fn link(name: &str) -> FabricLinkId {
    FabricLinkId::new(name).unwrap()
}

fn path(name: &str) -> FabricPath {
    FabricPath::new([FabricPathHop::new(link(name), 1, 8).unwrap()]).unwrap()
}

fn fabric_qos_request(id: u64, requestor: u32, priority: u8, order: u64) -> FabricQosRequest {
    FabricQosRequest::new(
        QosRequestorId::new(requestor),
        QosPriority::new(priority),
        order,
        packet(id),
        path("mesh_qos"),
    )
}

#[test]
fn qos_fixed_priority_assigns_explicit_requestor_priorities() {
    assert_eq!(
        QosFixedPriorityPolicy::new(0, QosPriority::new(0)).unwrap_err(),
        QosError::ZeroPriorityLevels
    );
    assert_eq!(
        QosFixedPriorityPolicy::new(2, QosPriority::new(2)).unwrap_err(),
        QosError::PriorityOutOfRange {
            priority: QosPriority::new(2),
            priority_levels: 2
        }
    );

    let policy = QosFixedPriorityPolicy::new(4, QosPriority::new(2))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(10), QosPriority::new(0))
        .unwrap()
        .with_requestor_priority(QosRequestorId::new(20), QosPriority::new(3))
        .unwrap();

    assert_eq!(
        policy.priority_for(QosRequestorId::new(10), 64),
        QosPriority::new(0)
    );
    assert_eq!(
        policy.priority_for(QosRequestorId::new(20), 64),
        QosPriority::new(3)
    );
    assert_eq!(
        policy.priority_for(QosRequestorId::new(30), 64),
        QosPriority::new(2)
    );
    assert_eq!(
        policy
            .clone()
            .with_requestor_priority(QosRequestorId::new(40), QosPriority::new(4))
            .unwrap_err(),
        QosError::PriorityOutOfRange {
            priority: QosPriority::new(4),
            priority_levels: 4
        }
    );
}

#[test]
fn qos_proportional_fair_maps_gem5_rank_to_rem6_priority_and_updates_scores() {
    let requestor_a = QosRequestorId::new(1);
    let requestor_b = QosRequestorId::new(2);
    let requestor_c = QosRequestorId::new(3);
    let mut policy = QosProportionalFairPolicy::new(3, 0.5)
        .unwrap()
        .with_requestor_score(requestor_a, 100.0)
        .unwrap()
        .with_requestor_score(requestor_b, 40.0)
        .unwrap()
        .with_requestor_score(requestor_c, 10.0)
        .unwrap();

    assert_eq!(
        policy.priority_for(requestor_a, 64).unwrap(),
        QosPriority::new(2)
    );
    assert_eq!(policy.score_for(requestor_a), Some(82.0));
    assert_eq!(policy.score_for(requestor_b), Some(20.0));
    assert_eq!(policy.score_for(requestor_c), Some(5.0));

    assert_eq!(
        policy.priority_for(requestor_c, 64).unwrap(),
        QosPriority::new(0)
    );
    assert_eq!(policy.score_for(requestor_a), Some(41.0));
    assert_eq!(policy.score_for(requestor_b), Some(10.0));
    assert_eq!(policy.score_for(requestor_c), Some(34.5));

    assert_eq!(
        policy.priority_for(requestor_b, 64).unwrap(),
        QosPriority::new(0)
    );
    assert_eq!(policy.score_for(requestor_a), Some(20.5));
    assert_eq!(policy.score_for(requestor_b), Some(37.0));
    assert_eq!(policy.score_for(requestor_c), Some(17.25));
}

#[test]
fn qos_proportional_fair_rejects_ambiguous_configuration() {
    assert_eq!(
        QosProportionalFairPolicy::new(0, 0.5).unwrap_err(),
        QosError::ZeroPriorityLevels
    );
    assert!(matches!(
        QosProportionalFairPolicy::new(2, -0.1).unwrap_err(),
        QosError::InvalidProportionalFairWeight { .. }
    ));
    assert!(matches!(
        QosProportionalFairPolicy::new(2, f64::NAN).unwrap_err(),
        QosError::InvalidProportionalFairWeight { .. }
    ));

    let requestor = QosRequestorId::new(10);
    let policy = QosProportionalFairPolicy::new(1, 0.5)
        .unwrap()
        .with_requestor_score(requestor, 1.0)
        .unwrap();
    assert_eq!(
        policy
            .clone()
            .with_requestor_score(requestor, 2.0)
            .unwrap_err(),
        QosError::DuplicateRequestorScore { requestor }
    );
    assert_eq!(
        policy
            .clone()
            .with_requestor_score(QosRequestorId::new(11), 1.0)
            .unwrap_err(),
        QosError::TooManyProportionalFairRequestors {
            requestor_count: 2,
            priority_levels: 1,
        }
    );
    assert!(matches!(
        QosProportionalFairPolicy::new(1, 0.5)
            .unwrap()
            .with_requestor_score(QosRequestorId::new(12), f64::INFINITY)
            .unwrap_err(),
        QosError::InvalidProportionalFairScore { .. }
    ));
}

#[test]
fn qos_proportional_fair_preserves_snapshot_replay_and_unknown_errors() {
    let requestor_a = QosRequestorId::new(21);
    let requestor_b = QosRequestorId::new(22);
    let mut policy = QosProportionalFairPolicy::new(2, 0.25)
        .unwrap()
        .with_requestor_score(requestor_a, 16.0)
        .unwrap()
        .with_requestor_score(requestor_b, 4.0)
        .unwrap();

    assert_eq!(
        policy.priority_for(requestor_a, 8).unwrap(),
        QosPriority::new(1)
    );
    let replay_point = policy.snapshot();
    let next_priority = policy.priority_for(requestor_b, 8).unwrap();
    let next_scores = (policy.score_for(requestor_a), policy.score_for(requestor_b));

    policy.restore(replay_point.clone()).unwrap();
    assert_eq!(policy.priority_for(requestor_b, 8).unwrap(), next_priority);
    assert_eq!(
        (policy.score_for(requestor_a), policy.score_for(requestor_b)),
        next_scores
    );

    policy.restore(replay_point).unwrap();
    let before_unknown = policy.snapshot();
    assert_eq!(
        policy.priority_for(QosRequestorId::new(23), 8).unwrap_err(),
        QosError::UnknownProportionalFairRequestor {
            requestor: QosRequestorId::new(23),
        }
    );
    assert_eq!(policy.snapshot(), before_unknown);
}

#[test]
fn qos_queue_arbiter_selects_highest_priority_fifo_and_lifo() {
    let queue = [
        request(1, 10, 1, 64, 0),
        request(2, 20, 0, 64, 1),
        request(3, 30, 0, 64, 2),
        request(4, 40, 2, 64, 3),
    ];

    let mut fifo = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let grant = fifo.grant(&queue).unwrap();
    assert_eq!(grant.queue_index(), 1);
    assert_eq!(grant.request_id(), QosRequestId::new(2));
    assert_eq!(grant.priority(), QosPriority::new(0));

    let mut lifo = QosQueueArbiter::new(QosQueuePolicyKind::Lifo);
    let grant = lifo.grant(&queue).unwrap();
    assert_eq!(grant.queue_index(), 2);
    assert_eq!(grant.request_id(), QosRequestId::new(3));
    assert_eq!(grant.priority(), QosPriority::new(0));
}

#[test]
fn qos_lrg_round_robins_requestors_without_mutating_empty_polls() {
    assert_eq!(
        QosQueuedRequest::new(
            QosRequestId::new(99),
            QosRequestorId::new(9),
            QosPriority::new(0),
            0,
            0
        )
        .unwrap_err(),
        QosError::ZeroRequestBytes
    );

    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::LeastRecentlyGranted);
    let empty_snapshot = arbiter.snapshot();
    assert_eq!(arbiter.grant(&[]), None);
    assert_eq!(arbiter.snapshot(), empty_snapshot);

    let first_queue = [
        request(10, 1, 0, 64, 0),
        request(11, 1, 0, 64, 1),
        request(12, 2, 0, 64, 2),
        request(13, 3, 1, 64, 3),
    ];
    let first = arbiter.grant(&first_queue).unwrap();
    assert_eq!(first.queue_index(), 0);
    assert_eq!(first.requestor(), QosRequestorId::new(1));
    assert_eq!(first.request_id(), QosRequestId::new(10));

    let restored = arbiter.snapshot();
    let second_queue = [
        request(11, 1, 0, 64, 1),
        request(12, 2, 0, 64, 2),
        request(13, 3, 1, 64, 3),
    ];
    let second = arbiter.grant(&second_queue).unwrap();
    assert_eq!(second.queue_index(), 1);
    assert_eq!(second.requestor(), QosRequestorId::new(2));
    assert_eq!(second.request_id(), QosRequestId::new(12));

    arbiter.restore(restored);
    let replayed = arbiter.grant(&second_queue).unwrap();
    assert_eq!(replayed, second);

    let third_queue = [
        request(11, 1, 0, 64, 1),
        request(13, 3, 1, 64, 3),
        request(14, 2, 1, 64, 4),
    ];
    let third = arbiter.grant(&third_queue).unwrap();
    assert_eq!(third.queue_index(), 0);
    assert_eq!(third.requestor(), QosRequestorId::new(1));
    assert_eq!(third.request_id(), QosRequestId::new(11));
}

#[test]
fn qos_batch_transmit_orders_fabric_reservations_by_priority_and_lrg() {
    let mut fabric = FabricModel::new();
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::LeastRecentlyGranted);

    let transfers = fabric
        .transmit_qos_batch(
            0,
            [
                fabric_qos_request(30, 3, 1, 0),
                fabric_qos_request(10, 1, 0, 1),
                fabric_qos_request(20, 2, 0, 2),
                fabric_qos_request(11, 1, 0, 3),
            ],
            &mut arbiter,
        )
        .unwrap();

    let ids: Vec<_> = transfers
        .iter()
        .map(|transfer| transfer.packet().id())
        .collect();
    assert_eq!(
        ids,
        vec![
            FabricPacketId::new(10),
            FabricPacketId::new(20),
            FabricPacketId::new(11),
            FabricPacketId::new(30)
        ]
    );
    assert_eq!(transfers[0].hops()[0].start_tick(), 0);
    assert_eq!(transfers[1].hops()[0].start_tick(), 1);
    assert_eq!(transfers[2].hops()[0].start_tick(), 2);
    assert_eq!(transfers[3].hops()[0].start_tick(), 3);
    assert_eq!(fabric.total_queue_delay_ticks(), 6);

    let snapshot = arbiter.snapshot();
    let replayed = fabric
        .transmit_qos_batch(
            10,
            [
                fabric_qos_request(40, 2, 0, 0),
                fabric_qos_request(41, 1, 0, 1),
            ],
            &mut arbiter,
        )
        .unwrap();
    assert_eq!(replayed[0].packet().id(), FabricPacketId::new(40));
    assert_eq!(replayed[1].packet().id(), FabricPacketId::new(41));

    arbiter.restore(snapshot);
    let replayed_again = fabric
        .transmit_qos_batch(
            20,
            [
                fabric_qos_request(50, 2, 0, 0),
                fabric_qos_request(51, 1, 0, 1),
            ],
            &mut arbiter,
        )
        .unwrap();
    assert_eq!(replayed_again[0].packet().id(), FabricPacketId::new(50));
    assert_eq!(replayed_again[1].packet().id(), FabricPacketId::new(51));
}
