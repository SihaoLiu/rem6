use std::collections::BTreeMap;

use crate::{formatting::json_escape, Rem6RunFabricSummary};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Rem6FabricTraceRecord {
    Lane {
        link: String,
        virtual_network: u64,
        transfer_count: u64,
        byte_count: u64,
        flit_count: u64,
        occupied_ticks: u64,
        queue_delay_ticks: u64,
        max_queue_delay_ticks: u64,
        credit_delay_ticks: u64,
        max_credit_delay_ticks: u64,
        first_tick: u64,
        last_tick: u64,
    },
    Hop {
        packet: u64,
        hop_index: u64,
        link: String,
        virtual_network: u64,
        bytes: u64,
        flits: u64,
        ready_tick: u64,
        start_tick: u64,
        occupied_ticks: u64,
        queue_delay_ticks: u64,
        credit_delay_ticks: u64,
        depart_tick: u64,
        arrival_tick: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6FabricTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricLaneStatSummary {
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FabricHopStatSummary {
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
}

impl Rem6FabricTraceStat {
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(&self) -> u64 {
        self.value
    }
}

impl Rem6FabricTraceRecord {
    pub(crate) fn to_json(&self) -> String {
        match self {
            Self::Lane {
                link,
                virtual_network,
                transfer_count,
                byte_count,
                flit_count,
                occupied_ticks,
                queue_delay_ticks,
                max_queue_delay_ticks,
                credit_delay_ticks,
                max_credit_delay_ticks,
                first_tick,
                last_tick,
            } => format!(
                "{{\"kind\":\"lane\",\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(link),
                virtual_network,
                transfer_count,
                byte_count,
                flit_count,
                occupied_ticks,
                queue_delay_ticks,
                max_queue_delay_ticks,
                credit_delay_ticks,
                max_credit_delay_ticks,
                first_tick,
                last_tick,
            ),
            Self::Hop {
                packet,
                hop_index,
                link,
                virtual_network,
                bytes,
                flits,
                ready_tick,
                start_tick,
                occupied_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                depart_tick,
                arrival_tick,
            } => format!(
                "{{\"kind\":\"hop\",\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                packet,
                hop_index,
                json_escape(link),
                virtual_network,
                bytes,
                flits,
                ready_tick,
                start_tick,
                occupied_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                depart_tick,
                arrival_tick,
            ),
        }
    }

    fn sort_key(&self) -> (u64, u8, String, u64, u64, u64) {
        match self {
            Self::Lane {
                first_tick,
                link,
                virtual_network,
                ..
            } => (*first_tick, 0, link.clone(), *virtual_network, 0, 0),
            Self::Hop {
                start_tick,
                link,
                virtual_network,
                packet,
                hop_index,
                ..
            } => (
                *start_tick,
                1,
                link.clone(),
                *virtual_network,
                *packet,
                *hop_index,
            ),
        }
    }
}

pub(crate) fn fabric_trace_records(fabric: &Rem6RunFabricSummary) -> Vec<Rem6FabricTraceRecord> {
    let mut records = Vec::new();
    records.extend(
        fabric
            .lane_activities()
            .iter()
            .map(|activity| Rem6FabricTraceRecord::Lane {
                link: activity.link().as_str().to_string(),
                virtual_network: u64::from(activity.virtual_network().get()),
                transfer_count: activity.transfer_count() as u64,
                byte_count: activity.byte_count(),
                flit_count: activity.flit_count(),
                occupied_ticks: activity.occupied_ticks(),
                queue_delay_ticks: activity.queue_delay_ticks(),
                max_queue_delay_ticks: activity.max_queue_delay_ticks(),
                credit_delay_ticks: activity.credit_delay_ticks(),
                max_credit_delay_ticks: activity.max_credit_delay_ticks(),
                first_tick: activity.first_tick(),
                last_tick: activity.last_tick(),
            }),
    );
    records.extend(
        fabric
            .hop_activities()
            .iter()
            .map(|activity| Rem6FabricTraceRecord::Hop {
                packet: activity.packet().get(),
                hop_index: activity.hop_index() as u64,
                link: activity.link().as_str().to_string(),
                virtual_network: u64::from(activity.virtual_network().get()),
                bytes: activity.bytes(),
                flits: activity.flits(),
                ready_tick: activity.ready_tick(),
                start_tick: activity.start_tick(),
                occupied_ticks: activity.occupied_ticks(),
                queue_delay_ticks: activity.queue_delay_ticks(),
                credit_delay_ticks: activity.credit_delay_ticks(),
                depart_tick: activity.depart_tick(),
                arrival_tick: activity.arrival_tick(),
            }),
    );
    records.sort_by_key(Rem6FabricTraceRecord::sort_key);
    records
}

pub(crate) fn fabric_trace_stats(
    records: &[Rem6FabricTraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6FabricTraceStat> {
    let mut lanes = BTreeMap::<(String, u64), FabricLaneStatSummary>::new();
    let mut hops = BTreeMap::<(String, u64, u64), FabricHopStatSummary>::new();
    for record in records {
        match record {
            Rem6FabricTraceRecord::Lane {
                link,
                virtual_network,
                transfer_count,
                byte_count,
                flit_count,
                occupied_ticks,
                queue_delay_ticks,
                max_queue_delay_ticks,
                credit_delay_ticks,
                max_credit_delay_ticks,
                ..
            } => {
                let summary = lanes.entry((link.clone(), *virtual_network)).or_default();
                summary.transfers = summary.transfers.saturating_add(*transfer_count);
                summary.bytes = summary.bytes.saturating_add(*byte_count);
                summary.flits = summary.flits.saturating_add(*flit_count);
                summary.occupied_ticks = summary.occupied_ticks.saturating_add(*occupied_ticks);
                summary.queue_delay_ticks =
                    summary.queue_delay_ticks.saturating_add(*queue_delay_ticks);
                summary.max_queue_delay_ticks =
                    summary.max_queue_delay_ticks.max(*max_queue_delay_ticks);
                summary.credit_delay_ticks = summary
                    .credit_delay_ticks
                    .saturating_add(*credit_delay_ticks);
                summary.max_credit_delay_ticks =
                    summary.max_credit_delay_ticks.max(*max_credit_delay_ticks);
            }
            Rem6FabricTraceRecord::Hop {
                link,
                virtual_network,
                hop_index,
                bytes,
                flits,
                occupied_ticks,
                queue_delay_ticks,
                credit_delay_ticks,
                ..
            } => {
                let summary = hops
                    .entry((link.clone(), *virtual_network, *hop_index))
                    .or_default();
                summary.transfers = summary.transfers.saturating_add(1);
                summary.bytes = summary.bytes.saturating_add(*bytes);
                summary.flits = summary.flits.saturating_add(*flits);
                summary.occupied_ticks = summary.occupied_ticks.saturating_add(*occupied_ticks);
                summary.queue_delay_ticks =
                    summary.queue_delay_ticks.saturating_add(*queue_delay_ticks);
                summary.max_queue_delay_ticks =
                    summary.max_queue_delay_ticks.max(*queue_delay_ticks);
                summary.credit_delay_ticks = summary
                    .credit_delay_ticks
                    .saturating_add(*credit_delay_ticks);
            }
        }
    }

    let mut stats = Vec::new();
    for ((link, virtual_network), summary) in lanes {
        let prefix = format!("lane.link.{}.vn{virtual_network}", stat_path_segment(&link));
        push_fabric_trace_stats(
            &mut stats,
            &prefix,
            &[
                ("transfers", "Count", summary.transfers),
                ("bytes", "Byte", summary.bytes),
                ("flits", "Count", summary.flits),
                ("occupied_ticks", "Tick", summary.occupied_ticks),
                ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
                (
                    "max_queue_delay_ticks",
                    "Tick",
                    summary.max_queue_delay_ticks,
                ),
                ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
                (
                    "max_credit_delay_ticks",
                    "Tick",
                    summary.max_credit_delay_ticks,
                ),
            ],
        );
    }
    for ((link, virtual_network, hop_index), summary) in hops {
        let prefix = format!(
            "hop.link.{}.vn{virtual_network}.hop{hop_index}",
            stat_path_segment(&link)
        );
        push_fabric_trace_stats(
            &mut stats,
            &prefix,
            &[
                ("transfers", "Count", summary.transfers),
                ("bytes", "Byte", summary.bytes),
                ("flits", "Count", summary.flits),
                ("occupied_ticks", "Tick", summary.occupied_ticks),
                ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
                (
                    "max_queue_delay_ticks",
                    "Tick",
                    summary.max_queue_delay_ticks,
                ),
                ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
            ],
        );
    }
    stats
}

fn push_fabric_trace_stats(
    stats: &mut Vec<Rem6FabricTraceStat>,
    prefix: &str,
    entries: &[(&'static str, &'static str, u64)],
) {
    for (suffix, unit, value) in entries {
        stats.push(Rem6FabricTraceStat {
            path: format!("{prefix}.{suffix}"),
            unit,
            value: *value,
        });
    }
}
