use super::*;

#[test]
fn checkpoint_and_transfer_totals_follow_owned_projections() {
    let checkpoint_chunks = vec![chunk("pipe_0", 3), chunk("pipe_1", 5)];
    let expected_component_chunks = checkpoint_chunks.len() as u64;
    let checkpoint_component = component("cpu0", checkpoint_chunks);

    assert_eq!(
        checkpoint_component.chunk_count(),
        expected_component_chunks
    );
    assert_eq!(checkpoint_component.payload_bytes(), 8);

    let checkpoint_components = vec![checkpoint_component.clone()];
    let expected_checkpoint_components = checkpoint_components.len() as u64;
    let checkpoint_summary = checkpoint("restore-a", checkpoint_components);

    assert_eq!(
        checkpoint_summary.component_count(),
        expected_checkpoint_components
    );
    assert_eq!(checkpoint_summary.chunk_count(), 2);
    assert_eq!(checkpoint_summary.payload_bytes(), 8);

    let transfer_components = vec![component("cpu1", vec![chunk("handoff", 7)])];
    let expected_transfer_components = transfer_components.len() as u64;
    let transfer = transfer(transfer_components);

    assert_eq!(transfer.component_count(), expected_transfer_components);
    assert_eq!(transfer.chunk_count(), 1);
    assert_eq!(transfer.payload_bytes(), 7);

    let summary = Rem6HostActionSummary {
        checkpoint_restores: vec![
            checkpoint_summary,
            checkpoint(
                "restore-b",
                vec![component("cpu1", vec![chunk("handoff", 7)])],
            ),
        ],
        ..Rem6HostActionSummary::default()
    };

    assert_eq!(summary.checkpoint_restored_count(), 2);
    assert_eq!(summary.checkpoint_restored_component_count(), 2);
    assert_eq!(summary.checkpoint_restored_chunk_count(), 3);
    assert_eq!(summary.checkpoint_restored_payload_bytes(), 15);
}

fn chunk(name: &str, payload_bytes: u64) -> Rem6HostCheckpointChunkSummary {
    Rem6HostCheckpointChunkSummary {
        name: name.to_string(),
        payload_bytes,
        payload_checksum: payload_bytes,
        o3_runtime: None,
        o3_live_data_handoff: None,
    }
}

fn component(
    component: &str,
    chunks: Vec<Rem6HostCheckpointChunkSummary>,
) -> Rem6HostCheckpointComponentSummary {
    Rem6HostCheckpointComponentSummary {
        component: component.to_string(),
        chunks,
    }
}

fn checkpoint(
    label: &str,
    components: Vec<Rem6HostCheckpointComponentSummary>,
) -> Rem6HostCheckpointSummary {
    Rem6HostCheckpointSummary {
        tick: 11,
        event: 13,
        source: 17,
        label: label.to_string(),
        manifest_tick: 19,
        execution_mode_authority_present: false,
        execution_mode_authority_cleared: false,
        execution_mode_authority_decode_error: false,
        execution_modes: Vec::new(),
        components,
    }
}

fn transfer(
    components: Vec<Rem6HostCheckpointComponentSummary>,
) -> Rem6ExecutionModeStateTransferSummary {
    let captured_component_count = components.len() as u64;
    let captured_chunk_count = components
        .iter()
        .map(|component| component.chunks.len() as u64)
        .sum();
    let captured_payload_bytes = components
        .iter()
        .flat_map(|component| component.chunks.iter())
        .map(|chunk| chunk.payload_bytes)
        .sum();

    Rem6ExecutionModeStateTransferSummary {
        manifest_label: "transfer-a".to_string(),
        manifest_tick: 23,
        restorable: true,
        live_data_handoff: true,
        writeback_width: None,
        reserved_future_completions: None,
        earliest_unpublished_writeback_tick: None,
        quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary {
            validated: true,
            target: "cpu1".to_string(),
            captured_component_count,
            captured_chunk_count,
            captured_payload_bytes,
            checker: None,
        },
        components,
    }
}
