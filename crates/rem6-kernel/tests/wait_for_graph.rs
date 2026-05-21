use rem6_kernel::{PartitionId, WaitForEdgeKind, WaitForGraph, WaitForGraphError, WaitForNode};

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
