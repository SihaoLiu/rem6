use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::mem_checker::{
    MemChecker, MemCheckerReadResult, MemCheckerSnapshot, MemCheckerWriteResult,
};
use crate::probes::{MemProbePacket, MemProbePacketAccess, MemProbePacketKind};
use crate::StatsError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemCheckerMonitorPendingTransaction {
    packet_id: u64,
    access: MemProbePacketAccess,
    address: u64,
    size: u64,
    serial: u64,
    request_tick: Tick,
}

impl MemCheckerMonitorPendingTransaction {
    pub const fn new(
        packet_id: u64,
        access: MemProbePacketAccess,
        address: u64,
        size: u64,
        serial: u64,
        request_tick: Tick,
    ) -> Self {
        Self {
            packet_id,
            access,
            address,
            size,
            serial,
            request_tick,
        }
    }

    pub const fn packet_id(self) -> u64 {
        self.packet_id
    }

    pub const fn access(self) -> MemProbePacketAccess {
        self.access
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn size(self) -> u64 {
        self.size
    }

    pub const fn serial(self) -> u64 {
        self.serial
    }

    pub const fn request_tick(self) -> Tick {
        self.request_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemCheckerMonitorCompletion {
    Read(MemCheckerReadResult),
    Write(MemCheckerWriteResult),
    AbortedWrite(MemCheckerWriteResult),
}

impl MemCheckerMonitorCompletion {
    pub const fn serial(&self) -> u64 {
        match self {
            Self::Read(result) => result.serial(),
            Self::Write(result) | Self::AbortedWrite(result) => result.serial(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerMonitorSnapshot {
    checker: MemCheckerSnapshot,
    pending: Vec<MemCheckerMonitorPendingTransaction>,
}

impl MemCheckerMonitorSnapshot {
    pub const fn new(
        checker: MemCheckerSnapshot,
        pending: Vec<MemCheckerMonitorPendingTransaction>,
    ) -> Self {
        Self { checker, pending }
    }

    pub const fn checker(&self) -> &MemCheckerSnapshot {
        &self.checker
    }

    pub fn pending(&self) -> &[MemCheckerMonitorPendingTransaction] {
        &self.pending
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerMonitor {
    checker: MemChecker,
    pending: BTreeMap<u64, MemCheckerMonitorPendingTransaction>,
}

impl MemCheckerMonitor {
    pub fn new() -> Self {
        Self {
            checker: MemChecker::new(),
            pending: BTreeMap::new(),
        }
    }

    pub fn with_checker(checker: MemChecker) -> Self {
        Self {
            checker,
            pending: BTreeMap::new(),
        }
    }

    pub fn from_snapshot(snapshot: &MemCheckerMonitorSnapshot) -> Result<Self, StatsError> {
        let checker = MemChecker::from_snapshot(snapshot.checker())?;
        let pending = validate_pending_transactions(snapshot.pending(), snapshot.checker())?;
        Ok(Self { checker, pending })
    }

    pub const fn checker(&self) -> &MemChecker {
        &self.checker
    }

    pub fn pending(&self) -> impl Iterator<Item = &MemCheckerMonitorPendingTransaction> {
        self.pending.values()
    }

    pub fn observe_timing_request(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
        expects_response: bool,
        forwarded: bool,
        request_data: Option<&[u8]>,
    ) -> Result<Option<MemCheckerMonitorPendingTransaction>, StatsError> {
        if packet.kind() != MemProbePacketKind::Request {
            return Ok(None);
        }
        let access = packet.access();
        if !tracks_checker_access(access) || !forwarded || !expects_response {
            return Ok(None);
        }
        if self.pending.contains_key(&packet.packet_id()) {
            return Err(StatsError::DuplicateMemCheckerMonitorPendingPacket {
                packet_id: packet.packet_id(),
            });
        }

        let size = packet_size(packet);
        let started = match access {
            MemProbePacketAccess::Read => self.checker.start_read(tick, packet.address(), size)?,
            MemProbePacketAccess::Write => {
                let data = validate_request_data(packet, request_data)?;
                self.checker.start_write(tick, packet.address(), data)?
            }
            MemProbePacketAccess::Other => unreachable!("access was checked above"),
        };
        let pending = MemCheckerMonitorPendingTransaction::new(
            packet.packet_id(),
            access,
            packet.address(),
            packet.size(),
            started.serial(),
            tick,
        );
        self.pending.insert(packet.packet_id(), pending);
        Ok(Some(pending))
    }

    pub fn observe_timing_response(
        &mut self,
        tick: Tick,
        packet: &MemProbePacket,
        forwarded: bool,
        response_data: Option<&[u8]>,
        store_conditional_failed: bool,
    ) -> Result<Option<MemCheckerMonitorCompletion>, StatsError> {
        if packet.kind() != MemProbePacketKind::Response {
            return Ok(None);
        }
        let access = packet.access();
        if !tracks_checker_access(access) {
            return Ok(None);
        }

        let pending = *self.pending.get(&packet.packet_id()).ok_or(
            StatsError::UnknownMemCheckerMonitorPendingPacket {
                packet_id: packet.packet_id(),
            },
        )?;
        validate_response_packet(packet, pending)?;
        let response_data = if access == MemProbePacketAccess::Read {
            Some(validate_response_data(packet, response_data)?)
        } else {
            None
        };

        if !forwarded {
            return Ok(None);
        }

        let completion = match access {
            MemProbePacketAccess::Read => {
                let result = self.checker.complete_read(
                    pending.serial(),
                    tick,
                    packet.address(),
                    response_data.expect("read response data was validated"),
                )?;
                MemCheckerMonitorCompletion::Read(result)
            }
            MemProbePacketAccess::Write => {
                let size = packet_size(packet);
                if store_conditional_failed {
                    let result =
                        self.checker
                            .abort_write(pending.serial(), packet.address(), size)?;
                    MemCheckerMonitorCompletion::AbortedWrite(result)
                } else {
                    let result = self.checker.complete_write(
                        pending.serial(),
                        tick,
                        packet.address(),
                        size,
                    )?;
                    MemCheckerMonitorCompletion::Write(result)
                }
            }
            MemProbePacketAccess::Other => unreachable!("access was checked above"),
        };
        self.pending.remove(&packet.packet_id());
        Ok(Some(completion))
    }

    pub fn observe_functional(&mut self, address: u64, size: usize) -> Result<(), StatsError> {
        self.checker.reset_range(address, size)
    }

    pub fn snapshot(&self) -> MemCheckerMonitorSnapshot {
        MemCheckerMonitorSnapshot::new(
            self.checker.snapshot(),
            self.pending.values().copied().collect(),
        )
    }
}

impl Default for MemCheckerMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn tracks_checker_access(access: MemProbePacketAccess) -> bool {
    matches!(
        access,
        MemProbePacketAccess::Read | MemProbePacketAccess::Write
    )
}

fn packet_size(packet: &MemProbePacket) -> usize {
    packet.size() as usize
}

fn validate_request_data<'a>(
    packet: &MemProbePacket,
    data: Option<&'a [u8]>,
) -> Result<&'a [u8], StatsError> {
    let data = data.ok_or(StatsError::MemCheckerMonitorRequestDataMissing {
        packet_id: packet.packet_id(),
    })?;
    if data.len() as u64 != packet.size() {
        return Err(StatsError::MemCheckerMonitorRequestDataSizeMismatch {
            packet_id: packet.packet_id(),
            packet_size: packet.size(),
            data_size: data.len(),
        });
    }
    Ok(data)
}

fn validate_response_data<'a>(
    packet: &MemProbePacket,
    data: Option<&'a [u8]>,
) -> Result<&'a [u8], StatsError> {
    let data = data.ok_or(StatsError::MemCheckerMonitorResponseDataMissing {
        packet_id: packet.packet_id(),
    })?;
    if data.len() as u64 != packet.size() {
        return Err(StatsError::MemCheckerMonitorResponseDataSizeMismatch {
            packet_id: packet.packet_id(),
            packet_size: packet.size(),
            data_size: data.len(),
        });
    }
    Ok(data)
}

fn validate_response_packet(
    packet: &MemProbePacket,
    pending: MemCheckerMonitorPendingTransaction,
) -> Result<(), StatsError> {
    if pending.access() != packet.access() {
        return Err(StatsError::MemCheckerMonitorResponseAccessMismatch {
            packet_id: packet.packet_id(),
            request_access: pending.access(),
            response_access: packet.access(),
        });
    }
    if pending.address() != packet.address() {
        return Err(StatsError::MemCheckerMonitorResponseAddressMismatch {
            packet_id: packet.packet_id(),
            request_address: pending.address(),
            response_address: packet.address(),
        });
    }
    if pending.size() != packet.size() {
        return Err(StatsError::MemCheckerMonitorResponseSizeMismatch {
            packet_id: packet.packet_id(),
            request_size: pending.size(),
            response_size: packet.size(),
        });
    }
    Ok(())
}

fn validate_pending_transactions(
    pending: &[MemCheckerMonitorPendingTransaction],
    checker: &MemCheckerSnapshot,
) -> Result<BTreeMap<u64, MemCheckerMonitorPendingTransaction>, StatsError> {
    let mut by_serial = BTreeSet::new();
    let mut by_packet = BTreeMap::new();
    for transaction in pending {
        if !tracks_checker_access(transaction.access()) {
            return Err(StatsError::InvalidMemCheckerMonitorPendingAccess {
                packet_id: transaction.packet_id(),
                access: transaction.access(),
            });
        }
        if transaction.serial() == 0 || transaction.serial() >= checker.next_serial() {
            return Err(StatsError::MemCheckerMonitorPendingSerialNotAllocated {
                packet_id: transaction.packet_id(),
                serial: transaction.serial(),
                next_serial: checker.next_serial(),
            });
        }
        if transaction.size() == 0 {
            return Err(StatsError::InvalidMemCheckerAccessSize { size: 0 });
        }
        if by_packet
            .insert(transaction.packet_id(), *transaction)
            .is_some()
        {
            return Err(StatsError::DuplicateMemCheckerMonitorPendingPacket {
                packet_id: transaction.packet_id(),
            });
        }
        if !by_serial.insert(transaction.serial()) {
            return Err(StatsError::DuplicateMemCheckerMonitorPendingSerial {
                serial: transaction.serial(),
            });
        }
    }
    Ok(by_packet)
}
