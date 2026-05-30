use rem6_memory::MemoryRequest;

pub(crate) fn with_source_attributes(
    downstream: MemoryRequest,
    source: &MemoryRequest,
) -> MemoryRequest {
    let downstream = downstream.with_ordering(source.ordering());
    if source.is_uncacheable() {
        downstream.with_uncacheable_strict_order()
    } else {
        downstream
    }
}
