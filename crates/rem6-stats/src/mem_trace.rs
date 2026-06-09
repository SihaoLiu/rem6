use std::collections::BTreeSet;

use rem6_kernel::Tick;

use crate::probes::{MemProbePacket, ProbeEvent, ProbePayload, ProbePointId};
use crate::StatsError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemTraceProbeHeader {
    object_id: String,
    tick_frequency: u64,
    requestors: Vec<(u32, String)>,
}

impl MemTraceProbeHeader {
    pub fn new(
        object_id: impl Into<String>,
        tick_frequency: u64,
        mut requestors: Vec<(u32, String)>,
    ) -> Result<Self, StatsError> {
        let object_id = object_id.into();
        if object_id.is_empty() {
            return Err(StatsError::EmptyMemTraceObjectId);
        }
        if tick_frequency == 0 {
            return Err(StatsError::InvalidMemTraceTickFrequency {
                frequency: tick_frequency,
            });
        }

        requestors.sort_by_key(|(requestor, _)| *requestor);
        let mut seen = BTreeSet::new();
        for (requestor, name) in &requestors {
            if name.is_empty() {
                return Err(StatsError::EmptyMemTraceRequestorName {
                    requestor: *requestor,
                });
            }
            if !seen.insert(*requestor) {
                return Err(StatsError::DuplicateMemTraceRequestor {
                    requestor: *requestor,
                });
            }
        }

        Ok(Self {
            object_id,
            tick_frequency,
            requestors,
        })
    }

    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub fn requestors(&self) -> &[(u32, String)] {
        &self.requestors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemTraceProbeConfig {
    header: MemTraceProbeHeader,
    with_pc: bool,
}

impl MemTraceProbeConfig {
    pub const fn new(header: MemTraceProbeHeader, with_pc: bool) -> Self {
        Self { header, with_pc }
    }

    pub const fn header(&self) -> &MemTraceProbeHeader {
        &self.header
    }

    pub const fn with_pc(&self) -> bool {
        self.with_pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemTracePacketRecord {
    tick: Tick,
    command: u32,
    flags: u64,
    address: u64,
    size: u64,
    program_counter: Option<u64>,
    packet_id: u64,
}

impl MemTracePacketRecord {
    pub const fn new(
        tick: Tick,
        command: u32,
        flags: u64,
        address: u64,
        size: u64,
        program_counter: Option<u64>,
        packet_id: u64,
    ) -> Self {
        Self {
            tick,
            command,
            flags,
            address,
            size,
            program_counter,
            packet_id,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn command(&self) -> u32 {
        self.command
    }

    pub const fn flags(&self) -> u64 {
        self.flags
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn program_counter(&self) -> Option<u64> {
        self.program_counter
    }

    pub const fn packet_id(&self) -> u64 {
        self.packet_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemTraceProbeSnapshot {
    header: MemTraceProbeHeader,
    with_pc: bool,
    records: Vec<MemTracePacketRecord>,
}

impl MemTraceProbeSnapshot {
    pub const fn new(
        header: MemTraceProbeHeader,
        with_pc: bool,
        records: Vec<MemTracePacketRecord>,
    ) -> Self {
        Self {
            header,
            with_pc,
            records,
        }
    }

    pub const fn header(&self) -> &MemTraceProbeHeader {
        &self.header
    }

    pub const fn with_pc(&self) -> bool {
        self.with_pc
    }

    pub fn records(&self) -> &[MemTracePacketRecord] {
        &self.records
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemTraceProbe {
    config: MemTraceProbeConfig,
    records: Vec<MemTracePacketRecord>,
}

impl MemTraceProbe {
    pub const fn new(config: MemTraceProbeConfig) -> Self {
        Self {
            config,
            records: Vec::new(),
        }
    }

    pub fn from_snapshot(snapshot: &MemTraceProbeSnapshot) -> Result<Self, StatsError> {
        validate_records(snapshot.records(), snapshot.with_pc())?;
        Ok(Self {
            config: MemTraceProbeConfig::new(snapshot.header().clone(), snapshot.with_pc()),
            records: snapshot.records().to_vec(),
        })
    }

    pub const fn header(&self) -> &MemTraceProbeHeader {
        self.config.header()
    }

    pub const fn with_pc(&self) -> bool {
        self.config.with_pc()
    }

    pub fn records(&self) -> &[MemTracePacketRecord] {
        &self.records
    }

    pub fn observe_packet(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
    ) -> Result<&MemTracePacketRecord, StatsError> {
        if let Some(previous) = self.records.last() {
            validate_record_time(tick, previous.tick())?;
        }

        let program_counter = if self.config.with_pc() && packet.program_counter() != 0 {
            Some(packet.program_counter())
        } else {
            None
        };
        self.records.push(MemTracePacketRecord::new(
            tick,
            packet.command(),
            packet.flags(),
            packet.address(),
            packet.size(),
            program_counter,
            packet.packet_id(),
        ));
        Ok(self.records.last().expect("trace record was just appended"))
    }

    pub fn observe_probe_event(
        &mut self,
        event: &ProbeEvent,
        packet_point: ProbePointId,
    ) -> Result<Option<&MemTracePacketRecord>, StatsError> {
        if event.point() != packet_point {
            return Ok(None);
        }
        let ProbePayload::MemoryPacket(packet) = event.payload() else {
            return Ok(None);
        };
        self.observe_packet(event.tick(), packet).map(Some)
    }

    pub fn snapshot(&self) -> MemTraceProbeSnapshot {
        MemTraceProbeSnapshot::new(
            self.config.header().clone(),
            self.config.with_pc(),
            self.records.clone(),
        )
    }
}

fn validate_records(records: &[MemTracePacketRecord], with_pc: bool) -> Result<(), StatsError> {
    let mut previous = None;
    for record in records {
        validate_record_program_counter(record, with_pc)?;
        if let Some(previous_tick) = previous {
            validate_record_time(record.tick(), previous_tick)?;
        }
        previous = Some(record.tick());
    }
    Ok(())
}

fn validate_record_program_counter(
    record: &MemTracePacketRecord,
    with_pc: bool,
) -> Result<(), StatsError> {
    match record.program_counter() {
        Some(0) => Err(StatsError::MemTraceSnapshotZeroProgramCounter {
            tick: record.tick(),
        }),
        Some(program_counter) if !with_pc => {
            Err(StatsError::MemTraceSnapshotUnexpectedProgramCounter {
                tick: record.tick(),
                program_counter,
            })
        }
        _ => Ok(()),
    }
}

fn validate_record_time(current_tick: Tick, previous_tick: Tick) -> Result<(), StatsError> {
    if current_tick < previous_tick {
        return Err(StatsError::MemTraceRecordTimeWentBack {
            previous_tick,
            current_tick,
        });
    }
    Ok(())
}
