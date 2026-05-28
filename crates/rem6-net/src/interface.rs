use std::collections::BTreeMap;

use crate::{EthernetPacket, NetworkError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EthernetInterfaceId(pub u16);

impl EthernetInterfaceId {
    pub const fn new(interface: u16) -> Self {
        Self(interface)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EthernetInterfaceEventKind {
    SendDone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetInterfaceRegistry {
    interfaces: Vec<EthernetInterfaceState>,
    names: BTreeMap<String, EthernetInterfaceId>,
}

impl EthernetInterfaceRegistry {
    pub const fn new() -> Self {
        Self {
            interfaces: Vec::new(),
            names: BTreeMap::new(),
        }
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
    ) -> Result<EthernetInterfaceId, NetworkError> {
        let name = name.into();
        if self.names.contains_key(&name) {
            return Err(NetworkError::DuplicateEthernetInterfaceName { name });
        }
        let raw_id = u16::try_from(self.interfaces.len()).map_err(|_| {
            NetworkError::EthernetInterfaceCountOverflow {
                interface_count: self.interfaces.len(),
            }
        })?;
        let id = EthernetInterfaceId::new(raw_id);
        self.interfaces.push(EthernetInterfaceState {
            name: name.clone(),
            peer: None,
            busy: false,
            receive_count: 0,
            send_done_count: 0,
            last_receive_tick: None,
            last_send_done_tick: None,
        });
        self.names.insert(name, id);
        Ok(id)
    }

    pub fn interface_count(&self) -> usize {
        self.interfaces.len()
    }

    pub fn name(&self, interface: EthernetInterfaceId) -> Result<&str, NetworkError> {
        Ok(&self.state(interface)?.name)
    }

    pub fn bind_pair(
        &mut self,
        local: EthernetInterfaceId,
        peer: EthernetInterfaceId,
    ) -> Result<EthernetInterfaceBinding, NetworkError> {
        if local == peer {
            self.validate_interface(local)?;
            return Err(NetworkError::EthernetInterfaceSelfBinding { interface: local });
        }
        self.validate_interface(local)?;
        self.validate_interface(peer)?;
        self.ensure_peer_available(local, peer)?;
        self.ensure_peer_available(peer, local)?;
        self.state_mut(local)?.peer = Some(peer);
        self.state_mut(peer)?.peer = Some(local);
        Ok(EthernetInterfaceBinding { local, peer })
    }

    pub fn unbind(&mut self, interface: EthernetInterfaceId) -> Result<(), NetworkError> {
        self.validate_interface(interface)?;
        let peer = self.state(interface)?.peer;
        self.state_mut(interface)?.peer = None;
        if let Some(peer) = peer {
            self.state_mut(peer)?.peer = None;
        }
        Ok(())
    }

    pub fn peer_of(
        &self,
        interface: EthernetInterfaceId,
    ) -> Result<Option<EthernetInterfaceId>, NetworkError> {
        Ok(self.state(interface)?.peer)
    }

    pub fn is_connected(&self, interface: EthernetInterfaceId) -> Result<bool, NetworkError> {
        Ok(self.peer_of(interface)?.is_some())
    }

    pub fn set_busy(
        &mut self,
        interface: EthernetInterfaceId,
        busy: bool,
    ) -> Result<(), NetworkError> {
        self.state_mut(interface)?.busy = busy;
        Ok(())
    }

    pub fn is_busy(&self, interface: EthernetInterfaceId) -> Result<bool, NetworkError> {
        Ok(self.state(interface)?.busy)
    }

    pub fn ask_busy(&self, interface: EthernetInterfaceId) -> Result<bool, NetworkError> {
        Ok(self
            .peer_of(interface)?
            .map(|peer| self.state(peer).map(|state| state.busy))
            .transpose()?
            .unwrap_or(false))
    }

    pub fn send_packet(
        &mut self,
        source: EthernetInterfaceId,
        packet: EthernetPacket,
        tick: u64,
    ) -> Result<EthernetInterfaceSendRecord, NetworkError> {
        let peer = self.peer_of(source)?;
        if let Some(peer) = peer {
            let peer_state = self.state_mut(peer)?;
            peer_state.receive_count = peer_state.receive_count.saturating_add(1);
            peer_state.last_receive_tick = Some(tick);
        }
        Ok(EthernetInterfaceSendRecord {
            source,
            peer,
            tick,
            accepted: true,
            packet,
        })
    }

    pub fn recv_done(
        &mut self,
        interface: EthernetInterfaceId,
        tick: u64,
    ) -> Result<EthernetInterfaceEvent, NetworkError> {
        let Some(peer) = self.peer_of(interface)? else {
            return Err(NetworkError::EthernetInterfacePeerMissing { interface });
        };
        let peer_state = self.state_mut(peer)?;
        peer_state.send_done_count = peer_state.send_done_count.saturating_add(1);
        peer_state.last_send_done_tick = Some(tick);
        Ok(EthernetInterfaceEvent {
            interface: peer,
            peer: interface,
            tick,
            kind: EthernetInterfaceEventKind::SendDone,
        })
    }

    pub fn receive_count(&self, interface: EthernetInterfaceId) -> Result<u64, NetworkError> {
        Ok(self.state(interface)?.receive_count)
    }

    pub fn send_done_count(&self, interface: EthernetInterfaceId) -> Result<u64, NetworkError> {
        Ok(self.state(interface)?.send_done_count)
    }

    pub fn last_receive_tick(
        &self,
        interface: EthernetInterfaceId,
    ) -> Result<Option<u64>, NetworkError> {
        Ok(self.state(interface)?.last_receive_tick)
    }

    pub fn last_send_done_tick(
        &self,
        interface: EthernetInterfaceId,
    ) -> Result<Option<u64>, NetworkError> {
        Ok(self.state(interface)?.last_send_done_tick)
    }

    pub fn snapshot(&self) -> EthernetInterfaceRegistrySnapshot {
        EthernetInterfaceRegistrySnapshot {
            interfaces: self.interfaces.clone(),
            names: self.names.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &EthernetInterfaceRegistrySnapshot,
    ) -> Result<(), NetworkError> {
        self.interfaces = snapshot.interfaces.clone();
        self.names = snapshot.names.clone();
        Ok(())
    }

    fn validate_interface(&self, interface: EthernetInterfaceId) -> Result<(), NetworkError> {
        if interface.index() >= self.interfaces.len() {
            return Err(NetworkError::UnknownEthernetInterface {
                interface,
                interface_count: self.interfaces.len(),
            });
        }
        Ok(())
    }

    fn ensure_peer_available(
        &self,
        interface: EthernetInterfaceId,
        requested_peer: EthernetInterfaceId,
    ) -> Result<(), NetworkError> {
        if let Some(current_peer) = self.state(interface)?.peer {
            if current_peer != requested_peer {
                return Err(NetworkError::EthernetInterfacePeerAlreadyBound {
                    interface,
                    current_peer,
                    requested_peer,
                });
            }
        }
        Ok(())
    }

    fn state(
        &self,
        interface: EthernetInterfaceId,
    ) -> Result<&EthernetInterfaceState, NetworkError> {
        self.validate_interface(interface)?;
        Ok(&self.interfaces[interface.index()])
    }

    fn state_mut(
        &mut self,
        interface: EthernetInterfaceId,
    ) -> Result<&mut EthernetInterfaceState, NetworkError> {
        self.validate_interface(interface)?;
        Ok(&mut self.interfaces[interface.index()])
    }
}

impl Default for EthernetInterfaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EthernetInterfaceBinding {
    local: EthernetInterfaceId,
    peer: EthernetInterfaceId,
}

impl EthernetInterfaceBinding {
    pub const fn local(&self) -> EthernetInterfaceId {
        self.local
    }

    pub const fn peer(&self) -> EthernetInterfaceId {
        self.peer
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetInterfaceSendRecord {
    source: EthernetInterfaceId,
    peer: Option<EthernetInterfaceId>,
    tick: u64,
    accepted: bool,
    packet: EthernetPacket,
}

impl EthernetInterfaceSendRecord {
    pub const fn source(&self) -> EthernetInterfaceId {
        self.source
    }

    pub const fn peer(&self) -> Option<EthernetInterfaceId> {
        self.peer
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn accepted(&self) -> bool {
        self.accepted
    }

    pub const fn packet(&self) -> &EthernetPacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetInterfaceEvent {
    interface: EthernetInterfaceId,
    peer: EthernetInterfaceId,
    tick: u64,
    kind: EthernetInterfaceEventKind,
}

impl EthernetInterfaceEvent {
    pub const fn interface(&self) -> EthernetInterfaceId {
        self.interface
    }

    pub const fn peer(&self) -> EthernetInterfaceId {
        self.peer
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn kind(&self) -> EthernetInterfaceEventKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EthernetInterfaceRegistrySnapshot {
    interfaces: Vec<EthernetInterfaceState>,
    names: BTreeMap<String, EthernetInterfaceId>,
}

impl EthernetInterfaceRegistrySnapshot {
    pub fn interface_count(&self) -> usize {
        self.interfaces.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EthernetInterfaceState {
    name: String,
    peer: Option<EthernetInterfaceId>,
    busy: bool,
    receive_count: u64,
    send_done_count: u64,
    last_receive_tick: Option<u64>,
    last_send_done_tick: Option<u64>,
}
