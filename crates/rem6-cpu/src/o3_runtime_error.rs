use std::error::Error;
use std::fmt;

use crate::o3_dependency::{O3PhysicalRegisterId, O3RegisterClass};
use crate::o3_pipeline::O3PipelineError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum O3RuntimeError {
    DuplicateReorderBufferSequence {
        sequence: u64,
    },
    DuplicateLoadStoreQueueSequence {
        sequence: u64,
    },
    DuplicateRenameMapEntry {
        register_class: O3RegisterClass,
        architectural: u32,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    InvalidRegisterClassCode {
        code: u8,
    },
    InvalidLoadStoreKindCode {
        code: u8,
    },
    InvalidCheckpointBool {
        field: &'static str,
        value: u8,
    },
    InvalidLiveStagedReorderBufferMetadata {
        sequence: u64,
        destination_present: bool,
        live_staged: bool,
        rename_destination_present: bool,
    },
    InvalidReorderBufferPhysicalRegister {
        sequence: u64,
    },
    DuplicateReorderBufferPhysicalRegister {
        physical: O3PhysicalRegisterId,
    },
    LiveStagedPhysicalRegisterAlreadyCommitted {
        sequence: u64,
        physical: O3PhysicalRegisterId,
    },
    InvalidPendingState {
        error: O3PipelineError,
    },
    DuplicateWritebackReadySequence {
        sequence: u64,
    },
    WritebackReservationMismatch {
        sequence: u64,
        existing_raw_ready_tick: u64,
        requested_raw_ready_tick: u64,
    },
    WritebackReservationSourceMismatch {
        sequence: u64,
        existing_source: &'static str,
        requested_source: &'static str,
    },
    WritebackReservationTickClosed {
        sequence: u64,
        raw_ready_tick: u64,
        closed_before_tick: u64,
    },
    WritebackOwnerReservationMismatch {
        sequence: u64,
        owner: &'static str,
        owner_raw_ready_tick: u64,
        reservation_raw_ready_tick: u64,
    },
    WritebackOwnerSourceMismatch {
        sequence: u64,
        owner: &'static str,
        reservation_source: &'static str,
    },
    WritebackOwnerMissing {
        sequence: u64,
        owner: &'static str,
    },
    WritebackOwnerMissingRawReadyTick {
        sequence: u64,
        owner: &'static str,
    },
    WritebackStatisticsUnderflow {
        counter: &'static str,
        current: u64,
        removed: u64,
    },
    WritebackStatisticsOverflow {
        counter: &'static str,
    },
    WritebackCalendarSlotOccupied {
        tick: u64,
        slot: usize,
    },
    StableWritebackQueueNotEmpty {
        deferred: usize,
    },
    WritebackTickOverflow {
        tick: u64,
    },
    WritebackClosureTickOverflow {
        tick: u64,
    },
    CheckpointValueTooLarge {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
}

impl fmt::Display for O3RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateReorderBufferSequence { sequence } => {
                write!(formatter, "O3 runtime ROB repeats sequence {sequence}")
            }
            Self::DuplicateLoadStoreQueueSequence { sequence } => {
                write!(formatter, "O3 runtime LSQ repeats sequence {sequence}")
            }
            Self::DuplicateRenameMapEntry {
                register_class,
                architectural,
            } => write!(
                formatter,
                "O3 runtime rename map repeats {register_class:?} architectural register {architectural}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "O3 runtime checkpoint payload has {actual} bytes but expected {expected}"
            ),
            Self::InvalidCheckpointMagic => {
                write!(formatter, "O3 runtime checkpoint payload has invalid magic")
            }
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "O3 runtime checkpoint payload version {version} is not supported"
            ),
            Self::InvalidRegisterClassCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid register-class code {code}"
            ),
            Self::InvalidLoadStoreKindCode { code } => write!(
                formatter,
                "O3 runtime checkpoint payload has invalid LSQ kind code {code}"
            ),
            Self::InvalidCheckpointBool { field, value } => write!(
                formatter,
                "O3 runtime checkpoint field {field} boolean has invalid value {value}"
            ),
            Self::InvalidLiveStagedReorderBufferMetadata {
                sequence,
                destination_present,
                live_staged,
                rename_destination_present,
            } => write!(
                formatter,
                "O3 runtime live-staged ROB metadata for sequence {sequence} is inconsistent: destination_present={destination_present}, live_staged={live_staged}, rename_destination_present={rename_destination_present}"
            ),
            Self::InvalidReorderBufferPhysicalRegister { sequence } => write!(
                formatter,
                "O3 runtime ROB sequence {sequence} uses an invalid physical register"
            ),
            Self::DuplicateReorderBufferPhysicalRegister { physical } => write!(
                formatter,
                "O3 runtime ROB repeats physical register {}",
                physical.get()
            ),
            Self::LiveStagedPhysicalRegisterAlreadyCommitted { sequence, physical } => write!(
                formatter,
                "O3 runtime live-staged ROB sequence {sequence} uses physical register {} that is already committed",
                physical.get()
            ),
            Self::InvalidPendingState { error } => {
                write!(formatter, "O3 runtime checkpoint has invalid pending state: {error}")
            }
            Self::DuplicateWritebackReadySequence { sequence } => {
                write!(formatter, "O3 runtime writeback ready row repeats sequence {sequence}")
            }
            Self::WritebackReservationMismatch {
                sequence,
                existing_raw_ready_tick,
                requested_raw_ready_tick,
            } => write!(
                formatter,
                "O3 runtime writeback reservation for sequence {sequence} has raw-ready tick {existing_raw_ready_tick} but was requested at {requested_raw_ready_tick}"
            ),
            Self::WritebackReservationSourceMismatch {
                sequence,
                existing_source,
                requested_source,
            } => write!(
                formatter,
                "O3 runtime writeback reservation for sequence {sequence} has source {existing_source} but was requested as {requested_source}"
            ),
            Self::WritebackReservationTickClosed {
                sequence,
                raw_ready_tick,
                closed_before_tick,
            } => write!(
                formatter,
                "O3 runtime writeback reservation for sequence {sequence} has raw-ready tick {raw_ready_tick} below closed watermark {closed_before_tick}"
            ),
            Self::WritebackOwnerReservationMismatch {
                sequence,
                owner,
                owner_raw_ready_tick,
                reservation_raw_ready_tick,
            } => write!(
                formatter,
                "O3 runtime {owner} owner for sequence {sequence} has raw-ready tick {owner_raw_ready_tick} but reservation has {reservation_raw_ready_tick}"
            ),
            Self::WritebackOwnerSourceMismatch {
                sequence,
                owner,
                reservation_source,
            } => write!(
                formatter,
                "O3 runtime {owner} owner for sequence {sequence} cannot use {reservation_source} writeback reservation source"
            ),
            Self::WritebackOwnerMissing { sequence, owner } => write!(
                formatter,
                "O3 runtime writeback reservation for sequence {sequence} is missing {owner} owner metadata"
            ),
            Self::WritebackOwnerMissingRawReadyTick { sequence, owner } => write!(
                formatter,
                "O3 runtime {owner} owner for sequence {sequence} is missing raw-ready tick metadata"
            ),
            Self::WritebackStatisticsUnderflow {
                counter,
                current,
                removed,
            } => write!(
                formatter,
                "O3 runtime writeback statistic {counter} cannot remove {removed} from {current}"
            ),
            Self::WritebackStatisticsOverflow { counter } => write!(
                formatter,
                "O3 runtime writeback statistic {counter} overflowed"
            ),
            Self::WritebackCalendarSlotOccupied { tick, slot } => write!(
                formatter,
                "O3 runtime writeback calendar tick {tick} slot {slot} is already occupied"
            ),
            Self::StableWritebackQueueNotEmpty { deferred } => write!(
                formatter,
                "O3 runtime writeback reservation requires an empty stable deferred queue but found {deferred} rows"
            ),
            Self::WritebackTickOverflow { tick } => write!(
                formatter,
                "O3 runtime writeback reservation tick overflowed after {tick}"
            ),
            Self::WritebackClosureTickOverflow { tick } => write!(
                formatter,
                "O3 runtime writeback closure watermark overflowed after tick {tick}"
            ),
            Self::CheckpointValueTooLarge {
                field,
                value,
                maximum,
            } => write!(
                formatter,
                "O3 runtime checkpoint field {field} value {value} exceeds maximum {maximum}"
            ),
        }
    }
}

impl Error for O3RuntimeError {}
