use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;
use rem6_memory::Address;

use crate::ProtoError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoSimAdapterKind {
    SystemC,
    Tlm,
    Sst,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CoSimEndpointId(String);

impl CoSimEndpointId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProtoError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProtoError::EmptyCoSimEndpoint);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoSimEndpoint {
    id: CoSimEndpointId,
    kind: CoSimAdapterKind,
    tick_frequency_hz: u64,
}

impl CoSimEndpoint {
    pub fn new(
        id: CoSimEndpointId,
        kind: CoSimAdapterKind,
        tick_frequency_hz: u64,
    ) -> Result<Self, ProtoError> {
        if tick_frequency_hz == 0 {
            return Err(ProtoError::ZeroCoSimEndpointTickFrequency);
        }
        Ok(Self {
            id,
            kind,
            tick_frequency_hz,
        })
    }

    pub const fn id(&self) -> &CoSimEndpointId {
        &self.id
    }

    pub const fn kind(&self) -> CoSimAdapterKind {
        self.kind
    }

    pub const fn tick_frequency_hz(&self) -> u64 {
        self.tick_frequency_hz
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoSimEventKind {
    ClockAdvance,
    TlmTransaction,
    Interrupt,
    TrafficPacket,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoSimEvent {
    sequence: u64,
    endpoint: CoSimEndpointId,
    tick: Tick,
    kind: CoSimEventKind,
    address: Option<Address>,
    size: Option<u32>,
    payload: Vec<u8>,
}

impl CoSimEvent {
    pub fn new(sequence: u64, endpoint: CoSimEndpointId, tick: Tick, kind: CoSimEventKind) -> Self {
        Self {
            sequence,
            endpoint,
            tick,
            kind,
            address: None,
            size: None,
            payload: Vec::new(),
        }
    }

    pub const fn with_address(mut self, address: Address) -> Self {
        self.address = Some(address);
        self
    }

    pub const fn with_size(mut self, size: u32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn with_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn endpoint(&self) -> &CoSimEndpointId {
        &self.endpoint
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> CoSimEventKind {
        self.kind
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn size(&self) -> Option<u32> {
        self.size
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoSimEventReceipt {
    sequence: u64,
    endpoint: CoSimEndpointId,
    kind: CoSimEventKind,
    issued_tick: Tick,
    accepted_tick: Tick,
}

impl CoSimEventReceipt {
    fn new(event: &CoSimEvent, accepted_tick: Tick) -> Self {
        Self {
            sequence: event.sequence,
            endpoint: event.endpoint.clone(),
            kind: event.kind,
            issued_tick: event.tick,
            accepted_tick,
        }
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn endpoint(&self) -> &CoSimEndpointId {
        &self.endpoint
    }

    pub const fn kind(&self) -> CoSimEventKind {
        self.kind
    }

    pub const fn issued_tick(&self) -> Tick {
        self.issued_tick
    }

    pub const fn accepted_tick(&self) -> Tick {
        self.accepted_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoSimAdapterSnapshot {
    endpoints: Vec<CoSimEndpoint>,
    completed_events: Vec<CoSimEventReceipt>,
}

impl CoSimAdapterSnapshot {
    pub fn endpoints(&self) -> &[CoSimEndpoint] {
        &self.endpoints
    }

    pub fn completed_events(&self) -> &[CoSimEventReceipt] {
        &self.completed_events
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CoSimAdapterBoundary {
    endpoints: BTreeMap<CoSimEndpointId, CoSimEndpoint>,
    pending_events: Vec<CoSimEvent>,
    completed_events: Vec<CoSimEventReceipt>,
    seen_sequences: BTreeSet<u64>,
}

impl CoSimAdapterBoundary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_endpoint(&mut self, endpoint: CoSimEndpoint) -> Result<(), ProtoError> {
        let id = endpoint.id().clone();
        if self.endpoints.contains_key(&id) {
            return Err(ProtoError::DuplicateCoSimEndpoint {
                endpoint: id.as_str().to_string(),
            });
        }
        self.endpoints.insert(id, endpoint);
        Ok(())
    }

    pub fn endpoint(&self, id: &CoSimEndpointId) -> Option<&CoSimEndpoint> {
        self.endpoints.get(id)
    }

    pub fn pending_events(&self) -> &[CoSimEvent] {
        &self.pending_events
    }

    pub fn completed_events(&self) -> &[CoSimEventReceipt] {
        &self.completed_events
    }

    pub fn handoff_event(&mut self, event: CoSimEvent) -> Result<(), ProtoError> {
        if event.sequence == 0 {
            return Err(ProtoError::ZeroCoSimEventSequence);
        }
        if !self.endpoints.contains_key(event.endpoint()) {
            return Err(ProtoError::UnknownCoSimEndpoint {
                endpoint: event.endpoint().as_str().to_string(),
            });
        }
        if event.size == Some(0) {
            return Err(ProtoError::ZeroCoSimEventSize {
                sequence: event.sequence,
            });
        }
        let is_data_event = matches!(
            event.kind,
            CoSimEventKind::TlmTransaction | CoSimEventKind::TrafficPacket
        );
        if is_data_event {
            let Some(size) = event.size else {
                return Err(ProtoError::MissingCoSimEventShape {
                    sequence: event.sequence,
                    kind: event.kind,
                });
            };
            if event.address.is_none() {
                return Err(ProtoError::MissingCoSimEventShape {
                    sequence: event.sequence,
                    kind: event.kind,
                });
            }
            let expected_bytes = size as usize;
            let actual_bytes = event.payload.len();
            if actual_bytes != expected_bytes {
                return Err(ProtoError::CoSimEventPayloadSizeMismatch {
                    sequence: event.sequence,
                    expected_bytes,
                    actual_bytes,
                });
            }
        }
        if !self.seen_sequences.insert(event.sequence) {
            return Err(ProtoError::DuplicateCoSimEvent {
                sequence: event.sequence,
            });
        }
        self.pending_events.push(event);
        Ok(())
    }

    pub fn acknowledge_event(
        &mut self,
        sequence: u64,
        accepted_tick: Tick,
    ) -> Result<CoSimEventReceipt, ProtoError> {
        let Some(index) = self
            .pending_events
            .iter()
            .position(|event| event.sequence == sequence)
        else {
            return Err(ProtoError::UnknownCoSimEvent { sequence });
        };
        let event = self.pending_events.remove(index);
        let receipt = CoSimEventReceipt::new(&event, accepted_tick);
        self.completed_events.push(receipt.clone());
        Ok(receipt)
    }

    pub fn snapshot(&self) -> Result<CoSimAdapterSnapshot, ProtoError> {
        if !self.pending_events.is_empty() {
            return Err(ProtoError::CoSimCheckpointHasPendingEvents {
                pending: self.pending_events.len(),
            });
        }
        Ok(CoSimAdapterSnapshot {
            endpoints: self.endpoints.values().cloned().collect(),
            completed_events: self.completed_events.clone(),
        })
    }

    pub fn restore(snapshot: CoSimAdapterSnapshot) -> Result<Self, ProtoError> {
        let mut boundary = Self::new();
        for endpoint in snapshot.endpoints {
            boundary.register_endpoint(endpoint)?;
        }
        for receipt in snapshot.completed_events {
            if receipt.sequence == 0 {
                return Err(ProtoError::ZeroCoSimEventSequence);
            }
            if !boundary.seen_sequences.insert(receipt.sequence) {
                return Err(ProtoError::DuplicateCoSimEvent {
                    sequence: receipt.sequence,
                });
            }
            boundary.completed_events.push(receipt);
        }
        Ok(boundary)
    }
}
