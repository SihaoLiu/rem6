use super::super::queue::O3LiveIssueSourceProducer;
use super::*;
use crate::o3_dependency::O3RegisterClass;

pub(super) const O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceClass {
    ScalarInteger,
    IntegerMulDiv,
    MemoryAgu,
    Control,
    ScalarFloat,
    VectorToScalar,
}

impl O3LiveIssueTraceClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ScalarInteger => "scalar_integer",
            Self::IntegerMulDiv => "integer_mul_div",
            Self::MemoryAgu => "memory_agu",
            Self::Control => "control",
            Self::ScalarFloat => "scalar_float",
            Self::VectorToScalar => "vector_to_scalar",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceAction {
    Queued,
    Selected,
    RetainedResource,
    RetainedDependency,
    Replayed,
    Squashed,
    Retired,
}

impl O3LiveIssueTraceAction {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Selected => "selected",
            Self::RetainedResource => "retained_resource",
            Self::RetainedDependency => "retained_dependency",
            Self::Replayed => "replayed",
            Self::Squashed => "squashed",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LiveIssueTraceDataProducer {
    sequence: u64,
    register_class: O3RegisterClass,
    architectural: u32,
}

impl O3LiveIssueTraceDataProducer {
    pub(in crate::o3_runtime) const fn new(
        sequence: u64,
        register_class: O3RegisterClass,
        architectural: u32,
    ) -> Self {
        Self {
            sequence,
            register_class,
            architectural,
        }
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub const fn architectural(self) -> u32 {
        self.architectural
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LiveIssueTraceRecord {
    sequence: u64,
    pc: Address,
    action: O3LiveIssueTraceAction,
    issue_class: O3LiveIssueTraceClass,
    data_producers: [Option<O3LiveIssueTraceDataProducer>; O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS],
    service_tick: u64,
    next_wake_tick: Option<u64>,
    raw_writeback_tick: Option<u64>,
    admitted_writeback_tick: Option<u64>,
    cleanup_boundary: Option<u64>,
}

impl O3LiveIssueTraceRecord {
    copy_getters!(sequence -> u64, pc -> Address);
    copy_getters!(action -> O3LiveIssueTraceAction, issue_class -> O3LiveIssueTraceClass);
    copy_getters!(service_tick -> u64, next_wake_tick -> Option<u64>);
    copy_getters!(raw_writeback_tick -> Option<u64>);
    copy_getters!(admitted_writeback_tick -> Option<u64>, cleanup_boundary -> Option<u64>);

    pub fn data_producers(&self) -> impl Iterator<Item = O3LiveIssueTraceDataProducer> + '_ {
        self.data_producers.iter().copied().flatten()
    }

    pub(super) const fn without_data_producers(
        sequence: u64,
        pc: Address,
        action: O3LiveIssueTraceAction,
        issue_class: O3LiveIssueTraceClass,
        service_tick: u64,
        next_wake_tick: Option<u64>,
        cleanup_boundary: Option<u64>,
    ) -> Self {
        Self {
            sequence,
            pc,
            action,
            issue_class,
            data_producers: [None; O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS],
            service_tick,
            next_wake_tick,
            raw_writeback_tick: None,
            admitted_writeback_tick: None,
            cleanup_boundary,
        }
    }

    pub(super) fn set_writeback_ticks(&mut self, raw: u64, admitted: u64) {
        self.raw_writeback_tick = Some(raw);
        self.admitted_writeback_tick = Some(admitted);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueTraceRow {
    sequence: u64,
    pc: Address,
    issue_class: O3LiveIssueTraceClass,
    data_producers: [Option<O3LiveIssueTraceDataProducer>; O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS],
}

impl O3LiveIssueTraceRow {
    pub(in crate::o3_runtime) fn new(
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        data_producers: &[O3LiveIssueSourceProducer],
    ) -> Option<Self> {
        if data_producers.len() > O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS {
            return None;
        }
        let mut producers = [None; O3_LIVE_ISSUE_TRACE_DATA_PRODUCER_SLOTS];
        for (slot, producer) in producers.iter_mut().zip(data_producers.iter().copied()) {
            let source = producer.source();
            *slot = Some(O3LiveIssueTraceDataProducer::new(
                producer.sequence(),
                source.register_class(),
                source.architectural(),
            ));
        }
        Some(Self {
            sequence,
            pc,
            issue_class,
            data_producers: producers,
        })
    }
}

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn finalize_service_turn_trace(
        &mut self,
        selected_sequences: &[u64],
        resource_blocked: &[O3LiveIssueTraceRow],
        dependency_blocked: &[O3LiveIssueTraceRow],
        service_tick: u64,
        next_wake_tick: Option<u64>,
    ) {
        for &sequence in selected_sequences {
            let selected = self.trace_records.iter_mut().rev().find(|record| {
                record.sequence == sequence
                    && record.action == O3LiveIssueTraceAction::Selected
                    && record.service_tick == service_tick
            });
            debug_assert!(selected.is_some(), "selected row must own a trace record");
            if let Some(selected) = selected {
                selected.next_wake_tick = next_wake_tick;
            }
        }
        self.append_retained_trace(
            resource_blocked,
            O3LiveIssueTraceAction::RetainedResource,
            service_tick,
            next_wake_tick,
        );
        self.append_retained_trace(
            dependency_blocked,
            O3LiveIssueTraceAction::RetainedDependency,
            service_tick,
            next_wake_tick,
        );
    }

    fn append_retained_trace(
        &mut self,
        rows: &[O3LiveIssueTraceRow],
        action: O3LiveIssueTraceAction,
        service_tick: u64,
        next_wake_tick: Option<u64>,
    ) {
        self.trace_records
            .extend(rows.iter().map(|row| O3LiveIssueTraceRecord {
                sequence: row.sequence,
                pc: row.pc,
                action,
                issue_class: row.issue_class,
                data_producers: row.data_producers,
                service_tick,
                next_wake_tick,
                raw_writeback_tick: None,
                admitted_writeback_tick: None,
                cleanup_boundary: None,
            }));
    }
}
