use crate::{Rem6DramPortSummary, Rem6DramSummary};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Rem6DramTraceRecord {
    Target {
        target: u32,
        accesses: u64,
        reads: u64,
        writes: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        turnarounds: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
        low_power: Rem6DramTraceLowPower,
    },
    Port {
        target: u32,
        port: u32,
        accesses: u64,
        reads: u64,
        writes: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        turnarounds: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
        low_power: Rem6DramTraceLowPower,
    },
    Bank {
        target: u32,
        port: u32,
        bank: u32,
        accesses: u64,
        read_bytes: u64,
        write_bytes: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
        low_power: Rem6DramTraceLowPower,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6DramTraceLowPower {
    active_powerdown_entries: u64,
    active_powerdown_ticks: u64,
    precharge_powerdown_entries: u64,
    precharge_powerdown_ticks: u64,
    self_refresh_entries: u64,
    self_refresh_ticks: u64,
    exits: u64,
    exit_latency_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6DramTraceStat {
    path: String,
    unit: &'static str,
    value: u64,
}

impl Rem6DramTraceStat {
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

impl Rem6DramTraceRecord {
    pub(crate) fn to_json(&self) -> String {
        match self {
            Self::Target {
                target,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => format!(
                "{{\"kind\":\"target\",\"target\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{}}}",
                target,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power.to_json(),
            ),
            Self::Port {
                target,
                port,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => format!(
                "{{\"kind\":\"port\",\"target\":{},\"port\":{},\"accesses\":{},\"reads\":{},\"writes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"turnarounds\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{}}}",
                target,
                port,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power.to_json(),
            ),
            Self::Bank {
                target,
                port,
                bank,
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => format!(
                "{{\"kind\":\"bank\",\"target\":{},\"port\":{},\"bank\":{},\"accesses\":{},\"read_bytes\":{},\"write_bytes\":{},\"row_hits\":{},\"row_misses\":{},\"refreshes\":{},\"refresh_ticks\":{},\"commands\":{},\"total_ready_latency_ticks\":{},\"max_ready_latency_ticks\":{},\"low_power\":{}}}",
                target,
                port,
                bank,
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power.to_json(),
            ),
        }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Target { .. } => "target",
            Self::Port { .. } => "port",
            Self::Bank { .. } => "bank",
        }
    }

    const fn low_power(&self) -> Rem6DramTraceLowPower {
        match self {
            Self::Target { low_power, .. }
            | Self::Port { low_power, .. }
            | Self::Bank { low_power, .. } => *low_power,
        }
    }

    pub(crate) fn stats(&self) -> Vec<Rem6DramTraceStat> {
        let mut stats = Vec::new();
        match self {
            Self::Target {
                target,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => {
                let prefix = format!("target{target}");
                for (suffix, unit, value) in [
                    ("accesses", "Count", *accesses),
                    ("reads", "Count", *reads),
                    ("writes", "Count", *writes),
                    ("row_hits", "Count", *row_hits),
                    ("row_misses", "Count", *row_misses),
                    ("refreshes", "Count", *refreshes),
                    ("refresh_ticks", "Tick", *refresh_ticks),
                    ("commands", "Count", *commands),
                    ("turnarounds", "Count", *turnarounds),
                    (
                        "total_ready_latency_ticks",
                        "Tick",
                        *total_ready_latency_ticks,
                    ),
                    ("max_ready_latency_ticks", "Tick", *max_ready_latency_ticks),
                ] {
                    push_dram_trace_stat(&mut stats, &prefix, suffix, unit, value);
                }
                low_power.push_stats(&mut stats, &format!("{prefix}.low_power"));
            }
            Self::Port {
                target,
                port,
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => {
                let prefix = format!("target{target}.port{port}");
                for (suffix, unit, value) in [
                    ("accesses", "Count", *accesses),
                    ("reads", "Count", *reads),
                    ("writes", "Count", *writes),
                    ("row_hits", "Count", *row_hits),
                    ("row_misses", "Count", *row_misses),
                    ("refreshes", "Count", *refreshes),
                    ("refresh_ticks", "Tick", *refresh_ticks),
                    ("commands", "Count", *commands),
                    ("turnarounds", "Count", *turnarounds),
                    (
                        "total_ready_latency_ticks",
                        "Tick",
                        *total_ready_latency_ticks,
                    ),
                    ("max_ready_latency_ticks", "Tick", *max_ready_latency_ticks),
                ] {
                    push_dram_trace_stat(&mut stats, &prefix, suffix, unit, value);
                }
                low_power.push_stats(&mut stats, &format!("{prefix}.low_power"));
            }
            Self::Bank {
                target,
                port,
                bank,
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                low_power,
            } => {
                let prefix = format!("target{target}.port{port}.bank{bank}");
                for (suffix, unit, value) in [
                    ("accesses", "Count", *accesses),
                    ("read_bytes", "Byte", *read_bytes),
                    ("write_bytes", "Byte", *write_bytes),
                    ("row_hits", "Count", *row_hits),
                    ("row_misses", "Count", *row_misses),
                    ("refreshes", "Count", *refreshes),
                    ("refresh_ticks", "Tick", *refresh_ticks),
                    ("commands", "Count", *commands),
                    (
                        "total_ready_latency_ticks",
                        "Tick",
                        *total_ready_latency_ticks,
                    ),
                    ("max_ready_latency_ticks", "Tick", *max_ready_latency_ticks),
                ] {
                    push_dram_trace_stat(&mut stats, &prefix, suffix, unit, value);
                }
                low_power.push_stats(&mut stats, &format!("{prefix}.low_power"));
            }
        }
        stats
    }

    const fn sort_key(&self) -> (u32, u8, u32, u32) {
        match self {
            Self::Target { target, .. } => (*target, 0, u32::MAX, u32::MAX),
            Self::Port { target, port, .. } => (*target, 1, *port, u32::MAX),
            Self::Bank {
                target, port, bank, ..
            } => (*target, 2, *port, *bank),
        }
    }
}

pub(crate) fn dram_trace_records(dram: &Rem6DramSummary) -> Vec<Rem6DramTraceRecord> {
    let mut records = Vec::new();
    for target in &dram.targets {
        records.push(Rem6DramTraceRecord::Target {
            target: target.target,
            accesses: target.accesses,
            reads: target.reads,
            writes: target.writes,
            row_hits: target.row_hits,
            row_misses: target.row_misses,
            refreshes: target.refreshes,
            refresh_ticks: target.refresh_ticks,
            commands: target.commands,
            turnarounds: target.turnarounds,
            total_ready_latency_ticks: target.total_ready_latency_ticks,
            max_ready_latency_ticks: target.max_ready_latency_ticks,
            low_power: Rem6DramTraceLowPower {
                active_powerdown_entries: target.low_power_active_powerdown_entries,
                active_powerdown_ticks: target.low_power_active_powerdown_ticks,
                precharge_powerdown_entries: target.low_power_precharge_powerdown_entries,
                precharge_powerdown_ticks: target.low_power_precharge_powerdown_ticks,
                self_refresh_entries: target.low_power_self_refresh_entries,
                self_refresh_ticks: target.low_power_self_refresh_ticks,
                exits: target.low_power_exits,
                exit_latency_ticks: target.low_power_exit_latency_ticks,
            },
        });
        for port in &target.ports {
            records.push(Rem6DramTraceRecord::Port {
                target: target.target,
                port: port.port,
                accesses: port.accesses,
                reads: port.reads,
                writes: port.writes,
                row_hits: dram_port_row_hits(port),
                row_misses: dram_port_row_misses(port),
                refreshes: dram_port_refreshes(port),
                refresh_ticks: dram_port_refresh_ticks(port),
                commands: port.commands,
                turnarounds: port.turnarounds,
                total_ready_latency_ticks: dram_port_total_ready_latency_ticks(port),
                max_ready_latency_ticks: dram_port_max_ready_latency_ticks(port),
                low_power: Rem6DramTraceLowPower {
                    active_powerdown_entries: port.low_power_active_powerdown_entries,
                    active_powerdown_ticks: port.low_power_active_powerdown_ticks,
                    precharge_powerdown_entries: port.low_power_precharge_powerdown_entries,
                    precharge_powerdown_ticks: port.low_power_precharge_powerdown_ticks,
                    self_refresh_entries: port.low_power_self_refresh_entries,
                    self_refresh_ticks: port.low_power_self_refresh_ticks,
                    exits: port.low_power_exits,
                    exit_latency_ticks: port.low_power_exit_latency_ticks,
                },
            });
            for bank in &port.banks {
                records.push(Rem6DramTraceRecord::Bank {
                    target: target.target,
                    port: port.port,
                    bank: bank.bank,
                    accesses: bank.accesses,
                    read_bytes: bank.read_bytes,
                    write_bytes: bank.write_bytes,
                    row_hits: bank.row_hits,
                    row_misses: bank.row_misses,
                    refreshes: bank.refreshes,
                    refresh_ticks: bank.refresh_ticks,
                    commands: bank.commands,
                    total_ready_latency_ticks: bank.total_ready_latency_ticks,
                    max_ready_latency_ticks: bank.max_ready_latency_ticks,
                    low_power: Rem6DramTraceLowPower {
                        active_powerdown_entries: bank.low_power_active_powerdown_entries,
                        active_powerdown_ticks: bank.low_power_active_powerdown_ticks,
                        precharge_powerdown_entries: bank.low_power_precharge_powerdown_entries,
                        precharge_powerdown_ticks: bank.low_power_precharge_powerdown_ticks,
                        self_refresh_entries: bank.low_power_self_refresh_entries,
                        self_refresh_ticks: bank.low_power_self_refresh_ticks,
                        exits: bank.low_power_exits,
                        exit_latency_ticks: bank.low_power_exit_latency_ticks,
                    },
                });
            }
        }
    }
    records.sort_by_key(Rem6DramTraceRecord::sort_key);
    records
}

pub(crate) fn dram_trace_kind_stats(records: &[Rem6DramTraceRecord]) -> Vec<Rem6DramTraceStat> {
    let mut target = Rem6DramTraceKindAggregate::default();
    let mut port = Rem6DramTraceKindAggregate::default();
    let mut bank = Rem6DramTraceKindAggregate::default();

    for record in records {
        match record {
            Rem6DramTraceRecord::Target {
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                ..
            } => {
                target.add_common(
                    *accesses,
                    *row_hits,
                    *row_misses,
                    *refreshes,
                    *refresh_ticks,
                    *commands,
                    *total_ready_latency_ticks,
                    *max_ready_latency_ticks,
                );
                target.reads = target.reads.saturating_add(*reads);
                target.writes = target.writes.saturating_add(*writes);
                target.turnarounds = target.turnarounds.saturating_add(*turnarounds);
            }
            Rem6DramTraceRecord::Port {
                accesses,
                reads,
                writes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                turnarounds,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                ..
            } => {
                port.add_common(
                    *accesses,
                    *row_hits,
                    *row_misses,
                    *refreshes,
                    *refresh_ticks,
                    *commands,
                    *total_ready_latency_ticks,
                    *max_ready_latency_ticks,
                );
                port.reads = port.reads.saturating_add(*reads);
                port.writes = port.writes.saturating_add(*writes);
                port.turnarounds = port.turnarounds.saturating_add(*turnarounds);
            }
            Rem6DramTraceRecord::Bank {
                accesses,
                read_bytes,
                write_bytes,
                row_hits,
                row_misses,
                refreshes,
                refresh_ticks,
                commands,
                total_ready_latency_ticks,
                max_ready_latency_ticks,
                ..
            } => {
                bank.add_common(
                    *accesses,
                    *row_hits,
                    *row_misses,
                    *refreshes,
                    *refresh_ticks,
                    *commands,
                    *total_ready_latency_ticks,
                    *max_ready_latency_ticks,
                );
                bank.read_bytes = bank.read_bytes.saturating_add(*read_bytes);
                bank.write_bytes = bank.write_bytes.saturating_add(*write_bytes);
            }
        }
    }

    let mut stats = Vec::new();
    target.push_target_or_port_stats(&mut stats, "target");
    port.push_target_or_port_stats(&mut stats, "port");
    bank.push_bank_stats(&mut stats);
    stats
}

pub(crate) fn dram_trace_payload_byte_count(records: &[Rem6DramTraceRecord]) -> u64 {
    records.iter().fold(0u64, |acc, record| match record {
        Rem6DramTraceRecord::Bank {
            read_bytes,
            write_bytes,
            ..
        } => acc.saturating_add(*read_bytes).saturating_add(*write_bytes),
        Rem6DramTraceRecord::Target { .. } | Rem6DramTraceRecord::Port { .. } => acc,
    })
}

pub(crate) fn dram_trace_low_power_kind_stats(
    records: &[Rem6DramTraceRecord],
) -> Vec<Rem6DramTraceStat> {
    let mut stats = Vec::new();
    for kind in ["target", "port", "bank"] {
        let low_power = records
            .iter()
            .filter(|record| record.kind() == kind)
            .fold(Rem6DramTraceLowPower::default(), |acc, record| {
                acc.saturating_add(record.low_power())
            });
        low_power.push_stats(&mut stats, &format!("{kind}.low_power"));
    }
    stats
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6DramTraceKindAggregate {
    accesses: u64,
    reads: u64,
    writes: u64,
    read_bytes: u64,
    write_bytes: u64,
    row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
}

impl Rem6DramTraceKindAggregate {
    fn add_common(
        &mut self,
        accesses: u64,
        row_hits: u64,
        row_misses: u64,
        refreshes: u64,
        refresh_ticks: u64,
        commands: u64,
        total_ready_latency_ticks: u64,
        max_ready_latency_ticks: u64,
    ) {
        self.accesses = self.accesses.saturating_add(accesses);
        self.row_hits = self.row_hits.saturating_add(row_hits);
        self.row_misses = self.row_misses.saturating_add(row_misses);
        self.refreshes = self.refreshes.saturating_add(refreshes);
        self.refresh_ticks = self.refresh_ticks.saturating_add(refresh_ticks);
        self.commands = self.commands.saturating_add(commands);
        self.total_ready_latency_ticks = self
            .total_ready_latency_ticks
            .saturating_add(total_ready_latency_ticks);
        self.max_ready_latency_ticks = self.max_ready_latency_ticks.max(max_ready_latency_ticks);
    }

    fn push_target_or_port_stats(self, stats: &mut Vec<Rem6DramTraceStat>, prefix: &'static str) {
        for (suffix, unit, value) in [
            ("accesses", "Count", self.accesses),
            ("reads", "Count", self.reads),
            ("writes", "Count", self.writes),
            ("row_hits", "Count", self.row_hits),
            ("row_misses", "Count", self.row_misses),
            ("refreshes", "Count", self.refreshes),
            ("refresh_ticks", "Tick", self.refresh_ticks),
            ("commands", "Count", self.commands),
            ("turnarounds", "Count", self.turnarounds),
            (
                "total_ready_latency_ticks",
                "Tick",
                self.total_ready_latency_ticks,
            ),
            (
                "max_ready_latency_ticks",
                "Tick",
                self.max_ready_latency_ticks,
            ),
        ] {
            push_dram_trace_stat(stats, prefix, suffix, unit, value);
        }
    }

    fn push_bank_stats(self, stats: &mut Vec<Rem6DramTraceStat>) {
        for (suffix, unit, value) in [
            ("accesses", "Count", self.accesses),
            ("read_bytes", "Byte", self.read_bytes),
            ("write_bytes", "Byte", self.write_bytes),
            ("row_hits", "Count", self.row_hits),
            ("row_misses", "Count", self.row_misses),
            ("refreshes", "Count", self.refreshes),
            ("refresh_ticks", "Tick", self.refresh_ticks),
            ("commands", "Count", self.commands),
            (
                "total_ready_latency_ticks",
                "Tick",
                self.total_ready_latency_ticks,
            ),
            (
                "max_ready_latency_ticks",
                "Tick",
                self.max_ready_latency_ticks,
            ),
        ] {
            push_dram_trace_stat(stats, "bank", suffix, unit, value);
        }
    }
}

impl Rem6DramTraceLowPower {
    const fn saturating_add(self, other: Self) -> Self {
        Self {
            active_powerdown_entries: self
                .active_powerdown_entries
                .saturating_add(other.active_powerdown_entries),
            active_powerdown_ticks: self
                .active_powerdown_ticks
                .saturating_add(other.active_powerdown_ticks),
            precharge_powerdown_entries: self
                .precharge_powerdown_entries
                .saturating_add(other.precharge_powerdown_entries),
            precharge_powerdown_ticks: self
                .precharge_powerdown_ticks
                .saturating_add(other.precharge_powerdown_ticks),
            self_refresh_entries: self
                .self_refresh_entries
                .saturating_add(other.self_refresh_entries),
            self_refresh_ticks: self
                .self_refresh_ticks
                .saturating_add(other.self_refresh_ticks),
            exits: self.exits.saturating_add(other.exits),
            exit_latency_ticks: self
                .exit_latency_ticks
                .saturating_add(other.exit_latency_ticks),
        }
    }

    fn to_json(self) -> String {
        format!(
            "{{\"active_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"precharge_powerdown\":{{\"entries\":{},\"ticks\":{}}},\"self_refresh\":{{\"entries\":{},\"ticks\":{}}},\"exits\":{},\"exit_latency_ticks\":{}}}",
            self.active_powerdown_entries,
            self.active_powerdown_ticks,
            self.precharge_powerdown_entries,
            self.precharge_powerdown_ticks,
            self.self_refresh_entries,
            self.self_refresh_ticks,
            self.exits,
            self.exit_latency_ticks,
        )
    }

    fn push_stats(self, stats: &mut Vec<Rem6DramTraceStat>, prefix: &str) {
        for (suffix, unit, value) in [
            (
                "active_powerdown.entries",
                "Count",
                self.active_powerdown_entries,
            ),
            (
                "active_powerdown.ticks",
                "Tick",
                self.active_powerdown_ticks,
            ),
            (
                "precharge_powerdown.entries",
                "Count",
                self.precharge_powerdown_entries,
            ),
            (
                "precharge_powerdown.ticks",
                "Tick",
                self.precharge_powerdown_ticks,
            ),
            ("self_refresh.entries", "Count", self.self_refresh_entries),
            ("self_refresh.ticks", "Tick", self.self_refresh_ticks),
            ("exits", "Count", self.exits),
            ("exit_latency_ticks", "Tick", self.exit_latency_ticks),
        ] {
            push_dram_trace_stat(stats, prefix, suffix, unit, value);
        }
    }
}

fn push_dram_trace_stat(
    stats: &mut Vec<Rem6DramTraceStat>,
    prefix: &str,
    suffix: &'static str,
    unit: &'static str,
    value: u64,
) {
    stats.push(Rem6DramTraceStat {
        path: format!("{prefix}.{suffix}"),
        unit,
        value,
    });
}

fn dram_port_row_hits(port: &Rem6DramPortSummary) -> u64 {
    port.banks.iter().map(|bank| bank.row_hits).sum()
}

fn dram_port_row_misses(port: &Rem6DramPortSummary) -> u64 {
    port.banks.iter().map(|bank| bank.row_misses).sum()
}

fn dram_port_refreshes(port: &Rem6DramPortSummary) -> u64 {
    port.banks.iter().map(|bank| bank.refreshes).sum()
}

fn dram_port_refresh_ticks(port: &Rem6DramPortSummary) -> u64 {
    port.banks.iter().map(|bank| bank.refresh_ticks).sum()
}

fn dram_port_total_ready_latency_ticks(port: &Rem6DramPortSummary) -> u64 {
    port.banks
        .iter()
        .map(|bank| bank.total_ready_latency_ticks)
        .sum()
}

fn dram_port_max_ready_latency_ticks(port: &Rem6DramPortSummary) -> u64 {
    port.banks
        .iter()
        .map(|bank| bank.max_ready_latency_ticks)
        .max()
        .unwrap_or(0)
}
