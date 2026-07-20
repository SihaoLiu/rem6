use rem6_memory::{AgentId, MemoryRequestId, ResponseStatus};
use rem6_transport::{MemoryRouteId, MemoryTraceEvent, TransportEndpointId};

use super::*;

#[test]
fn duplicate_final_memory_response_is_rejected() {
    let route = MemoryRouteId::new(0);
    let request = MemoryRequestId::new(AgentId::new(0), 7);
    let source = TransportEndpointId::new("gpu.global").unwrap();
    let trace = MemoryTrace::from_events(vec![
        MemoryTraceEvent::request(
            10,
            route,
            source.clone(),
            MemoryTraceKind::RequestSent,
            request,
        ),
        MemoryTraceEvent::response(
            12,
            route,
            source.clone(),
            request,
            ResponseStatus::Completed,
        ),
        MemoryTraceEvent::response(13, route, source, request, ResponseStatus::Completed),
    ]);
    let mut activity = vec![Rem6GpuComputeUnitActivity::new(0)];

    let error = record_gpu_compute_unit_memory_transport(&mut activity, &trace).unwrap_err();

    assert!(matches!(
        error,
        Rem6CliError::Execute { error }
            if error.contains("received more than one final response")
    ));
}
