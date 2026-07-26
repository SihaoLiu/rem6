use rem6_isa_riscv::{FloatRegisterWrite, RegisterWrite};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) enum O3LiveIssueForwardedValue {
    Integer(RegisterWrite),
    FloatingPoint(FloatRegisterWrite),
}

impl O3LiveIssueForwardedValue {
    pub(in crate::o3_runtime) fn architectural_register(&self) -> O3ArchitecturalRegister {
        match self {
            Self::Integer(write) => O3ArchitecturalRegister::integer(write.register()),
            Self::FloatingPoint(write) => O3ArchitecturalRegister::floating_point(write.register()),
        }
    }
}

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

pub(super) fn materialize_candidate(
    runtime: &O3RuntimeState,
    scheduling: &O3LiveIssueSchedulingCandidate,
) -> Option<O3LiveSpeculativeIssueCandidate> {
    let mut producer_sequences = Vec::new();
    let mut forwarded_values = Vec::new();
    let mut forwarded_ready_tick = 0;
    for producer in scheduling.data_producers.iter().copied() {
        let (value, ready_tick) =
            match runtime.live_issue_source_value(producer.sequence(), producer.source()) {
                Some((value, ready_tick)) => (Some(value), ready_tick),
                None if scheduling.is_pending_data_address() => {
                    let register = producer.source().integer_register()?;
                    let ready_tick = runtime.pending_data_address_committed_producer_ready_tick(
                        producer.sequence(),
                        register,
                    )?;
                    (None, ready_tick)
                }
                None => return None,
            };
        if !producer_sequences.contains(&producer.sequence()) {
            producer_sequences.push(producer.sequence());
        }
        if let Some(value) = value {
            if !forwarded_values
                .iter()
                .any(|forwarded: &O3LiveIssueForwardedValue| {
                    forwarded.architectural_register() == producer.source()
                })
            {
                forwarded_values.push(value);
            }
        }
        forwarded_ready_tick = forwarded_ready_tick.max(ready_tick);
    }
    if let Some(control_sequence) = scheduling.control_dependency {
        if !producer_sequences.contains(&control_sequence) {
            producer_sequences.push(control_sequence);
        }
    }
    Some(O3LiveSpeculativeIssueCandidate {
        scheduling: scheduling.clone(),
        producer_sequences,
        forwarded_values,
        forwarded_ready_tick,
    })
}
