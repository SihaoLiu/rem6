use rem6_kernel::{
    PartitionId, WaitForBlockedNodeWindow, WaitForEdgeKind, WaitForEdgeKindWindow, WaitForGraph,
    WaitForGraphError, WaitForNode,
};

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn resource(name: &str) -> WaitForNode {
    WaitForNode::resource(name).unwrap()
}

fn transaction(name: &str) -> WaitForNode {
    WaitForNode::transaction(name).unwrap()
}

#[test]
fn wait_for_graph_tracks_checkpoint_barrier_waits_and_release() {
    let first_partition = WaitForNode::partition(PartitionId::new(0));
    let second_partition = WaitForNode::partition(PartitionId::new(1));
    let barrier = WaitForNode::checkpoint_barrier("boot-cpt").unwrap();
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(
            first_partition.clone(),
            barrier.clone(),
            WaitForEdgeKind::Barrier,
            21,
        )
        .unwrap();
    graph
        .record_wait(
            first_partition.clone(),
            barrier.clone(),
            WaitForEdgeKind::Barrier,
            25,
        )
        .unwrap();
    graph
        .record_wait(
            second_partition.clone(),
            barrier.clone(),
            WaitForEdgeKind::Barrier,
            23,
        )
        .unwrap();

    let snapshot = graph.snapshot();

    assert_eq!(snapshot.edge_count(), 2);
    assert_eq!(snapshot.edge_count_by_kind(WaitForEdgeKind::Barrier), 2);
    assert_eq!(snapshot.first_observed_tick(), Some(21));
    assert_eq!(snapshot.last_observed_tick(), Some(25));
    assert_eq!(snapshot.total_observation_count(), 3);
    assert_eq!(snapshot.longest_observed_span(), Some(4));
    assert_eq!(
        snapshot.blocked_nodes(),
        vec![first_partition.clone(), second_partition]
    );
    assert_eq!(graph.clear_waits_to(&barrier), 2);
    assert!(graph.is_empty());
    assert_eq!(
        WaitForNode::checkpoint_barrier("bad barrier"),
        Err(WaitForGraphError::InvalidNodeLabel {
            label: "bad barrier".to_string(),
        })
    );
}

#[test]
fn wait_for_graph_reports_deterministic_deadlock_cycle() {
    let cache = component("l1d0");
    let directory = component("dir0");
    let memory = resource("mem0.bank0");
    let transaction = transaction("miss.7.4");
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(
            cache.clone(),
            directory.clone(),
            WaitForEdgeKind::Protocol,
            5,
        )
        .unwrap();
    graph
        .record_wait(
            directory.clone(),
            memory.clone(),
            WaitForEdgeKind::Resource,
            8,
        )
        .unwrap();
    graph
        .record_wait(
            memory.clone(),
            transaction.clone(),
            WaitForEdgeKind::Message,
            11,
        )
        .unwrap();
    graph
        .record_wait(
            transaction.clone(),
            cache.clone(),
            WaitForEdgeKind::Queue,
            13,
        )
        .unwrap();

    let diagnostic = graph.deadlock_diagnostic().unwrap();
    assert_eq!(
        diagnostic.cycle_nodes(),
        &[
            directory.clone(),
            memory.clone(),
            transaction.clone(),
            cache,
            directory
        ]
    );
    assert_eq!(diagnostic.edge_count(), 4);
    assert_eq!(
        diagnostic.edge_kinds(),
        &[
            WaitForEdgeKind::Resource,
            WaitForEdgeKind::Message,
            WaitForEdgeKind::Queue,
            WaitForEdgeKind::Protocol,
        ]
    );
    assert_eq!(diagnostic.first_observed_tick(), 5);
    assert_eq!(diagnostic.last_observed_tick(), 13);

    let snapshot = graph.snapshot();
    assert_eq!(snapshot.edge_count(), 4);
    assert!(snapshot.has_edges());
    assert_eq!(snapshot.first_observed_tick(), Some(5));
    assert_eq!(snapshot.last_observed_tick(), Some(13));
    assert_eq!(snapshot.deadlock_diagnostic(), Some(&diagnostic));
    assert!(snapshot.has_deadlock());
}

#[test]
fn wait_for_graph_updates_edges_and_clears_resolved_waits() {
    let core = WaitForNode::partition(PartitionId::new(0));
    let queue = resource("l1d0.mshr");
    let memory = component("mem0");
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(core.clone(), queue.clone(), WaitForEdgeKind::Queue, 10)
        .unwrap();
    graph
        .record_wait(core.clone(), queue.clone(), WaitForEdgeKind::Queue, 14)
        .unwrap();
    graph
        .record_wait(queue.clone(), memory.clone(), WaitForEdgeKind::Resource, 16)
        .unwrap();

    let dependencies = graph.dependencies(&core);
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].source(), &core);
    assert_eq!(dependencies[0].target(), &queue);
    assert_eq!(dependencies[0].first_observed_tick(), 10);
    assert_eq!(dependencies[0].last_observed_tick(), 14);
    assert_eq!(dependencies[0].observation_count(), 2);
    assert_eq!(graph.blocked_nodes(), vec![core.clone(), queue.clone()]);

    assert_eq!(graph.deadlock_diagnostic(), None);
    assert_eq!(graph.clear_waits_from(&core), 1);
    assert!(graph.dependencies(&core).is_empty());
    assert_eq!(graph.blocked_nodes(), vec![queue]);
}

#[test]
fn wait_for_graph_exposes_deterministic_edge_snapshot() {
    let core = WaitForNode::partition(PartitionId::new(0));
    let queue = resource("l1d0.mshr");
    let memory = component("mem0");
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(queue.clone(), memory.clone(), WaitForEdgeKind::Resource, 16)
        .unwrap();
    graph
        .record_wait(core.clone(), queue.clone(), WaitForEdgeKind::Queue, 10)
        .unwrap();
    graph
        .record_wait(core.clone(), queue.clone(), WaitForEdgeKind::Queue, 14)
        .unwrap();

    let edges = graph.edges();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].source(), &core);
    assert_eq!(edges[0].target(), &queue);
    assert_eq!(edges[0].kind(), WaitForEdgeKind::Queue);
    assert_eq!(edges[0].first_observed_tick(), 10);
    assert_eq!(edges[0].last_observed_tick(), 14);
    assert_eq!(edges[0].observation_count(), 2);
    assert_eq!(edges[1].source(), &queue);
    assert_eq!(edges[1].target(), &memory);
    assert_eq!(edges[1].kind(), WaitForEdgeKind::Resource);
    assert_eq!(edges[1].first_observed_tick(), 16);
    assert_eq!(edges[1].last_observed_tick(), 16);
    assert_eq!(edges[1].observation_count(), 1);

    let snapshot = graph.snapshot();
    assert_eq!(snapshot.edges(), edges.as_slice());
    assert_eq!(snapshot.edge_count(), 2);
    assert!(snapshot.has_edges());
    assert!(!snapshot.is_empty());
    assert_eq!(snapshot.first_observed_tick(), Some(10));
    assert_eq!(snapshot.last_observed_tick(), Some(16));
    assert_eq!(snapshot.deadlock_diagnostic(), None);
    assert!(!snapshot.has_deadlock());
    assert_eq!(snapshot.blocked_nodes(), vec![core.clone(), queue.clone()]);
    assert_eq!(snapshot.dependencies(&core), vec![edges[0].clone()]);
    assert_eq!(snapshot.dependents(&queue), vec![edges[0].clone()]);
    assert!(snapshot.contains_edge(&core, &queue, WaitForEdgeKind::Queue));
    assert!(!snapshot.contains_edge(&core, &memory, WaitForEdgeKind::Message));
    assert_eq!(snapshot.edge_count_by_kind(WaitForEdgeKind::Queue), 1);
    assert_eq!(snapshot.edge_count_by_kind(WaitForEdgeKind::Resource), 1);
    assert_eq!(snapshot.edge_count_by_kind(WaitForEdgeKind::Message), 0);
    assert_eq!(
        snapshot
            .edge_kind_counts()
            .get(&WaitForEdgeKind::Queue)
            .copied(),
        Some(1)
    );
    assert_eq!(snapshot.oldest_wait_edge(), Some(&edges[0]));
    assert_eq!(snapshot.newest_observed_edge(), Some(&edges[1]));
    assert_eq!(snapshot.total_observation_count(), 3);
    assert_eq!(snapshot.longest_observed_span(), Some(4));
}

#[test]
fn wait_for_graph_summarizes_edge_kind_observation_windows() {
    let first_core = WaitForNode::partition(PartitionId::new(0));
    let second_core = WaitForNode::partition(PartitionId::new(1));
    let queue = resource("l1d0.mshr");
    let memory = component("mem0");
    let transaction = transaction("miss.7");
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(
            first_core.clone(),
            queue.clone(),
            WaitForEdgeKind::Queue,
            10,
        )
        .unwrap();
    graph
        .record_wait(first_core, queue.clone(), WaitForEdgeKind::Queue, 14)
        .unwrap();
    graph
        .record_wait(second_core, queue.clone(), WaitForEdgeKind::Queue, 12)
        .unwrap();
    graph
        .record_wait(queue.clone(), memory.clone(), WaitForEdgeKind::Resource, 7)
        .unwrap();
    graph
        .record_wait(
            memory.clone(),
            transaction.clone(),
            WaitForEdgeKind::Message,
            20,
        )
        .unwrap();
    graph
        .record_wait(memory, transaction, WaitForEdgeKind::Message, 23)
        .unwrap();

    let expected = vec![
        WaitForEdgeKindWindow::new(WaitForEdgeKind::Resource, 1, 7, 7),
        WaitForEdgeKindWindow::new(WaitForEdgeKind::Message, 1, 20, 23),
        WaitForEdgeKindWindow::new(WaitForEdgeKind::Queue, 2, 10, 14),
    ];
    let snapshot = graph.snapshot();

    assert_eq!(snapshot.edge_kind_windows(), expected);
    assert_eq!(graph.edge_kind_windows(), expected);
}

#[test]
fn wait_for_graph_summarizes_blocked_node_observation_windows() {
    let first_core = WaitForNode::partition(PartitionId::new(0));
    let second_core = WaitForNode::partition(PartitionId::new(1));
    let queue = resource("l1d0.mshr");
    let memory = component("mem0");
    let credit = resource("noc.credit.0");
    let mut graph = WaitForGraph::new();

    graph
        .record_wait(
            first_core.clone(),
            queue.clone(),
            WaitForEdgeKind::Queue,
            10,
        )
        .unwrap();
    graph
        .record_wait(
            first_core.clone(),
            queue.clone(),
            WaitForEdgeKind::Queue,
            18,
        )
        .unwrap();
    graph
        .record_wait(
            first_core.clone(),
            memory.clone(),
            WaitForEdgeKind::Resource,
            14,
        )
        .unwrap();
    graph
        .record_wait(
            second_core.clone(),
            credit.clone(),
            WaitForEdgeKind::Credit,
            12,
        )
        .unwrap();

    let expected = vec![
        WaitForBlockedNodeWindow::new(first_core, 2, 10, 18),
        WaitForBlockedNodeWindow::new(second_core, 1, 12, 12),
    ];
    let snapshot = graph.snapshot();

    assert_eq!(snapshot.blocked_node_windows(), expected);
    assert_eq!(graph.blocked_node_windows(), expected);
}

#[test]
fn wait_for_graph_rejects_invalid_nodes_and_self_waits_without_mutation() {
    let core = WaitForNode::partition(PartitionId::new(3));
    let mut graph = WaitForGraph::new();

    assert_eq!(
        WaitForNode::component(""),
        Err(WaitForGraphError::EmptyNodeLabel)
    );
    assert_eq!(
        WaitForNode::resource("dram bank"),
        Err(WaitForGraphError::InvalidNodeLabel {
            label: "dram bank".to_string(),
        })
    );
    assert_eq!(
        graph.record_wait(core.clone(), core, WaitForEdgeKind::Resource, 20),
        Err(WaitForGraphError::SelfWait)
    );
    assert!(graph.is_empty());
}
