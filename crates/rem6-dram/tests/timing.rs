use rem6_dram::{
    DramAccessKind, DramCommandKind, DramController, DramError, DramGeometry, DramQosRequest,
    DramQosSchedulingPolicy, DramQosTurnaroundPolicy, DramTiming,
};
use rem6_fabric::{
    QosPriority, QosProportionalFairPolicy, QosQueueArbiter, QosQueuePolicyKind, QosRequestorId,
};
use rem6_kernel::{WaitForEdgeKind, WaitForNode};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(3, 5, 7, 2, 4).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(4, 256, 64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    request_id_from(2, sequence)
}

fn request_id_from(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    read_from(2, address, size, sequence)
}

fn read_from(agent: u32, address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id_from(agent, sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn dram_controller_qos_batch_orders_same_arrival_before_timing() {
    let mut controller = DramController::new(geometry(), timing());
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let low = read_from(1, 0x0000, 8, 30);
    let high = read_from(9, 0x0100, 8, 31);
    let marker = controller.mark_wait_for();

    let accesses = controller
        .schedule_qos_batch(
            0,
            [
                DramQosRequest::new(&low, QosPriority::new(1), 0),
                DramQosRequest::new(&high, QosPriority::new(0), 1),
            ],
            &mut arbiter,
        )
        .unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].request(), high.id());
    assert_eq!(accesses[0].command_cycle(), 3);
    assert_eq!(accesses[0].ready_cycle(), 8);
    assert_eq!(accesses[1].request(), low.id());
    assert!(accesses[1].row_hit());
    assert_eq!(accesses[1].command_cycle(), 8);
    assert_eq!(accesses[1].ready_cycle(), 13);

    let low_request = WaitForNode::transaction("dram.agent.1.request.30").unwrap();
    let bank = WaitForNode::resource("dram.port.0.bank.0").unwrap();
    let graph = controller.wait_for_graph_since(marker).snapshot();
    assert_eq!(graph.edge_count(), 1);
    assert!(graph.contains_edge(&low_request, &bank, WaitForEdgeKind::Queue));
    assert_eq!(graph.dependencies(&low_request)[0].last_observed_tick(), 7);
}

#[test]
fn dram_controller_qos_batch_can_prefer_current_bus_direction() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 40)).unwrap();
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let write_first = write_from(4, 0x0040, 41);
    let read_second = read_from(5, 0x0100, 8, 42);

    let accesses = controller
        .schedule_qos_batch_with_turnaround_policy(
            8,
            [
                DramQosRequest::new(&write_first, QosPriority::new(0), 0),
                DramQosRequest::new(&read_second, QosPriority::new(0), 1),
            ],
            &mut arbiter,
            DramQosTurnaroundPolicy::PreferCurrentDirection,
        )
        .unwrap();

    assert_eq!(accesses.len(), 2);
    assert_eq!(accesses[0].request(), read_second.id());
    assert_eq!(accesses[0].kind(), DramAccessKind::Read);
    assert!(accesses[0].row_hit());
    assert_eq!(accesses[0].command_cycle(), 8);
    assert_eq!(accesses[0].ready_cycle(), 13);
    assert_eq!(accesses[1].request(), write_first.id());
    assert_eq!(accesses[1].kind(), DramAccessKind::Write);
    assert_eq!(accesses[1].command_cycle(), 12);
    assert_eq!(accesses[1].ready_cycle(), 19);
}

#[test]
fn dram_controller_qos_turnaround_highest_priority_uses_opposite_direction_on_tie() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 43)).unwrap();
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let low_priority_read = read_from(4, 0x0040, 8, 44);
    let high_priority_write = write_from(5, 0x0080, 45);
    let high_priority_read = read_from(6, 0x0100, 8, 46);

    let accesses = controller
        .schedule_qos_batch_with_turnaround_policy(
            8,
            [
                DramQosRequest::new(&low_priority_read, QosPriority::new(1), 0),
                DramQosRequest::new(&high_priority_write, QosPriority::new(0), 1),
                DramQosRequest::new(&high_priority_read, QosPriority::new(0), 2),
            ],
            &mut arbiter,
            DramQosTurnaroundPolicy::HighestPriorityOppositeOnTie,
        )
        .unwrap();

    assert_eq!(
        accesses
            .iter()
            .map(|access| (access.request(), access.kind()))
            .collect::<Vec<_>>(),
        vec![
            (high_priority_write.id(), DramAccessKind::Write),
            (high_priority_read.id(), DramAccessKind::Read),
            (low_priority_read.id(), DramAccessKind::Read),
        ]
    );
}

#[test]
fn dram_controller_qos_turnaround_burst_limit_switches_only_to_waiting_direction() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &write(0x0000, 30)).unwrap();
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let write_a = write_from(3, 0x0040, 31);
    let write_b = write_from(4, 0x0080, 32);
    let write_c = write_from(5, 0x00c0, 33);
    let read_waiting = read_from(6, 0x0100, 8, 34);
    let policy = DramQosSchedulingPolicy::new()
        .with_turnaround(DramQosTurnaroundPolicy::PreferCurrentDirection)
        .with_max_same_direction_burst(2)
        .unwrap();

    let accesses = controller
        .schedule_qos_batch_with_policy(
            8,
            [
                DramQosRequest::new(&write_a, QosPriority::new(0), 0),
                DramQosRequest::new(&write_b, QosPriority::new(0), 1),
                DramQosRequest::new(&write_c, QosPriority::new(0), 2),
                DramQosRequest::new(&read_waiting, QosPriority::new(0), 3),
            ],
            &mut arbiter,
            policy,
        )
        .unwrap();

    assert_eq!(
        accesses
            .iter()
            .map(|access| access.request())
            .collect::<Vec<_>>(),
        vec![write_a.id(), write_b.id(), read_waiting.id(), write_c.id()]
    );

    let mut write_only_controller = DramController::new(geometry(), timing());
    write_only_controller
        .schedule(0, &write(0x0000, 40))
        .unwrap();
    let mut write_only_arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let only_a = write_from(3, 0x0040, 41);
    let only_b = write_from(4, 0x0080, 42);
    let only_c = write_from(5, 0x00c0, 43);

    let write_only_accesses = write_only_controller
        .schedule_qos_batch_with_policy(
            8,
            [
                DramQosRequest::new(&only_a, QosPriority::new(0), 0),
                DramQosRequest::new(&only_b, QosPriority::new(0), 1),
                DramQosRequest::new(&only_c, QosPriority::new(0), 2),
            ],
            &mut write_only_arbiter,
            policy,
        )
        .unwrap();

    assert_eq!(
        write_only_accesses
            .iter()
            .map(|access| access.request())
            .collect::<Vec<_>>(),
        vec![only_a.id(), only_b.id(), only_c.id()]
    );
}

#[test]
fn dram_controller_qos_batch_can_escalate_requestor_priority() {
    let mut controller = DramController::new(geometry(), timing());
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let old_low = read_from(7, 0x0000, 8, 50);
    let other_mid = read_from(8, 0x0040, 8, 51);
    let new_high = read_from(7, 0x0100, 8, 52);

    let accesses = controller
        .schedule_qos_batch_with_policy(
            0,
            [
                DramQosRequest::new(&old_low, QosPriority::new(2), 0),
                DramQosRequest::new(&other_mid, QosPriority::new(1), 1),
                DramQosRequest::new(&new_high, QosPriority::new(0), 2),
            ],
            &mut arbiter,
            DramQosSchedulingPolicy::new()
                .with_priority_escalation()
                .with_turnaround(DramQosTurnaroundPolicy::RequestOrder),
        )
        .unwrap();

    assert_eq!(accesses.len(), 3);
    assert_eq!(accesses[0].request(), old_low.id());
    assert_eq!(accesses[0].command_cycle(), 3);
    assert_eq!(accesses[0].ready_cycle(), 8);
    assert_eq!(accesses[1].request(), new_high.id());
    assert!(accesses[1].row_hit());
    assert_eq!(accesses[1].command_cycle(), 8);
    assert_eq!(accesses[1].ready_cycle(), 13);
    assert_eq!(accesses[2].request(), other_mid.id());
    assert_eq!(accesses[2].command_cycle(), 8);
    assert_eq!(accesses[2].ready_cycle(), 13);

    let escalated = accesses[0].qos().unwrap();
    assert_eq!(escalated.requestor(), QosRequestorId::new(7));
    assert_eq!(escalated.assigned_priority(), QosPriority::new(2));
    assert_eq!(escalated.effective_priority(), QosPriority::new(0));
    assert_eq!(escalated.bytes(), 8);
    assert!(escalated.escalated());

    let profile = controller.activity_profile();
    assert_eq!(profile.qos_access_count(), 3);
    assert_eq!(profile.qos_byte_count(), 24);
    assert_eq!(profile.qos_escalated_access_count(), 1);
    assert_eq!(profile.qos_priority_access_count(QosPriority::new(0)), 2);
    assert_eq!(profile.qos_priority_byte_count(QosPriority::new(0)), 16);
    assert_eq!(profile.qos_priority_access_count(QosPriority::new(1)), 1);
    assert_eq!(
        profile.qos_requestor_access_count(QosRequestorId::new(7)),
        2
    );
    assert_eq!(profile.qos_requestor_byte_count(QosRequestorId::new(7)), 16);
}

#[test]
fn dram_controller_qos_batch_consumes_proportional_fair_priorities() {
    let mut controller = DramController::new(geometry(), timing());
    let mut arbiter = QosQueueArbiter::new(QosQueuePolicyKind::Fifo);
    let requestor_a = QosRequestorId::new(10);
    let requestor_b = QosRequestorId::new(20);
    let requestor_c = QosRequestorId::new(30);
    let mut policy = QosProportionalFairPolicy::new(3, 0.5)
        .unwrap()
        .with_requestor_score(requestor_a, 100.0)
        .unwrap()
        .with_requestor_score(requestor_b, 40.0)
        .unwrap()
        .with_requestor_score(requestor_c, 10.0)
        .unwrap();
    let first = read_from(10, 0x0000, 8, 60);
    let second = read_from(20, 0x0040, 8, 61);
    let third = read_from(30, 0x0080, 8, 62);

    let accesses = controller
        .schedule_qos_batch_with_policy(
            0,
            [
                DramQosRequest::from_proportional_fair_policy(&first, &mut policy, 0).unwrap(),
                DramQosRequest::from_proportional_fair_policy(&second, &mut policy, 1).unwrap(),
                DramQosRequest::from_proportional_fair_policy(&third, &mut policy, 2).unwrap(),
            ],
            &mut arbiter,
            DramQosSchedulingPolicy::new().with_turnaround(DramQosTurnaroundPolicy::RequestOrder),
        )
        .unwrap();

    assert_eq!(
        accesses
            .iter()
            .map(|access| access.request())
            .collect::<Vec<_>>(),
        vec![third.id(), second.id(), first.id()]
    );
    assert_eq!(
        accesses
            .iter()
            .map(|access| access.qos().unwrap().effective_priority())
            .collect::<Vec<_>>(),
        vec![
            QosPriority::new(0),
            QosPriority::new(1),
            QosPriority::new(2)
        ]
    );
    assert_eq!(policy.score_for(requestor_a), Some(13.5));
    assert_eq!(policy.score_for(requestor_b), Some(7.0));
    assert_eq!(policy.score_for(requestor_c), Some(5.25));
}

fn write(address: u64, sequence: u64) -> MemoryRequest {
    write_from(2, address, sequence)
}

fn write_from(agent: u32, address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id_from(agent, sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

#[test]
fn dram_controller_schedules_closed_row_read_with_activate_latency() {
    let mut controller = DramController::new(geometry(), timing());

    let access = controller.schedule(10, &read(0x0000, 8, 1)).unwrap();

    assert_eq!(access.kind(), DramAccessKind::Read);
    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 0);
    assert!(!access.row_hit());
    assert_eq!(access.arrival_cycle(), 10);
    assert_eq!(access.command_cycle(), 13);
    assert_eq!(access.ready_cycle(), 18);
    assert_eq!(access.commands().len(), 2);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[0].cycle(), 10);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Read);
    assert_eq!(access.commands()[1].cycle(), 13);
}

#[test]
fn dram_controller_enforces_burst_spacing_across_banks() {
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_burst_spacing(2)
        .unwrap();
    let mut controller = DramController::new(geometry(), timing);
    let marker = controller.mark_wait_for();

    let first = controller.schedule(0, &read(0x0000, 8, 50)).unwrap();
    let second = controller.schedule(0, &read(0x0040, 8, 51)).unwrap();

    assert_eq!(first.bank(), 0);
    assert_eq!(first.command_cycle(), 3);
    assert_eq!(first.ready_cycle(), 8);
    assert_eq!(second.bank(), 1);
    assert_eq!(second.command_cycle(), 5);
    assert_eq!(second.ready_cycle(), 10);

    let request = WaitForNode::transaction("dram.agent.2.request.51").unwrap();
    let bus = WaitForNode::resource("dram.port.0.bus").unwrap();
    let graph = controller.wait_for_graph_since(marker).snapshot();

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.first_observed_tick(), Some(3));
    assert_eq!(graph.last_observed_tick(), Some(4));
    assert!(graph.contains_edge(&request, &bus, WaitForEdgeKind::Resource));
}

#[test]
fn dram_controller_enforces_command_window_across_row_commands() {
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_command_window(10, 2)
        .unwrap();
    let mut controller = DramController::new(geometry(), timing);
    let marker = controller.mark_wait_for();

    let first = controller.schedule(0, &read(0x0000, 8, 52)).unwrap();
    let second = controller.schedule(0, &read(0x0040, 8, 53)).unwrap();

    assert_eq!(first.commands()[0].cycle(), 0);
    assert_eq!(first.command_cycle(), 3);
    assert_eq!(first.ready_cycle(), 8);
    assert_eq!(second.bank(), 1);
    assert_eq!(second.commands()[0].kind(), DramCommandKind::Activate);
    assert_eq!(second.commands()[0].cycle(), 10);
    assert_eq!(second.command_cycle(), 13);
    assert_eq!(second.ready_cycle(), 18);

    let request = WaitForNode::transaction("dram.agent.2.request.53").unwrap();
    let bus = WaitForNode::resource("dram.port.0.bus").unwrap();
    let graph = controller.wait_for_graph_since(marker).snapshot();

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.first_observed_tick(), Some(0));
    assert_eq!(graph.last_observed_tick(), Some(9));
    assert!(graph.contains_edge(&request, &bus, WaitForEdgeKind::Resource));
}

#[test]
fn dram_controller_uses_same_bank_group_burst_spacing() {
    let geometry = DramGeometry::new(4, 256, 64)
        .unwrap()
        .with_bank_groups(2)
        .unwrap();
    let timing = DramTiming::new(3, 5, 7, 2, 4)
        .unwrap()
        .with_burst_spacing(2)
        .unwrap()
        .with_same_bank_group_burst_spacing(6)
        .unwrap();
    let mut controller = DramController::new(geometry, timing);
    let marker = controller.mark_wait_for();

    let first = controller.schedule(0, &read(0x0000, 8, 54)).unwrap();
    let same_group = controller.schedule(0, &read(0x0080, 8, 55)).unwrap();
    let different_group = controller.schedule(0, &read(0x0040, 8, 56)).unwrap();

    assert_eq!(first.bank(), 0);
    assert_eq!(first.command_cycle(), 3);
    assert_eq!(same_group.bank(), 2);
    assert_eq!(same_group.command_cycle(), 9);
    assert_eq!(same_group.ready_cycle(), 14);
    assert_eq!(different_group.bank(), 1);
    assert_eq!(different_group.command_cycle(), 11);
    assert_eq!(different_group.ready_cycle(), 16);

    let request = WaitForNode::transaction("dram.agent.2.request.55").unwrap();
    let bus = WaitForNode::resource("dram.port.0.bus").unwrap();
    let graph = controller.wait_for_graph_since(marker).snapshot();

    assert!(graph.contains_edge(&request, &bus, WaitForEdgeKind::Resource));
    assert_eq!(graph.dependencies(&request)[0].first_observed_tick(), 3);
    assert_eq!(graph.dependencies(&request)[0].last_observed_tick(), 8);
}

#[test]
fn dram_controller_keeps_open_row_for_row_hits() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(1, &read(0x0100, 8, 2)).unwrap();

    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 0);
    assert!(access.row_hit());
    assert_eq!(access.command_cycle(), 8);
    assert_eq!(access.ready_cycle(), 13);
    assert_eq!(access.commands().len(), 1);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Read);
}

#[test]
fn dram_controller_precharges_on_row_conflict() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(8, &read(0x0400, 8, 2)).unwrap();

    assert_eq!(access.bank(), 0);
    assert_eq!(access.row(), 1);
    assert!(!access.row_hit());
    assert_eq!(access.command_cycle(), 13);
    assert_eq!(access.ready_cycle(), 18);
    assert_eq!(access.commands().len(), 3);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Precharge);
    assert_eq!(access.commands()[0].cycle(), 8);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[1].cycle(), 10);
    assert_eq!(access.commands()[2].kind(), DramCommandKind::Read);
    assert_eq!(access.commands()[2].cycle(), 13);
}

#[test]
fn dram_controller_enforces_read_write_turnaround_across_banks() {
    let mut controller = DramController::new(geometry(), timing());
    controller.schedule(0, &read(0x0000, 8, 1)).unwrap();

    let access = controller.schedule(0, &write(0x0040, 2)).unwrap();

    assert_eq!(access.kind(), DramAccessKind::Write);
    assert_eq!(access.bank(), 1);
    assert_eq!(access.row(), 0);
    assert_eq!(access.command_cycle(), 7);
    assert_eq!(access.ready_cycle(), 14);
    assert_eq!(access.commands()[0].kind(), DramCommandKind::Activate);
    assert_eq!(access.commands()[0].cycle(), 0);
    assert_eq!(access.commands()[1].kind(), DramCommandKind::Write);
    assert_eq!(access.commands()[1].cycle(), 7);
}

#[test]
fn dram_controller_reports_bank_port_and_window_activity() {
    let mut controller = DramController::new(geometry(), timing());
    let activity_start = controller.mark_activity();

    controller.schedule(0, &read(0x0000, 8, 10)).unwrap();
    controller.schedule(1, &read(0x0100, 8, 11)).unwrap();
    controller.schedule(0, &write(0x0040, 12)).unwrap();

    let profile = controller.activity_profile();
    assert_eq!(profile.active_port_count(), 1);
    assert_eq!(profile.active_bank_count(), 2);
    assert_eq!(profile.access_count(), 3);
    assert_eq!(profile.read_count(), 2);
    assert_eq!(profile.write_count(), 1);
    assert_eq!(profile.row_hit_count(), 1);
    assert_eq!(profile.row_miss_count(), 2);
    assert_eq!(profile.command_count(), 5);
    assert_eq!(profile.turnaround_count(), 1);
    assert_eq!(profile.total_ready_latency_cycles(), 39);
    assert_eq!(profile.max_ready_latency_cycles(), 19);
    assert!(profile.has_row_misses());
    assert_eq!(controller.activity_profile_since(activity_start), profile);

    let bank0 = controller.bank_activity(0, 0).unwrap();
    assert_eq!(bank0.access_count(), 2);
    assert_eq!(bank0.row_hit_count(), 1);
    assert_eq!(bank0.row_miss_count(), 1);
    assert_eq!(bank0.command_count(), 3);
    assert_eq!(bank0.first_arrival_cycle(), 0);
    assert_eq!(bank0.last_ready_cycle(), 13);
    assert_eq!(bank0.total_ready_latency_cycles(), 20);
    assert_eq!(bank0.max_ready_latency_cycles(), 12);

    let bank1 = controller.bank_activity(0, 1).unwrap();
    assert_eq!(bank1.access_count(), 1);
    assert_eq!(bank1.row_miss_count(), 1);
    assert_eq!(bank1.command_count(), 2);
    assert_eq!(bank1.total_ready_latency_cycles(), 19);

    let port = controller.port_activity(0).unwrap();
    assert_eq!(port.access_count(), 3);
    assert_eq!(port.read_count(), 2);
    assert_eq!(port.write_count(), 1);
    assert_eq!(port.turnaround_count(), 1);
    assert_eq!(port.command_count(), 5);

    controller.clear_activity();
    assert!(controller.activity_profile().is_empty());
}

#[test]
fn dram_controller_records_wait_for_edges_for_bank_and_port_contention() {
    let mut controller = DramController::new(geometry(), timing());
    let marker = controller.mark_wait_for();

    controller.schedule(0, &read(0x0000, 8, 20)).unwrap();
    controller.schedule(1, &read(0x0100, 8, 21)).unwrap();
    controller.schedule(0, &write(0x0040, 22)).unwrap();

    let request_waiting_for_bank = WaitForNode::transaction("dram.agent.2.request.21").unwrap();
    let bank = WaitForNode::resource("dram.port.0.bank.0").unwrap();
    let request_waiting_for_bus = WaitForNode::transaction("dram.agent.2.request.22").unwrap();
    let bus = WaitForNode::resource("dram.port.0.bus").unwrap();
    let graph = controller.wait_for_graph_since(marker).snapshot();

    assert_eq!(graph.edge_count(), 2);
    assert_eq!(graph.first_observed_tick(), Some(1));
    assert_eq!(graph.last_observed_tick(), Some(11));
    assert!(graph.contains_edge(&request_waiting_for_bank, &bank, WaitForEdgeKind::Queue));
    assert!(graph.contains_edge(&request_waiting_for_bus, &bus, WaitForEdgeKind::Resource));
    assert_eq!(
        graph.dependencies(&request_waiting_for_bank)[0].last_observed_tick(),
        7
    );
    assert_eq!(
        graph.dependencies(&request_waiting_for_bus)[0].last_observed_tick(),
        11
    );
}

#[test]
fn dram_controller_rejects_invalid_geometry_and_line_mismatch() {
    assert_eq!(
        DramGeometry::new(0, 256, 64).unwrap_err(),
        DramError::ZeroBankCount
    );
    assert_eq!(
        DramGeometry::new(4, 0, 64).unwrap_err(),
        DramError::ZeroRowSize
    );
    assert_eq!(
        DramGeometry::new(4, 256, 0).unwrap_err(),
        DramError::ZeroLineSize
    );
    assert_eq!(
        DramGeometry::new(4, 96, 64).unwrap_err(),
        DramError::RowSizeNotLineMultiple {
            row_size: 96,
            line_size: 64,
        }
    );
    assert_eq!(
        DramQosSchedulingPolicy::new()
            .with_max_same_direction_burst(0)
            .unwrap_err(),
        DramError::ZeroQosDirectionBurst
    );

    let mut controller = DramController::new(geometry(), timing());
    let actual = CacheLineLayout::new(128).unwrap();
    let request = MemoryRequest::read_shared(
        request_id(3),
        Address::new(0x0000),
        AccessSize::new(8).unwrap(),
        actual,
    )
    .unwrap();
    assert_eq!(
        controller.schedule(0, &request).unwrap_err(),
        DramError::LineSizeMismatch {
            request: request.id(),
            expected: 64,
            actual: 128,
        }
    );
}

#[test]
fn dram_controller_rejects_requests_crossing_decoded_rows() {
    let mut controller = DramController::new(DramGeometry::new(1, 64, 64).unwrap(), timing());
    let request = MemoryRequest::read_shared(
        request_id(4),
        Address::new(0x0030),
        AccessSize::new(64).unwrap(),
        layout(),
    )
    .unwrap();

    assert_eq!(
        controller.schedule(0, &request).unwrap_err(),
        DramError::RequestCrossesRow {
            request: request.id(),
            start_bank: 0,
            start_row: 0,
            end_bank: 0,
            end_row: 1,
        }
    );
}
