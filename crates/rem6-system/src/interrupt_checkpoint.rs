use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_interrupt::{
    InterruptClaim, InterruptController, InterruptEvent, InterruptEventKind, InterruptLineId,
    InterruptPriority, InterruptRoute, InterruptSnapshot, InterruptSourceId, InterruptTargetId,
    PendingInterrupt,
};
use rem6_kernel::PartitionId;

const INTERRUPT_CHUNK: &str = "interrupt";
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptControllerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: InterruptSnapshot,
}

impl InterruptControllerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: InterruptSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &InterruptSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct InterruptControllerCheckpointPort {
    component: CheckpointComponentId,
    controller: Arc<Mutex<InterruptController>>,
}

impl InterruptControllerCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        controller: Arc<Mutex<InterruptController>>,
    ) -> Self {
        Self {
            component,
            controller,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.controller)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
        tick: u64,
    ) -> Result<InterruptControllerCheckpointRecord, CheckpointError> {
        let snapshot = self
            .controller
            .lock()
            .expect("interrupt controller lock")
            .snapshot(tick);
        registry.write_chunk(
            &self.component,
            INTERRUPT_CHUNK,
            encode_interrupt(&snapshot),
        )?;
        Ok(InterruptControllerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<InterruptControllerCheckpointRecord, InterruptControllerCheckpointError> {
        let record = self.decode_from(registry)?;
        self.controller
            .lock()
            .expect("interrupt controller lock")
            .restore(record.snapshot());
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<InterruptControllerCheckpointRecord, InterruptControllerCheckpointError> {
        let payload = registry
            .chunk(&self.component, INTERRUPT_CHUNK)
            .ok_or_else(|| InterruptControllerCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: INTERRUPT_CHUNK.to_string(),
            })?;
        let snapshot = decode_interrupt(&self.component, payload)?;
        Ok(InterruptControllerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct InterruptControllerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, InterruptControllerCheckpointPort>,
}

impl InterruptControllerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = InterruptControllerCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
        tick: u64,
    ) -> Result<Vec<InterruptControllerCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry, tick))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<InterruptControllerCheckpointRecord>, InterruptControllerCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.controller
                .lock()
                .expect("interrupt controller lock")
                .restore(record.snapshot());
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), InterruptControllerCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptControllerCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
}

impl InterruptControllerCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                component
            }
        }
    }
}

impl fmt::Display for InterruptControllerCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "interrupt checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "interrupt checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
        }
    }
}

impl Error for InterruptControllerCheckpointError {}

fn encode_interrupt(snapshot: &InterruptSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.tick());
    write_u64(&mut payload, snapshot.routes().len() as u64);
    for route in snapshot.routes() {
        encode_route(&mut payload, *route);
    }
    write_u64(&mut payload, snapshot.priorities().len() as u64);
    for (line, priority) in snapshot.priorities() {
        write_u64(&mut payload, line.get());
        write_u32(&mut payload, priority.get());
    }
    write_u64(&mut payload, snapshot.pending().len() as u64);
    for pending in snapshot.pending() {
        encode_pending(&mut payload, pending);
    }
    write_u64(&mut payload, snapshot.claimed().len() as u64);
    for claim in snapshot.claimed() {
        encode_claim(&mut payload, *claim);
    }
    write_u64(&mut payload, snapshot.history().len() as u64);
    for event in snapshot.history() {
        encode_event(&mut payload, event);
    }
    payload
}

fn decode_interrupt(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<InterruptSnapshot, InterruptControllerCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let tick = cursor.read_u64("interrupt snapshot tick")?;
    let routes = read_routes(&mut cursor)?;
    let priorities = read_priorities(&mut cursor)?;
    let pending = read_pending(&mut cursor)?;
    let claimed = read_claims(&mut cursor)?;
    let history = read_history(&mut cursor)?;
    cursor.finish()?;
    Ok(InterruptSnapshot::new(
        tick, routes, priorities, pending, claimed, history,
    ))
}

fn encode_route(payload: &mut Vec<u8>, route: InterruptRoute) {
    write_u64(payload, route.line().get());
    write_u32(payload, route.target().get());
    write_u32(payload, route.target_partition().index());
}

fn decode_route(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptRoute, InterruptControllerCheckpointError> {
    Ok(InterruptRoute::new(
        InterruptLineId::new(cursor.read_u64("interrupt route line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt route target")?),
        PartitionId::new(cursor.read_u32("interrupt route target partition")?),
    ))
}

fn read_routes(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<InterruptRoute>, InterruptControllerCheckpointError> {
    let count = cursor.read_count("interrupt route count")?;
    let mut routes = Vec::with_capacity(count);
    for _ in 0..count {
        routes.push(decode_route(cursor)?);
    }
    Ok(routes)
}

fn read_priorities(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<(InterruptLineId, InterruptPriority)>, InterruptControllerCheckpointError> {
    let count = cursor.read_count("interrupt priority count")?;
    let mut priorities = Vec::with_capacity(count);
    for _ in 0..count {
        priorities.push((
            InterruptLineId::new(cursor.read_u64("interrupt priority line")?),
            InterruptPriority::new(cursor.read_u32("interrupt priority value")?),
        ));
    }
    Ok(priorities)
}

fn encode_pending(payload: &mut Vec<u8>, pending: &PendingInterrupt) {
    write_u64(payload, pending.line().get());
    write_u32(payload, pending.target().get());
    write_u32(payload, pending.target_partition().index());
    write_u32(payload, pending.source().get());
    write_u64(payload, pending.asserted_tick());
}

fn decode_pending(
    cursor: &mut PayloadCursor<'_>,
) -> Result<PendingInterrupt, InterruptControllerCheckpointError> {
    Ok(PendingInterrupt::routed(
        InterruptLineId::new(cursor.read_u64("interrupt pending line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt pending target")?),
        PartitionId::new(cursor.read_u32("interrupt pending target partition")?),
        InterruptSourceId::new(cursor.read_u32("interrupt pending source")?),
        cursor.read_u64("interrupt pending asserted tick")?,
    ))
}

fn read_pending(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<PendingInterrupt>, InterruptControllerCheckpointError> {
    let count = cursor.read_count("interrupt pending count")?;
    let mut pending = Vec::with_capacity(count);
    for _ in 0..count {
        pending.push(decode_pending(cursor)?);
    }
    Ok(pending)
}

fn encode_claim(payload: &mut Vec<u8>, claim: InterruptClaim) {
    write_u64(payload, claim.line().get());
    write_u32(payload, claim.target().get());
    write_u32(payload, claim.target_partition().index());
    write_u32(payload, claim.source().get());
    write_u64(payload, claim.asserted_tick());
    write_u64(payload, claim.claimed_tick());
}

fn decode_claim(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptClaim, InterruptControllerCheckpointError> {
    Ok(InterruptClaim::new(
        InterruptLineId::new(cursor.read_u64("interrupt claim line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt claim target")?),
        PartitionId::new(cursor.read_u32("interrupt claim target partition")?),
        InterruptSourceId::new(cursor.read_u32("interrupt claim source")?),
        cursor.read_u64("interrupt claim asserted tick")?,
        cursor.read_u64("interrupt claim claimed tick")?,
    ))
}

fn read_claims(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<InterruptClaim>, InterruptControllerCheckpointError> {
    let count = cursor.read_count("interrupt claim count")?;
    let mut claims = Vec::with_capacity(count);
    for _ in 0..count {
        claims.push(decode_claim(cursor)?);
    }
    Ok(claims)
}

fn encode_event(payload: &mut Vec<u8>, event: &InterruptEvent) {
    write_u64(payload, event.tick());
    write_u64(payload, event.line().get());
    write_u32(payload, event.target().get());
    write_u32(payload, event.target_partition().index());
    write_u32(payload, event.source().get());
    encode_event_kind(payload, event.kind());
}

fn decode_event(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptEvent, InterruptControllerCheckpointError> {
    Ok(InterruptEvent::routed(
        cursor.read_u64("interrupt event tick")?,
        InterruptLineId::new(cursor.read_u64("interrupt event line")?),
        InterruptTargetId::new(cursor.read_u32("interrupt event target")?),
        PartitionId::new(cursor.read_u32("interrupt event target partition")?),
        InterruptSourceId::new(cursor.read_u32("interrupt event source")?),
        decode_event_kind(cursor)?,
    ))
}

fn read_history(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<InterruptEvent>, InterruptControllerCheckpointError> {
    let count = cursor.read_count("interrupt history count")?;
    let mut history = Vec::with_capacity(count);
    for _ in 0..count {
        history.push(decode_event(cursor)?);
    }
    Ok(history)
}

fn encode_event_kind(payload: &mut Vec<u8>, kind: InterruptEventKind) {
    write_u64(
        payload,
        match kind {
            InterruptEventKind::Assert => 0,
            InterruptEventKind::Deassert => 1,
            InterruptEventKind::Claim => 2,
            InterruptEventKind::Complete => 3,
        },
    );
}

fn decode_event_kind(
    cursor: &mut PayloadCursor<'_>,
) -> Result<InterruptEventKind, InterruptControllerCheckpointError> {
    match cursor.read_u64("interrupt event kind")? {
        0 => Ok(InterruptEventKind::Assert),
        1 => Ok(InterruptEventKind::Deassert),
        2 => Ok(InterruptEventKind::Claim),
        3 => Ok(InterruptEventKind::Complete),
        value => Err(cursor.invalid(format!("interrupt event kind has invalid value {value}"))),
    }
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    position: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            position: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, InterruptControllerCheckpointError> {
        usize::try_from(self.read_u64(name)?)
            .map_err(|_| self.invalid(format!("{name} cannot fit in usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, InterruptControllerCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        let mut value = [0; U32_BYTES];
        value.copy_from_slice(bytes);
        Ok(u32::from_le_bytes(value))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, InterruptControllerCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        let mut value = [0; U64_BYTES];
        value.copy_from_slice(bytes);
        Ok(u64::from_le_bytes(value))
    }

    fn read_bytes(
        &mut self,
        name: &str,
        count: usize,
    ) -> Result<&'a [u8], InterruptControllerCheckpointError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| self.invalid(format!("{name} at offset {} overflows", self.position)))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} at offset {} needs {count} bytes but payload has {} remaining",
                self.position,
                self.payload.len().saturating_sub(self.position)
            )));
        }

        let bytes = &self.payload[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), InterruptControllerCheckpointError> {
        if self.position == self.payload.len() {
            return Ok(());
        }

        Err(self.invalid(format!(
            "payload has {} trailing bytes",
            self.payload.len() - self.position
        )))
    }

    fn invalid(&self, reason: String) -> InterruptControllerCheckpointError {
        InterruptControllerCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
