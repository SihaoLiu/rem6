use super::*;

pub(super) fn source_producers(
    runtime: &O3RuntimeState,
    consumer_index: usize,
    sources: &[O3ArchitecturalRegister],
) -> Vec<O3LiveIssueSourceProducer> {
    let mut producers = Vec::new();
    for source in sources.iter().copied().filter(|source| {
        source.register_class() != O3RegisterClass::Integer || source.architectural() != 0
    }) {
        let producer = runtime.snapshot.reorder_buffer[..consumer_index]
            .iter()
            .rev()
            .copied()
            .find(|producer| {
                producer.is_live_staged()
                    && producer.rename_destination()
                        == Some((source.register_class(), source.architectural()))
            });
        if let Some(producer) = producer {
            let producer = O3LiveIssueSourceProducer {
                sequence: producer.sequence(),
                source,
            };
            if !producers.contains(&producer) {
                producers.push(producer);
            }
        }
    }
    producers
}
