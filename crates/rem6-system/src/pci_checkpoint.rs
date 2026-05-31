use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_pci::{
    PciError, PciFunctionAddress, PciHostBridge, PciHostBridgeTopologySnapshot,
    PciHostOptionalCapabilityPayloadMap, PciHostRawCapabilityPayloadMap,
};

const PCI_HOST_BRIDGE_CONFIG_CHUNK: &str = "host-bridge-config-space";
const PCI_HOST_BRIDGE_BAR_CHUNK: &str = "host-bridge-bars";
const PCI_HOST_ENDPOINT_CONFIG_CHUNK: &str = "host-endpoint-config-space";
const PCI_HOST_ENDPOINT_BAR_CHUNK: &str = "host-endpoint-bars";
const PCI_HOST_ENDPOINT_RAW_CAPABILITY_CHUNK: &str = "host-endpoint-raw-capabilities";
const PCI_HOST_ENDPOINT_POWER_MANAGEMENT_CHUNK: &str = "host-endpoint-power-management";
const PCI_HOST_ENDPOINT_PCIE_CHUNK: &str = "host-endpoint-pcie";
const PCI_HOST_ENDPOINT_MSI_CHUNK: &str = "host-endpoint-msi";
const PCI_HOST_ENDPOINT_MSIX_CHUNK: &str = "host-endpoint-msix";
const PCI_HOST_TOPOLOGY_CHUNK: &str = "host-topology";
const PCI_HOST_CONFIG_SPACE_MAP_MAGIC: &[u8; 8] = b"R6PHCFG1";
const PCI_HOST_BAR_MAP_MAGIC: &[u8; 8] = b"R6PHBAR1";
const PCI_HOST_RAW_CAPABILITY_MAP_MAGIC: &[u8; 8] = b"R6PHRAW1";
const PCI_HOST_POWER_MANAGEMENT_MAP_MAGIC: &[u8; 8] = b"R6PHPM01";
const PCI_HOST_PCIE_MAP_MAGIC: &[u8; 8] = b"R6PHPCI1";
const PCI_HOST_MSI_MAP_MAGIC: &[u8; 8] = b"R6PHMSI1";
const PCI_HOST_MSIX_MAP_MAGIC: &[u8; 8] = b"R6PHMXI1";
const PCI_HOST_CONFIG_SPACE_MAP_VERSION: u16 = 1;
const PCI_HOST_BAR_MAP_VERSION: u16 = 1;
const PCI_HOST_CAPABILITY_MAP_VERSION: u16 = 1;
const PCI_BAR_SLOT_ABSENT: u8 = 0;
const PCI_BAR_SLOT_PRESENT: u8 = 1;
const PCI_CAPABILITY_PAYLOAD_ABSENT: u8 = 0;
const PCI_CAPABILITY_PAYLOAD_PRESENT: u8 = 1;
const U16_BYTES: usize = 2;
const U32_BYTES: usize = 4;

pub type PciHostBarPayloadMap = BTreeMap<PciFunctionAddress, Vec<Option<Vec<u8>>>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PciHostEndpointCapabilityPayloads {
    raw_capabilities: PciHostRawCapabilityPayloadMap,
    power_management: PciHostOptionalCapabilityPayloadMap,
    pcie: PciHostOptionalCapabilityPayloadMap,
    msi: PciHostOptionalCapabilityPayloadMap,
    msix: PciHostOptionalCapabilityPayloadMap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostCheckpointRecord {
    component: CheckpointComponentId,
    topology: PciHostBridgeTopologySnapshot,
    bridge_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
    endpoint_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
    bridge_bar_payloads: PciHostBarPayloadMap,
    endpoint_bar_payloads: PciHostBarPayloadMap,
    endpoint_raw_capability_payloads: PciHostRawCapabilityPayloadMap,
    endpoint_power_management_payloads: PciHostOptionalCapabilityPayloadMap,
    endpoint_pcie_payloads: PciHostOptionalCapabilityPayloadMap,
    endpoint_msi_payloads: PciHostOptionalCapabilityPayloadMap,
    endpoint_msix_payloads: PciHostOptionalCapabilityPayloadMap,
    config_space_payloads_present: bool,
    bar_payloads_present: bool,
    capability_payloads_present: bool,
}

impl PciHostCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        topology: PciHostBridgeTopologySnapshot,
        bridge_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
        endpoint_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
        bridge_bar_payloads: PciHostBarPayloadMap,
        endpoint_bar_payloads: PciHostBarPayloadMap,
    ) -> Self {
        Self {
            component,
            topology,
            bridge_config_space_payloads,
            endpoint_config_space_payloads,
            bridge_bar_payloads,
            endpoint_bar_payloads,
            endpoint_raw_capability_payloads: BTreeMap::new(),
            endpoint_power_management_payloads: BTreeMap::new(),
            endpoint_pcie_payloads: BTreeMap::new(),
            endpoint_msi_payloads: BTreeMap::new(),
            endpoint_msix_payloads: BTreeMap::new(),
            config_space_payloads_present: true,
            bar_payloads_present: true,
            capability_payloads_present: false,
        }
    }

    pub fn with_endpoint_capability_payloads(
        mut self,
        endpoint_raw_capability_payloads: PciHostRawCapabilityPayloadMap,
        endpoint_power_management_payloads: PciHostOptionalCapabilityPayloadMap,
        endpoint_pcie_payloads: PciHostOptionalCapabilityPayloadMap,
        endpoint_msi_payloads: PciHostOptionalCapabilityPayloadMap,
        endpoint_msix_payloads: PciHostOptionalCapabilityPayloadMap,
    ) -> Self {
        self.endpoint_raw_capability_payloads = endpoint_raw_capability_payloads;
        self.endpoint_power_management_payloads = endpoint_power_management_payloads;
        self.endpoint_pcie_payloads = endpoint_pcie_payloads;
        self.endpoint_msi_payloads = endpoint_msi_payloads;
        self.endpoint_msix_payloads = endpoint_msix_payloads;
        self.capability_payloads_present = true;
        self
    }

    fn config_only(
        component: CheckpointComponentId,
        topology: PciHostBridgeTopologySnapshot,
        bridge_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
        endpoint_config_space_payloads: BTreeMap<PciFunctionAddress, Vec<u8>>,
    ) -> Self {
        Self {
            component,
            topology,
            bridge_config_space_payloads,
            endpoint_config_space_payloads,
            bridge_bar_payloads: BTreeMap::new(),
            endpoint_bar_payloads: BTreeMap::new(),
            endpoint_raw_capability_payloads: BTreeMap::new(),
            endpoint_power_management_payloads: BTreeMap::new(),
            endpoint_pcie_payloads: BTreeMap::new(),
            endpoint_msi_payloads: BTreeMap::new(),
            endpoint_msix_payloads: BTreeMap::new(),
            config_space_payloads_present: true,
            bar_payloads_present: false,
            capability_payloads_present: false,
        }
    }

    fn topology_only(
        component: CheckpointComponentId,
        topology: PciHostBridgeTopologySnapshot,
    ) -> Self {
        Self {
            component,
            topology,
            bridge_config_space_payloads: BTreeMap::new(),
            endpoint_config_space_payloads: BTreeMap::new(),
            bridge_bar_payloads: BTreeMap::new(),
            endpoint_bar_payloads: BTreeMap::new(),
            endpoint_raw_capability_payloads: BTreeMap::new(),
            endpoint_power_management_payloads: BTreeMap::new(),
            endpoint_pcie_payloads: BTreeMap::new(),
            endpoint_msi_payloads: BTreeMap::new(),
            endpoint_msix_payloads: BTreeMap::new(),
            config_space_payloads_present: false,
            bar_payloads_present: false,
            capability_payloads_present: false,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn topology(&self) -> &PciHostBridgeTopologySnapshot {
        &self.topology
    }

    pub fn bridge_config_space_payloads(&self) -> &BTreeMap<PciFunctionAddress, Vec<u8>> {
        &self.bridge_config_space_payloads
    }

    pub fn endpoint_config_space_payloads(&self) -> &BTreeMap<PciFunctionAddress, Vec<u8>> {
        &self.endpoint_config_space_payloads
    }

    pub fn bridge_bar_payloads(&self) -> &PciHostBarPayloadMap {
        &self.bridge_bar_payloads
    }

    pub fn endpoint_bar_payloads(&self) -> &PciHostBarPayloadMap {
        &self.endpoint_bar_payloads
    }

    pub fn endpoint_raw_capability_payloads(&self) -> &PciHostRawCapabilityPayloadMap {
        &self.endpoint_raw_capability_payloads
    }

    pub fn endpoint_power_management_payloads(&self) -> &PciHostOptionalCapabilityPayloadMap {
        &self.endpoint_power_management_payloads
    }

    pub fn endpoint_pcie_payloads(&self) -> &PciHostOptionalCapabilityPayloadMap {
        &self.endpoint_pcie_payloads
    }

    pub fn endpoint_msi_payloads(&self) -> &PciHostOptionalCapabilityPayloadMap {
        &self.endpoint_msi_payloads
    }

    pub fn endpoint_msix_payloads(&self) -> &PciHostOptionalCapabilityPayloadMap {
        &self.endpoint_msix_payloads
    }

    pub const fn has_config_space_payloads(&self) -> bool {
        self.config_space_payloads_present
    }

    pub const fn has_bar_payloads(&self) -> bool {
        self.bar_payloads_present
    }

    pub const fn has_capability_payloads(&self) -> bool {
        self.capability_payloads_present
    }
}

#[derive(Clone, Debug)]
pub struct PciHostCheckpointPort {
    component: CheckpointComponentId,
    host: Arc<Mutex<PciHostBridge>>,
}

impl PciHostCheckpointPort {
    pub fn new(component: CheckpointComponentId, host: Arc<Mutex<PciHostBridge>>) -> Self {
        Self { component, host }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn host(&self) -> Arc<Mutex<PciHostBridge>> {
        Arc::clone(&self.host)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let snapshot = self
            .host
            .lock()
            .expect("PCI host checkpoint lock")
            .snapshot();
        let topology = snapshot.topology_snapshot();
        let bridge_config_space_payloads = snapshot.bridge_config_space_payloads();
        let endpoint_config_space_payloads = snapshot.endpoint_config_space_payloads();
        let bridge_bar_payloads = snapshot.bridge_bar_payloads();
        let endpoint_bar_payloads = snapshot.endpoint_bar_payloads();
        let endpoint_raw_capability_payloads = snapshot.endpoint_raw_capability_payloads();
        let endpoint_power_management_payloads = snapshot.endpoint_power_management_payloads();
        let endpoint_pcie_payloads = snapshot.endpoint_pcie_payloads();
        let endpoint_msi_payloads = snapshot.endpoint_msi_payloads();
        let endpoint_msix_payloads = snapshot.endpoint_msix_payloads();
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_TOPOLOGY_CHUNK,
                topology.to_bytes(),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_BRIDGE_CONFIG_CHUNK,
                encode_config_space_payloads(&bridge_config_space_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_BRIDGE_BAR_CHUNK,
                encode_bar_payloads(&bridge_bar_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_CONFIG_CHUNK,
                encode_config_space_payloads(&endpoint_config_space_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_BAR_CHUNK,
                encode_bar_payloads(&endpoint_bar_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_RAW_CAPABILITY_CHUNK,
                encode_raw_capability_payloads(&endpoint_raw_capability_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_POWER_MANAGEMENT_CHUNK,
                encode_optional_capability_payloads(
                    PCI_HOST_POWER_MANAGEMENT_MAP_MAGIC,
                    &endpoint_power_management_payloads,
                ),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_PCIE_CHUNK,
                encode_optional_capability_payloads(
                    PCI_HOST_PCIE_MAP_MAGIC,
                    &endpoint_pcie_payloads,
                ),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_MSI_CHUNK,
                encode_optional_capability_payloads(PCI_HOST_MSI_MAP_MAGIC, &endpoint_msi_payloads),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_ENDPOINT_MSIX_CHUNK,
                encode_optional_capability_payloads(
                    PCI_HOST_MSIX_MAP_MAGIC,
                    &endpoint_msix_payloads,
                ),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        Ok(PciHostCheckpointRecord::new(
            self.component.clone(),
            topology,
            bridge_config_space_payloads,
            endpoint_config_space_payloads,
            bridge_bar_payloads,
            endpoint_bar_payloads,
        )
        .with_endpoint_capability_payloads(
            endpoint_raw_capability_payloads,
            endpoint_power_management_payloads,
            endpoint_pcie_payloads,
            endpoint_msi_payloads,
            endpoint_msix_payloads,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_record(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let payload = registry
            .chunk(&self.component, PCI_HOST_TOPOLOGY_CHUNK)
            .ok_or_else(|| PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_HOST_TOPOLOGY_CHUNK.to_string(),
            })?;
        let topology = PciHostBridgeTopologySnapshot::from_bytes(payload).map_err(|error| {
            PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        let bridge_config_space_payloads =
            self.decode_optional_config_space_payloads(registry, PCI_HOST_BRIDGE_CONFIG_CHUNK)?;
        let endpoint_config_space_payloads =
            self.decode_optional_config_space_payloads(registry, PCI_HOST_ENDPOINT_CONFIG_CHUNK)?;
        let (bridge_config_space_payloads, endpoint_config_space_payloads, has_config_space) =
            match (bridge_config_space_payloads, endpoint_config_space_payloads) {
                (Some(bridge_config_space_payloads), Some(endpoint_config_space_payloads)) => (
                    bridge_config_space_payloads,
                    endpoint_config_space_payloads,
                    true,
                ),
                (None, None) => (BTreeMap::new(), BTreeMap::new(), false),
                (None, Some(_)) => {
                    return Err(PciHostCheckpointError::MissingChunk {
                        component: self.component.clone(),
                        name: PCI_HOST_BRIDGE_CONFIG_CHUNK.to_string(),
                    });
                }
                (Some(_), None) => {
                    return Err(PciHostCheckpointError::MissingChunk {
                        component: self.component.clone(),
                        name: PCI_HOST_ENDPOINT_CONFIG_CHUNK.to_string(),
                    });
                }
            };
        let bridge_bar_payloads =
            self.decode_optional_bar_payloads(registry, PCI_HOST_BRIDGE_BAR_CHUNK)?;
        let endpoint_bar_payloads =
            self.decode_optional_bar_payloads(registry, PCI_HOST_ENDPOINT_BAR_CHUNK)?;
        let (bridge_bar_payloads, endpoint_bar_payloads, has_bars) =
            match (bridge_bar_payloads, endpoint_bar_payloads) {
                (Some(bridge_bar_payloads), Some(endpoint_bar_payloads)) => {
                    (bridge_bar_payloads, endpoint_bar_payloads, true)
                }
                (None, None) => (BTreeMap::new(), BTreeMap::new(), false),
                (None, Some(_)) => {
                    return Err(PciHostCheckpointError::MissingChunk {
                        component: self.component.clone(),
                        name: PCI_HOST_BRIDGE_BAR_CHUNK.to_string(),
                    });
                }
                (Some(_), None) => {
                    return Err(PciHostCheckpointError::MissingChunk {
                        component: self.component.clone(),
                        name: PCI_HOST_ENDPOINT_BAR_CHUNK.to_string(),
                    });
                }
            };
        let endpoint_capability_payloads =
            self.decode_optional_endpoint_capability_payloads(registry)?;
        let has_capabilities = endpoint_capability_payloads.is_some();
        let endpoint_capability_payloads = endpoint_capability_payloads.unwrap_or_default();
        if has_capabilities && !has_config_space {
            return Err(PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_HOST_BRIDGE_CONFIG_CHUNK.to_string(),
            });
        }
        if has_capabilities && !has_bars {
            return Err(PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_HOST_BRIDGE_BAR_CHUNK.to_string(),
            });
        }
        match (has_config_space, has_bars) {
            (true, true) if has_capabilities => Ok(PciHostCheckpointRecord::new(
                self.component.clone(),
                topology,
                bridge_config_space_payloads,
                endpoint_config_space_payloads,
                bridge_bar_payloads,
                endpoint_bar_payloads,
            )
            .with_endpoint_capability_payloads(
                endpoint_capability_payloads.raw_capabilities,
                endpoint_capability_payloads.power_management,
                endpoint_capability_payloads.pcie,
                endpoint_capability_payloads.msi,
                endpoint_capability_payloads.msix,
            )),
            (true, true) => Ok(PciHostCheckpointRecord::new(
                self.component.clone(),
                topology,
                bridge_config_space_payloads,
                endpoint_config_space_payloads,
                bridge_bar_payloads,
                endpoint_bar_payloads,
            )),
            (true, false) => Ok(PciHostCheckpointRecord::config_only(
                self.component.clone(),
                topology,
                bridge_config_space_payloads,
                endpoint_config_space_payloads,
            )),
            (false, false) => Ok(PciHostCheckpointRecord::topology_only(
                self.component.clone(),
                topology,
            )),
            (false, true) => Err(PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_HOST_BRIDGE_CONFIG_CHUNK.to_string(),
            }),
        }
    }

    fn decode_optional_config_space_payloads(
        &self,
        registry: &CheckpointRegistry,
        name: &str,
    ) -> Result<Option<BTreeMap<PciFunctionAddress, Vec<u8>>>, PciHostCheckpointError> {
        let Some(payload) = registry.chunk(&self.component, name) else {
            return Ok(None);
        };
        decode_config_space_payloads(payload)
            .map_err(|error| PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            })
            .map(Some)
    }

    fn decode_optional_bar_payloads(
        &self,
        registry: &CheckpointRegistry,
        name: &str,
    ) -> Result<Option<PciHostBarPayloadMap>, PciHostCheckpointError> {
        let Some(payload) = registry.chunk(&self.component, name) else {
            return Ok(None);
        };
        decode_bar_payloads(payload)
            .map_err(|error| PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            })
            .map(Some)
    }

    fn decode_optional_endpoint_capability_payloads(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Option<PciHostEndpointCapabilityPayloads>, PciHostCheckpointError> {
        let raw_capabilities = self.decode_optional_raw_capability_payloads(
            registry,
            PCI_HOST_ENDPOINT_RAW_CAPABILITY_CHUNK,
        )?;
        let power_management = self.decode_optional_capability_payloads(
            registry,
            PCI_HOST_ENDPOINT_POWER_MANAGEMENT_CHUNK,
            PCI_HOST_POWER_MANAGEMENT_MAP_MAGIC,
            PciError::InvalidPowerManagementCapabilitySnapshot,
        )?;
        let pcie = self.decode_optional_capability_payloads(
            registry,
            PCI_HOST_ENDPOINT_PCIE_CHUNK,
            PCI_HOST_PCIE_MAP_MAGIC,
            PciError::InvalidPciExpressCapabilitySnapshot,
        )?;
        let msi = self.decode_optional_capability_payloads(
            registry,
            PCI_HOST_ENDPOINT_MSI_CHUNK,
            PCI_HOST_MSI_MAP_MAGIC,
            PciError::InvalidMsiCapabilitySnapshot,
        )?;
        let msix = self.decode_optional_capability_payloads(
            registry,
            PCI_HOST_ENDPOINT_MSIX_CHUNK,
            PCI_HOST_MSIX_MAP_MAGIC,
            PciError::InvalidMsixCapabilitySnapshot,
        )?;
        let present = [
            (
                raw_capabilities.is_some(),
                PCI_HOST_ENDPOINT_RAW_CAPABILITY_CHUNK,
            ),
            (
                power_management.is_some(),
                PCI_HOST_ENDPOINT_POWER_MANAGEMENT_CHUNK,
            ),
            (pcie.is_some(), PCI_HOST_ENDPOINT_PCIE_CHUNK),
            (msi.is_some(), PCI_HOST_ENDPOINT_MSI_CHUNK),
            (msix.is_some(), PCI_HOST_ENDPOINT_MSIX_CHUNK),
        ];
        if present.iter().all(|(is_present, _)| !*is_present) {
            return Ok(None);
        }
        if let Some((_, name)) = present.iter().find(|(is_present, _)| !*is_present) {
            return Err(PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: (*name).to_string(),
            });
        }
        Ok(Some(PciHostEndpointCapabilityPayloads {
            raw_capabilities: raw_capabilities.expect("validated raw capability chunk"),
            power_management: power_management.expect("validated PM capability chunk"),
            pcie: pcie.expect("validated PCIe capability chunk"),
            msi: msi.expect("validated MSI capability chunk"),
            msix: msix.expect("validated MSI-X capability chunk"),
        }))
    }

    fn decode_optional_raw_capability_payloads(
        &self,
        registry: &CheckpointRegistry,
        name: &str,
    ) -> Result<Option<PciHostRawCapabilityPayloadMap>, PciHostCheckpointError> {
        let Some(payload) = registry.chunk(&self.component, name) else {
            return Ok(None);
        };
        decode_raw_capability_payloads(payload)
            .map_err(|error| PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            })
            .map(Some)
    }

    fn decode_optional_capability_payloads(
        &self,
        registry: &CheckpointRegistry,
        name: &str,
        magic: &[u8; 8],
        invalid: PciError,
    ) -> Result<Option<PciHostOptionalCapabilityPayloadMap>, PciHostCheckpointError> {
        let Some(payload) = registry.chunk(&self.component, name) else {
            return Ok(None);
        };
        decode_optional_capability_payloads(payload, magic, invalid)
            .map_err(|error| PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            })
            .map(Some)
    }

    fn validate_record(
        &self,
        record: &PciHostCheckpointRecord,
    ) -> Result<(), PciHostCheckpointError> {
        let snapshot = self
            .host
            .lock()
            .expect("PCI host checkpoint lock")
            .snapshot();
        if snapshot.topology_snapshot() != *record.topology() {
            return Err(PciHostCheckpointError::Pci {
                component: self.component.clone(),
                error: PciError::SnapshotHostBridgeMismatch,
            });
        }
        if record.has_config_space_payloads() {
            snapshot
                .validate_bridge_config_space_payloads(record.bridge_config_space_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_config_space_payloads(record.endpoint_config_space_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
        }
        if record.has_bar_payloads() {
            snapshot
                .validate_bridge_bar_payloads(record.bridge_bar_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_bar_payloads(record.endpoint_bar_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
        }
        if record.has_capability_payloads() {
            snapshot
                .validate_endpoint_raw_capability_payloads(
                    record.endpoint_raw_capability_payloads(),
                )
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_power_management_payloads(
                    record.endpoint_power_management_payloads(),
                )
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_pcie_payloads(record.endpoint_pcie_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_msi_payloads(record.endpoint_msi_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
            snapshot
                .validate_endpoint_msix_payloads(record.endpoint_msix_payloads())
                .map_err(|error| PciHostCheckpointError::Pci {
                    component: self.component.clone(),
                    error,
                })?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PciHostCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, PciHostCheckpointPort>,
}

impl PciHostCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = PciHostCheckpointPort>,
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
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn decode_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self.decode_all_from(registry)?;
        for record in &records {
            let port = self
                .ports
                .get(record.component())
                .expect("decoded PCI host checkpoint record has registered port");
            port.validate_record(record)?;
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), PciHostCheckpointError> {
        for record in self.decode_all_from(registry)? {
            let port = self
                .ports
                .get(record.component())
                .expect("decoded PCI host checkpoint record has registered port");
            port.validate_record(&record)?;
        }
        Ok(())
    }
}

fn encode_config_space_payloads(payloads: &BTreeMap<PciFunctionAddress, Vec<u8>>) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(PCI_HOST_CONFIG_SPACE_MAP_MAGIC);
    payload.extend_from_slice(&PCI_HOST_CONFIG_SPACE_MAP_VERSION.to_le_bytes());
    payload.extend_from_slice(&(payloads.len() as u32).to_le_bytes());
    for (function, config_space) in payloads {
        payload.push(function.bus());
        payload.push(function.device());
        payload.push(function.function());
        payload.extend_from_slice(&(config_space.len() as u32).to_le_bytes());
        payload.extend_from_slice(config_space);
    }
    payload
}

fn encode_bar_payloads(payloads: &PciHostBarPayloadMap) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(PCI_HOST_BAR_MAP_MAGIC);
    payload.extend_from_slice(&PCI_HOST_BAR_MAP_VERSION.to_le_bytes());
    payload.extend_from_slice(&(payloads.len() as u32).to_le_bytes());
    for (function, bars) in payloads {
        payload.push(function.bus());
        payload.push(function.device());
        payload.push(function.function());
        payload.extend_from_slice(&(bars.len() as u32).to_le_bytes());
        for bar in bars {
            match bar {
                Some(bar) => {
                    payload.push(PCI_BAR_SLOT_PRESENT);
                    payload.extend_from_slice(&(bar.len() as u32).to_le_bytes());
                    payload.extend_from_slice(bar);
                }
                None => payload.push(PCI_BAR_SLOT_ABSENT),
            }
        }
    }
    payload
}

fn encode_raw_capability_payloads(payloads: &PciHostRawCapabilityPayloadMap) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(PCI_HOST_RAW_CAPABILITY_MAP_MAGIC);
    payload.extend_from_slice(&PCI_HOST_CAPABILITY_MAP_VERSION.to_le_bytes());
    payload.extend_from_slice(&(payloads.len() as u32).to_le_bytes());
    for (function, capabilities) in payloads {
        payload.push(function.bus());
        payload.push(function.device());
        payload.push(function.function());
        payload.extend_from_slice(&(capabilities.len() as u32).to_le_bytes());
        for capability in capabilities {
            payload.extend_from_slice(&(capability.len() as u32).to_le_bytes());
            payload.extend_from_slice(capability);
        }
    }
    payload
}

fn encode_optional_capability_payloads(
    magic: &[u8; 8],
    payloads: &PciHostOptionalCapabilityPayloadMap,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(magic);
    payload.extend_from_slice(&PCI_HOST_CAPABILITY_MAP_VERSION.to_le_bytes());
    payload.extend_from_slice(&(payloads.len() as u32).to_le_bytes());
    for (function, capability) in payloads {
        payload.push(function.bus());
        payload.push(function.device());
        payload.push(function.function());
        match capability {
            Some(capability) => {
                payload.push(PCI_CAPABILITY_PAYLOAD_PRESENT);
                payload.extend_from_slice(&(capability.len() as u32).to_le_bytes());
                payload.extend_from_slice(capability);
            }
            None => payload.push(PCI_CAPABILITY_PAYLOAD_ABSENT),
        }
    }
    payload
}

fn decode_config_space_payloads(
    payload: &[u8],
) -> Result<BTreeMap<PciFunctionAddress, Vec<u8>>, PciError> {
    let mut cursor = 0;
    if read_exact(payload, &mut cursor, PCI_HOST_CONFIG_SPACE_MAP_MAGIC.len())?
        != PCI_HOST_CONFIG_SPACE_MAP_MAGIC
    {
        return Err(PciError::InvalidConfigSpaceSnapshot);
    }
    if read_u16(payload, &mut cursor)? != PCI_HOST_CONFIG_SPACE_MAP_VERSION {
        return Err(PciError::InvalidConfigSpaceSnapshot);
    }
    let count = read_u32(payload, &mut cursor)? as usize;
    let mut payloads = BTreeMap::new();
    for _ in 0..count {
        let function = PciFunctionAddress::new(
            read_u8(payload, &mut cursor)?,
            read_u8(payload, &mut cursor)?,
            read_u8(payload, &mut cursor)?,
        )?;
        let config_space_len = read_u32(payload, &mut cursor)? as usize;
        let config_space = read_exact(payload, &mut cursor, config_space_len)?.to_vec();
        if payloads.insert(function, config_space).is_some() {
            return Err(PciError::InvalidConfigSpaceSnapshot);
        }
    }
    if cursor != payload.len() {
        return Err(PciError::InvalidConfigSpaceSnapshot);
    }
    Ok(payloads)
}

fn decode_raw_capability_payloads(
    payload: &[u8],
) -> Result<PciHostRawCapabilityPayloadMap, PciError> {
    let invalid = PciError::InvalidRawCapabilitySnapshot;
    let mut cursor = 0;
    if read_capability_exact(
        payload,
        &mut cursor,
        PCI_HOST_RAW_CAPABILITY_MAP_MAGIC.len(),
        &invalid,
    )? != PCI_HOST_RAW_CAPABILITY_MAP_MAGIC
    {
        return Err(invalid);
    }
    if read_capability_u16(payload, &mut cursor, &invalid)? != PCI_HOST_CAPABILITY_MAP_VERSION {
        return Err(invalid);
    }
    let count = read_capability_u32(payload, &mut cursor, &invalid)? as usize;
    let mut payloads = BTreeMap::new();
    for _ in 0..count {
        let function = PciFunctionAddress::new(
            read_capability_u8(payload, &mut cursor, &invalid)?,
            read_capability_u8(payload, &mut cursor, &invalid)?,
            read_capability_u8(payload, &mut cursor, &invalid)?,
        )
        .map_err(|_| invalid.clone())?;
        let capability_count = read_capability_u32(payload, &mut cursor, &invalid)? as usize;
        let mut capabilities = Vec::new();
        for _ in 0..capability_count {
            let capability_len = read_capability_u32(payload, &mut cursor, &invalid)? as usize;
            let capability =
                read_capability_exact(payload, &mut cursor, capability_len, &invalid)?.to_vec();
            capabilities.push(capability);
        }
        if payloads.insert(function, capabilities).is_some() {
            return Err(invalid);
        }
    }
    if cursor != payload.len() {
        return Err(invalid);
    }
    Ok(payloads)
}

fn decode_optional_capability_payloads(
    payload: &[u8],
    magic: &[u8; 8],
    invalid: PciError,
) -> Result<PciHostOptionalCapabilityPayloadMap, PciError> {
    let mut cursor = 0;
    if read_capability_exact(payload, &mut cursor, magic.len(), &invalid)? != magic {
        return Err(invalid);
    }
    if read_capability_u16(payload, &mut cursor, &invalid)? != PCI_HOST_CAPABILITY_MAP_VERSION {
        return Err(invalid);
    }
    let count = read_capability_u32(payload, &mut cursor, &invalid)? as usize;
    let mut payloads = BTreeMap::new();
    for _ in 0..count {
        let function = PciFunctionAddress::new(
            read_capability_u8(payload, &mut cursor, &invalid)?,
            read_capability_u8(payload, &mut cursor, &invalid)?,
            read_capability_u8(payload, &mut cursor, &invalid)?,
        )
        .map_err(|_| invalid.clone())?;
        let capability = match read_capability_u8(payload, &mut cursor, &invalid)? {
            PCI_CAPABILITY_PAYLOAD_ABSENT => None,
            PCI_CAPABILITY_PAYLOAD_PRESENT => {
                let capability_len = read_capability_u32(payload, &mut cursor, &invalid)? as usize;
                Some(
                    read_capability_exact(payload, &mut cursor, capability_len, &invalid)?.to_vec(),
                )
            }
            _ => return Err(invalid),
        };
        if payloads.insert(function, capability).is_some() {
            return Err(invalid);
        }
    }
    if cursor != payload.len() {
        return Err(invalid);
    }
    Ok(payloads)
}

fn decode_bar_payloads(payload: &[u8]) -> Result<PciHostBarPayloadMap, PciError> {
    let mut cursor = 0;
    if read_bar_exact(payload, &mut cursor, PCI_HOST_BAR_MAP_MAGIC.len())? != PCI_HOST_BAR_MAP_MAGIC
    {
        return Err(PciError::InvalidBarSnapshot);
    }
    if read_bar_u16(payload, &mut cursor)? != PCI_HOST_BAR_MAP_VERSION {
        return Err(PciError::InvalidBarSnapshot);
    }
    let count = read_bar_u32(payload, &mut cursor)? as usize;
    let mut payloads = BTreeMap::new();
    for _ in 0..count {
        let function = PciFunctionAddress::new(
            read_bar_u8(payload, &mut cursor)?,
            read_bar_u8(payload, &mut cursor)?,
            read_bar_u8(payload, &mut cursor)?,
        )?;
        let slot_count = read_bar_u32(payload, &mut cursor)? as usize;
        if slot_count > payload.len().saturating_sub(cursor) {
            return Err(PciError::InvalidBarSnapshot);
        }
        let mut bars = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            match read_bar_u8(payload, &mut cursor)? {
                PCI_BAR_SLOT_ABSENT => bars.push(None),
                PCI_BAR_SLOT_PRESENT => {
                    let bar_len = read_bar_u32(payload, &mut cursor)? as usize;
                    let bar = read_bar_exact(payload, &mut cursor, bar_len)?.to_vec();
                    bars.push(Some(bar));
                }
                _ => return Err(PciError::InvalidBarSnapshot),
            }
        }
        if payloads.insert(function, bars).is_some() {
            return Err(PciError::InvalidBarSnapshot);
        }
    }
    if cursor != payload.len() {
        return Err(PciError::InvalidBarSnapshot);
    }
    Ok(payloads)
}

fn read_u8(payload: &[u8], cursor: &mut usize) -> Result<u8, PciError> {
    let byte = *payload
        .get(*cursor)
        .ok_or(PciError::InvalidConfigSpaceSnapshot)?;
    *cursor += 1;
    Ok(byte)
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Result<u16, PciError> {
    let bytes = read_exact(payload, cursor, U16_BYTES)?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_u32(payload: &[u8], cursor: &mut usize) -> Result<u32, PciError> {
    let bytes = read_exact(payload, cursor, U32_BYTES)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_exact<'a>(
    payload: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], PciError> {
    let end = cursor
        .checked_add(length)
        .ok_or(PciError::InvalidConfigSpaceSnapshot)?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(PciError::InvalidConfigSpaceSnapshot)?;
    *cursor = end;
    Ok(bytes)
}

fn read_capability_u8(
    payload: &[u8],
    cursor: &mut usize,
    invalid: &PciError,
) -> Result<u8, PciError> {
    let byte = *payload.get(*cursor).ok_or_else(|| invalid.clone())?;
    *cursor += 1;
    Ok(byte)
}

fn read_capability_u16(
    payload: &[u8],
    cursor: &mut usize,
    invalid: &PciError,
) -> Result<u16, PciError> {
    let bytes = read_capability_exact(payload, cursor, U16_BYTES, invalid)?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_capability_u32(
    payload: &[u8],
    cursor: &mut usize,
    invalid: &PciError,
) -> Result<u32, PciError> {
    let bytes = read_capability_exact(payload, cursor, U32_BYTES, invalid)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_capability_exact<'a>(
    payload: &'a [u8],
    cursor: &mut usize,
    length: usize,
    invalid: &PciError,
) -> Result<&'a [u8], PciError> {
    let end = cursor.checked_add(length).ok_or_else(|| invalid.clone())?;
    let bytes = payload.get(*cursor..end).ok_or_else(|| invalid.clone())?;
    *cursor = end;
    Ok(bytes)
}

fn read_bar_u8(payload: &[u8], cursor: &mut usize) -> Result<u8, PciError> {
    let byte = *payload.get(*cursor).ok_or(PciError::InvalidBarSnapshot)?;
    *cursor += 1;
    Ok(byte)
}

fn read_bar_u16(payload: &[u8], cursor: &mut usize) -> Result<u16, PciError> {
    let bytes = read_bar_exact(payload, cursor, U16_BYTES)?;
    Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_bar_u32(payload: &[u8], cursor: &mut usize) -> Result<u32, PciError> {
    let bytes = read_bar_exact(payload, cursor, U32_BYTES)?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_bar_exact<'a>(
    payload: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], PciError> {
    let end = cursor
        .checked_add(length)
        .ok_or(PciError::InvalidBarSnapshot)?;
    let bytes = payload
        .get(*cursor..end)
        .ok_or(PciError::InvalidBarSnapshot)?;
    *cursor = end;
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PciHostCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Pci {
        component: CheckpointComponentId,
        error: PciError,
    },
}

impl PciHostCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Pci { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for PciHostCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "PCI host checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "PCI host checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Pci { component, error } => write!(
                formatter,
                "PCI host checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for PciHostCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Pci { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}
