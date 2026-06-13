use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::error::StatsError;
use crate::stat_metadata::StatPathError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProbePointId(u64);

impl ProbePointId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProbeListenerId(u64);

impl ProbeListenerId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemProbePacketKind {
    Request,
    Response,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemProbePacketAccess {
    Other,
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemProbePacket {
    address: u64,
    kind: MemProbePacketKind,
    access: MemProbePacketAccess,
    command: u32,
    flags: u64,
    size: u64,
    packet_id: u64,
    program_counter: u64,
}

impl MemProbePacket {
    pub const fn new(address: u64, kind: MemProbePacketKind) -> Self {
        Self {
            address,
            kind,
            access: MemProbePacketAccess::Other,
            command: 0,
            flags: 0,
            size: 0,
            packet_id: 0,
            program_counter: 0,
        }
    }

    pub const fn request(address: u64) -> Self {
        Self::new(address, MemProbePacketKind::Request)
    }

    pub const fn response(address: u64) -> Self {
        Self::new(address, MemProbePacketKind::Response)
    }

    pub const fn read(address: u64) -> Self {
        Self::request(address).with_access(MemProbePacketAccess::Read)
    }

    pub const fn write(address: u64) -> Self {
        Self::request(address).with_access(MemProbePacketAccess::Write)
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn kind(self) -> MemProbePacketKind {
        self.kind
    }

    pub const fn access(self) -> MemProbePacketAccess {
        self.access
    }

    pub const fn command(self) -> u32 {
        self.command
    }

    pub const fn flags(self) -> u64 {
        self.flags
    }

    pub const fn size(self) -> u64 {
        self.size
    }

    pub const fn packet_id(self) -> u64 {
        self.packet_id
    }

    pub const fn program_counter(self) -> u64 {
        self.program_counter
    }

    pub const fn with_access(mut self, access: MemProbePacketAccess) -> Self {
        self.access = access;
        self
    }

    pub const fn with_command(mut self, command: u32) -> Self {
        self.command = command;
        self
    }

    pub const fn with_flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    pub const fn with_size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    pub const fn with_packet_id(mut self, packet_id: u64) -> Self {
        self.packet_id = packet_id;
        self
    }

    pub const fn with_program_counter(mut self, program_counter: u64) -> Self {
        self.program_counter = program_counter;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProbePayload {
    Unit,
    Counter { amount: u64 },
    ProgramCounter { pc: u64 },
    MemoryPacket(MemProbePacket),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeListenerRef {
    id: ProbeListenerId,
    name: String,
}

impl ProbeListenerRef {
    pub fn new(id: ProbeListenerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }

    pub const fn id(&self) -> ProbeListenerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeEvent {
    tick: Tick,
    sequence: u64,
    point: ProbePointId,
    listeners: Vec<ProbeListenerRef>,
    payload: ProbePayload,
}

impl ProbeEvent {
    pub fn new(
        tick: Tick,
        sequence: u64,
        point: ProbePointId,
        listeners: Vec<ProbeListenerRef>,
        payload: ProbePayload,
    ) -> Self {
        Self {
            tick,
            sequence,
            point,
            listeners,
            payload,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn point(&self) -> ProbePointId {
        self.point
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    pub fn listener_refs(&self) -> &[ProbeListenerRef] {
        &self.listeners
    }

    pub const fn payload(&self) -> &ProbePayload {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeSnapshot {
    points: Vec<(String, String, ProbePointId)>,
    listeners: Vec<(String, ProbePointId, ProbeListenerId)>,
    events: Vec<ProbeEvent>,
    next_point: u64,
    next_listener: u64,
    next_sequence: u64,
}

impl ProbeSnapshot {
    pub fn new(
        points: Vec<(String, String, ProbePointId)>,
        listeners: Vec<(String, ProbePointId, ProbeListenerId)>,
        events: Vec<ProbeEvent>,
    ) -> Self {
        let next_point = next_probe_point_cursor(&points);
        let next_listener = next_probe_listener_cursor(&listeners);
        let next_sequence = next_probe_event_cursor(&events);
        Self::with_cursors(
            points,
            listeners,
            events,
            next_point,
            next_listener,
            next_sequence,
        )
    }

    pub const fn with_cursors(
        points: Vec<(String, String, ProbePointId)>,
        listeners: Vec<(String, ProbePointId, ProbeListenerId)>,
        events: Vec<ProbeEvent>,
        next_point: u64,
        next_listener: u64,
        next_sequence: u64,
    ) -> Self {
        Self {
            points,
            listeners,
            events,
            next_point,
            next_listener,
            next_sequence,
        }
    }

    pub fn points(&self) -> &[(String, String, ProbePointId)] {
        &self.points
    }

    pub fn listeners(&self) -> &[(String, ProbePointId, ProbeListenerId)] {
        &self.listeners
    }

    pub fn events(&self) -> &[ProbeEvent] {
        &self.events
    }

    pub const fn next_point(&self) -> u64 {
        self.next_point
    }

    pub const fn next_listener(&self) -> u64 {
        self.next_listener
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeRegistry {
    next_point: u64,
    next_listener: u64,
    next_sequence: u64,
    point_names: BTreeSet<(String, String)>,
    points: BTreeMap<ProbePointId, ProbePointRecord>,
    listeners: BTreeMap<ProbeListenerId, ProbeListenerRecord>,
    point_listeners: BTreeMap<ProbePointId, BTreeSet<ProbeListenerId>>,
    events: Vec<ProbeEvent>,
}

impl ProbeRegistry {
    pub fn new() -> Self {
        Self {
            next_point: 0,
            next_listener: 0,
            next_sequence: 0,
            point_names: BTreeSet::new(),
            points: BTreeMap::new(),
            listeners: BTreeMap::new(),
            point_listeners: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub fn register_point(
        &mut self,
        component: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<ProbePointId, StatsError> {
        let component = component.into();
        let name = name.into();
        validate_probe_component(&component)?;
        validate_probe_name(&name)?;
        if self
            .point_names
            .contains(&(component.clone(), name.clone()))
        {
            return Err(StatsError::DuplicateProbePoint { component, name });
        }
        let next_point = self
            .next_point
            .checked_add(1)
            .ok_or(StatsError::ProbePointSequenceOverflow)?;

        let id = ProbePointId::new(self.next_point);
        self.next_point = next_point;
        self.point_names.insert((component.clone(), name.clone()));
        self.points.insert(id, ProbePointRecord { component, name });
        self.point_listeners.insert(id, BTreeSet::new());
        Ok(id)
    }

    pub fn add_listener(
        &mut self,
        point: ProbePointId,
        name: impl Into<String>,
    ) -> Result<ProbeListenerId, StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        let name = name.into();
        validate_probe_listener_name(&name)?;
        if self
            .listeners
            .values()
            .any(|listener| listener.point == point && listener.name == name)
        {
            return Err(StatsError::DuplicateProbeListener { point, name });
        }

        let id = ProbeListenerId::new(self.next_listener);
        let next_listener = self
            .next_listener
            .checked_add(1)
            .ok_or(StatsError::ProbeListenerSequenceOverflow)?;
        self.next_listener = next_listener;
        self.listeners
            .insert(id, ProbeListenerRecord { point, name });
        self.point_listeners.entry(point).or_default().insert(id);
        Ok(id)
    }

    pub fn remove_listener(
        &mut self,
        point: ProbePointId,
        listener: ProbeListenerId,
    ) -> Result<(), StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        let record = self
            .listeners
            .remove(&listener)
            .ok_or(StatsError::UnknownProbeListener { listener })?;
        if record.point != point {
            self.listeners.insert(listener, record);
            return Err(StatsError::ProbeListenerPointMismatch { point, listener });
        }
        if let Some(listeners) = self.point_listeners.get_mut(&point) {
            listeners.remove(&listener);
        }
        Ok(())
    }

    pub fn emit(
        &mut self,
        tick: Tick,
        point: ProbePointId,
        payload: ProbePayload,
    ) -> Result<&ProbeEvent, StatsError> {
        if !self.points.contains_key(&point) {
            return Err(StatsError::UnknownProbePoint { point });
        }
        if let Some(previous) = self.events.last() {
            validate_probe_event_time(tick, previous.tick())?;
        }
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(StatsError::ProbeSequenceOverflow)?;
        let listeners = self
            .point_listeners
            .get(&point)
            .into_iter()
            .flatten()
            .map(|listener| {
                let record = self
                    .listeners
                    .get(listener)
                    .expect("point listener must have a listener record");
                ProbeListenerRef::new(*listener, record.name.clone())
            })
            .collect();
        self.events
            .push(ProbeEvent::new(tick, sequence, point, listeners, payload));
        Ok(self.events.last().expect("probe event was just appended"))
    }

    pub fn events(&self) -> &[ProbeEvent] {
        &self.events
    }

    pub fn snapshot(&self) -> ProbeSnapshot {
        let points = self
            .points
            .iter()
            .map(|(id, point)| (point.component.clone(), point.name.clone(), *id))
            .collect();
        let listeners = self
            .listeners
            .iter()
            .map(|(id, listener)| (listener.name.clone(), listener.point, *id))
            .collect();
        ProbeSnapshot::with_cursors(
            points,
            listeners,
            self.events.clone(),
            self.next_point,
            self.next_listener,
            self.next_sequence,
        )
    }

    pub fn restore(&mut self, snapshot: &ProbeSnapshot) -> Result<(), StatsError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn from_snapshot(snapshot: &ProbeSnapshot) -> Result<Self, StatsError> {
        let mut point_names = BTreeSet::new();
        let mut points = BTreeMap::new();
        let mut point_listeners = BTreeMap::new();
        let mut highest_point = None;
        for (component, name, point) in snapshot.points() {
            validate_probe_point_fields(component, name)?;
            if points.contains_key(point) {
                return Err(StatsError::DuplicateProbePointId { point: *point });
            }
            if !point_names.insert((component.clone(), name.clone())) {
                return Err(StatsError::DuplicateProbePoint {
                    component: component.clone(),
                    name: name.clone(),
                });
            }
            points.insert(
                *point,
                ProbePointRecord {
                    component: component.clone(),
                    name: name.clone(),
                },
            );
            point_listeners.insert(*point, BTreeSet::new());
            highest_point = max_probe_point(highest_point, *point);
        }
        validate_probe_point_cursor(snapshot.next_point(), highest_point)?;

        let mut listeners = BTreeMap::new();
        let mut listener_names = BTreeSet::new();
        let mut highest_listener = None;
        for (name, point, listener) in snapshot.listeners() {
            if !points.contains_key(point) {
                return Err(StatsError::UnknownProbePoint { point: *point });
            }
            validate_probe_listener_name(name)?;
            if listeners.contains_key(listener) {
                return Err(StatsError::DuplicateProbeListenerId {
                    listener: *listener,
                });
            }
            if !listener_names.insert((*point, name.clone())) {
                return Err(StatsError::DuplicateProbeListener {
                    point: *point,
                    name: name.clone(),
                });
            }
            listeners.insert(
                *listener,
                ProbeListenerRecord {
                    point: *point,
                    name: name.clone(),
                },
            );
            point_listeners.entry(*point).or_default().insert(*listener);
            highest_listener = max_probe_listener(highest_listener, *listener);
        }
        let mut previous_sequence = None;
        let mut previous_tick = None;
        let mut highest_sequence = None;
        for event in snapshot.events() {
            if !points.contains_key(&event.point()) {
                return Err(StatsError::UnknownProbePoint {
                    point: event.point(),
                });
            }
            if let Some(previous_sequence) = previous_sequence {
                if event.sequence() <= previous_sequence {
                    return Err(StatsError::ProbeEventSequenceNotIncreasing {
                        previous_sequence,
                        current_sequence: event.sequence(),
                    });
                }
            }
            if let Some(previous_tick) = previous_tick {
                validate_probe_event_time(event.tick(), previous_tick)?;
            }
            validate_probe_event_listener_refs(event.point(), event.listener_refs())?;
            for listener in event.listener_refs() {
                highest_listener = max_probe_listener(highest_listener, listener.id());
            }
            previous_sequence = Some(event.sequence());
            previous_tick = Some(event.tick());
            highest_sequence = Some(event.sequence());
        }
        validate_probe_listener_cursor(snapshot.next_listener(), highest_listener)?;
        validate_probe_event_cursor(snapshot.next_sequence(), highest_sequence)?;

        Ok(Self {
            next_point: snapshot.next_point(),
            next_listener: snapshot.next_listener(),
            next_sequence: snapshot.next_sequence(),
            point_names,
            points,
            listeners,
            point_listeners,
            events: snapshot.events.clone(),
        })
    }
}

impl Default for ProbeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbePointRecord {
    component: String,
    name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeListenerRecord {
    point: ProbePointId,
    name: String,
}

fn validate_probe_point_fields(component: &str, name: &str) -> Result<(), StatsError> {
    validate_probe_component(component)?;
    validate_probe_name(name)
}

fn validate_probe_component(component: &str) -> Result<(), StatsError> {
    if component.is_empty() {
        return Err(StatsError::EmptyProbeComponent);
    }
    validate_probe_identifier(component).map_err(|reason| StatsError::InvalidProbeComponent {
        component: component.to_string(),
        reason,
    })
}

fn validate_probe_name(name: &str) -> Result<(), StatsError> {
    if name.is_empty() {
        return Err(StatsError::EmptyProbeName);
    }
    validate_probe_identifier(name).map_err(|reason| StatsError::InvalidProbeName {
        name: name.to_string(),
        reason,
    })
}

fn validate_probe_listener_name(name: &str) -> Result<(), StatsError> {
    if name.is_empty() {
        return Err(StatsError::EmptyProbeListenerName);
    }
    validate_probe_identifier(name).map_err(|reason| StatsError::InvalidProbeListenerName {
        name: name.to_string(),
        reason,
    })
}

fn validate_probe_event_listener_refs(
    point: ProbePointId,
    listeners: &[ProbeListenerRef],
) -> Result<(), StatsError> {
    let mut listener_ids = BTreeSet::new();
    let mut listener_names = BTreeSet::new();
    for listener in listeners {
        validate_probe_listener_name(listener.name())?;
        if !listener_ids.insert(listener.id()) {
            return Err(StatsError::DuplicateProbeListenerId {
                listener: listener.id(),
            });
        }
        if !listener_names.insert(listener.name().to_string()) {
            return Err(StatsError::DuplicateProbeListener {
                point,
                name: listener.name().to_string(),
            });
        }
    }
    Ok(())
}

fn validate_probe_identifier(identifier: &str) -> Result<(), StatPathError> {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return Err(StatPathError::EmptySegment { index: 0 });
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return Err(StatPathError::InvalidSegmentStart {
            segment: identifier.to_string(),
            character: first,
        });
    }
    for character in chars {
        if !character.is_ascii_alphanumeric() && character != '_' {
            return Err(StatPathError::InvalidSegmentCharacter {
                segment: identifier.to_string(),
                character,
            });
        }
    }
    Ok(())
}

fn next_probe_point_cursor(points: &[(String, String, ProbePointId)]) -> u64 {
    points
        .iter()
        .map(|(_, _, point)| point.get())
        .max()
        .map_or(0, |point| point.saturating_add(1))
}

fn next_probe_listener_cursor(listeners: &[(String, ProbePointId, ProbeListenerId)]) -> u64 {
    listeners
        .iter()
        .map(|(_, _, listener)| listener.get())
        .max()
        .map_or(0, |listener| listener.saturating_add(1))
}

fn next_probe_event_cursor(events: &[ProbeEvent]) -> u64 {
    events
        .iter()
        .map(ProbeEvent::sequence)
        .max()
        .map_or(0, |sequence| sequence.saturating_add(1))
}

fn validate_probe_point_cursor(
    next_point: u64,
    highest_point: Option<ProbePointId>,
) -> Result<(), StatsError> {
    let Some(highest_point) = highest_point else {
        return Ok(());
    };
    let Some(required_next_point) = highest_point.get().checked_add(1) else {
        return Err(StatsError::ProbePointSequenceOverflow);
    };
    if next_point < required_next_point {
        return Err(StatsError::ProbePointCursorBehind {
            next_point,
            highest_point,
        });
    }
    Ok(())
}

fn validate_probe_listener_cursor(
    next_listener: u64,
    highest_listener: Option<ProbeListenerId>,
) -> Result<(), StatsError> {
    let Some(highest_listener) = highest_listener else {
        return Ok(());
    };
    let Some(required_next_listener) = highest_listener.get().checked_add(1) else {
        return Err(StatsError::ProbeListenerSequenceOverflow);
    };
    if next_listener < required_next_listener {
        return Err(StatsError::ProbeListenerCursorBehind {
            next_listener,
            highest_listener,
        });
    }
    Ok(())
}

fn validate_probe_event_cursor(
    next_sequence: u64,
    highest_sequence: Option<u64>,
) -> Result<(), StatsError> {
    let Some(highest_sequence) = highest_sequence else {
        return Ok(());
    };
    let Some(required_next_sequence) = highest_sequence.checked_add(1) else {
        return Err(StatsError::ProbeSequenceOverflow);
    };
    if next_sequence < required_next_sequence {
        return Err(StatsError::ProbeEventCursorBehind {
            next_sequence,
            highest_sequence,
        });
    }
    Ok(())
}

fn validate_probe_event_time(tick: Tick, previous_tick: Tick) -> Result<(), StatsError> {
    if tick < previous_tick {
        return Err(StatsError::ProbeEventTimeWentBack {
            previous_tick,
            current_tick: tick,
        });
    }
    Ok(())
}

fn max_probe_point(current: Option<ProbePointId>, candidate: ProbePointId) -> Option<ProbePointId> {
    Some(current.map_or(candidate, |current| current.max(candidate)))
}

fn max_probe_listener(
    current: Option<ProbeListenerId>,
    candidate: ProbeListenerId,
) -> Option<ProbeListenerId> {
    Some(current.map_or(candidate, |current| current.max(candidate)))
}
