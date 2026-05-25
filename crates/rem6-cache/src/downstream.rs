use rem6_memory::MemoryRequest;

pub(crate) fn with_source_ordering(
    downstream: MemoryRequest,
    source: &MemoryRequest,
) -> MemoryRequest {
    downstream.with_ordering(source.ordering())
}
