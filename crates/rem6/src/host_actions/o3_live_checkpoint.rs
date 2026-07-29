use rem6_cpu::{
    RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile, RISCV_O3_LIVE_CHECKPOINT_CHUNK,
};
use rem6_kernel::ScheduledEventKind;

use super::{Rem6HostO3RuntimeCheckpointStatAggregation, Rem6HostO3RuntimeCheckpointStatValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6HostO3LiveCheckpointChunkSummary {
    pub(crate) decode_error: bool,
    pub(crate) version: Option<u64>,
    pub(crate) profile: Option<&'static str>,
    pub(crate) payload_bytes: u64,
    pub(crate) event_count: Option<u64>,
    pub(crate) resident_rows: Option<u64>,
    pub(crate) writeback_reservations: Option<u64>,
    pub(crate) wake_partition: Option<u64>,
    pub(crate) wake_tick: Option<u64>,
    pub(crate) wake_kind: Option<&'static str>,
    pub(crate) rebound_wakes: u64,
}

impl Rem6HostO3LiveCheckpointChunkSummary {
    fn decoded(
        payload_bytes: u64,
        wire_version: u8,
        payload: &RiscvO3LiveCheckpointPayload,
        wake_rebound: bool,
    ) -> Self {
        Self {
            decode_error: false,
            version: Some(u64::from(wire_version)),
            profile: Some(match payload.profile {
                RiscvO3LiveCheckpointProfile::ComputeQueue => "compute_queue",
                RiscvO3LiveCheckpointProfile::CompletedFpLoad => "completed_fp_load",
                RiscvO3LiveCheckpointProfile::PendingDataAddress => "pending_data_address",
            }),
            payload_bytes,
            event_count: Some(payload.events.len() as u64),
            resident_rows: Some(payload.resident_sequences.len() as u64),
            writeback_reservations: Some(u64::from(payload.reservation.is_some())),
            wake_partition: Some(u64::from(payload.wake.partition.index())),
            wake_tick: Some(payload.wake.tick),
            wake_kind: Some(match payload.wake.kind {
                ScheduledEventKind::Serial => "serial",
                ScheduledEventKind::Parallel => "parallel",
            }),
            rebound_wakes: u64::from(wake_rebound),
        }
    }

    fn decode_error(payload_bytes: u64) -> Self {
        Self {
            decode_error: true,
            version: None,
            profile: None,
            payload_bytes,
            event_count: None,
            resident_rows: None,
            writeback_reservations: None,
            wake_partition: None,
            wake_tick: None,
            wake_kind: None,
            rebound_wakes: 0,
        }
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"decode_error\":{},\"version\":{},\"profile\":{},\"payload_bytes\":{},\"event_count\":{},\"resident_rows\":{},\"writeback_reservations\":{},\"wake_partition\":{},\"wake_tick\":{},\"wake_kind\":{},\"rebound_wakes\":{}}}",
            self.decode_error,
            optional_u64_json(self.version),
            optional_static_str_json(self.profile),
            self.payload_bytes,
            optional_u64_json(self.event_count),
            optional_u64_json(self.resident_rows),
            optional_u64_json(self.writeback_reservations),
            optional_u64_json(self.wake_partition),
            optional_u64_json(self.wake_tick),
            optional_static_str_json(self.wake_kind),
            self.rebound_wakes,
        )
    }

    pub(crate) fn numeric_stat_fields(
        &self,
    ) -> Vec<(&'static str, Rem6HostO3RuntimeCheckpointStatValue)> {
        [
            ("version", self.version),
            ("payload_bytes", Some(self.payload_bytes)),
            ("event_count", self.event_count),
            ("resident_rows", self.resident_rows),
            ("writeback_reservations", self.writeback_reservations),
            ("wake_partition", self.wake_partition),
            ("wake_tick", self.wake_tick),
            ("rebound_wakes", Some(self.rebound_wakes)),
        ]
        .into_iter()
        .filter_map(|(name, value)| {
            value.map(|value| {
                let (aggregation, unit) = o3_live_checkpoint_stat_metadata(name);
                (
                    name,
                    Rem6HostO3RuntimeCheckpointStatValue::new(aggregation, unit, value),
                )
            })
        })
        .collect()
    }
}

pub(super) fn decode_o3_live_checkpoint_chunk(
    name: &str,
    payload: &[u8],
    wake_rebound: bool,
) -> Option<Rem6HostO3LiveCheckpointChunkSummary> {
    if name != RISCV_O3_LIVE_CHECKPOINT_CHUNK {
        return None;
    }
    let payload_bytes = payload.len() as u64;
    Some(
        match RiscvO3LiveCheckpointPayload::decode_versioned(payload) {
            Ok((wire_version, decoded)) => Rem6HostO3LiveCheckpointChunkSummary::decoded(
                payload_bytes,
                wire_version,
                &decoded,
                wake_rebound,
            ),
            Err(_) => Rem6HostO3LiveCheckpointChunkSummary::decode_error(payload_bytes),
        },
    )
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_static_str_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{value}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn o3_live_checkpoint_stat_metadata(
    name: &str,
) -> (Rem6HostO3RuntimeCheckpointStatAggregation, &'static str) {
    match name {
        "payload_bytes" => (Rem6HostO3RuntimeCheckpointStatAggregation::Sum, "Byte"),
        "event_count" | "resident_rows" | "writeback_reservations" | "rebound_wakes" => {
            (Rem6HostO3RuntimeCheckpointStatAggregation::Sum, "Count")
        }
        "wake_tick" => (Rem6HostO3RuntimeCheckpointStatAggregation::Max, "Tick"),
        "version" | "wake_partition" => (Rem6HostO3RuntimeCheckpointStatAggregation::Max, "Count"),
        _ => (Rem6HostO3RuntimeCheckpointStatAggregation::Max, "Count"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rem6_checkpoint::{
        CheckpointChunk, CheckpointComponentId, CheckpointManifest, CheckpointState,
    };
    use serde_json::Value;

    use super::RISCV_O3_LIVE_CHECKPOINT_CHUNK;
    use crate::host_actions::checkpoint_summary_from_manifest;

    #[test]
    fn o3_live_checkpoint_summary_is_exact_name_bounded_and_non_panicking() {
        let manifest = CheckpointManifest::new(
            "bad-o3-live",
            17,
            vec![CheckpointState::new(
                CheckpointComponentId::new("cpu0").unwrap(),
                vec![
                    CheckpointChunk::new(RISCV_O3_LIVE_CHECKPOINT_CHUNK, b"not-o3-live".to_vec()),
                    CheckpointChunk::new("o3-live-checkpoint-shadow", b"not-o3-live".to_vec()),
                ],
            )],
        );

        let capture =
            checkpoint_summary_from_manifest(23, 29, 0, &manifest, false, &BTreeSet::new());
        let restore =
            checkpoint_summary_from_manifest(31, 37, 0, &manifest, true, &BTreeSet::new());
        for summary in [&capture, &restore] {
            let live = summary.components[0].chunks[0]
                .o3_live_checkpoint
                .as_ref()
                .expect("exact O3LC chunk should expose a bounded decode summary");
            assert!(live.decode_error);
            assert_eq!(live.payload_bytes, 11);
            assert_eq!(live.version, None);
            assert_eq!(live.profile, None);
            assert_eq!(live.event_count, None);
            assert_eq!(live.resident_rows, None);
            assert_eq!(live.writeback_reservations, None);
            assert_eq!(live.wake_partition, None);
            assert_eq!(live.wake_tick, None);
            assert_eq!(live.wake_kind, None);
            assert_eq!(live.rebound_wakes, 0);
            assert!(summary.components[0].chunks[1].o3_live_checkpoint.is_none());

            let json: Value = serde_json::from_str(&summary.to_json()).unwrap();
            assert_eq!(
                json.pointer("/components/0/chunks/0/o3_live_checkpoint/decode_error")
                    .and_then(Value::as_bool),
                Some(true)
            );
            assert_eq!(
                json.pointer("/components/0/chunks/0/o3_live_checkpoint/payload_bytes")
                    .and_then(Value::as_u64),
                Some(11)
            );
            assert_eq!(
                json.pointer("/components/0/chunks/0/o3_live_checkpoint/rebound_wakes")
                    .and_then(Value::as_u64),
                Some(0)
            );
            for field in [
                "version",
                "profile",
                "event_count",
                "resident_rows",
                "writeback_reservations",
                "wake_partition",
                "wake_tick",
                "wake_kind",
            ] {
                assert_eq!(
                    json.pointer(&format!(
                        "/components/0/chunks/0/o3_live_checkpoint/{field}"
                    )),
                    Some(&Value::Null)
                );
            }
            assert!(json
                .pointer("/components/0/chunks/1/o3_live_checkpoint")
                .is_none());
        }
    }
}
