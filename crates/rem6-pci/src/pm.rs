use rem6_memory::AccessSize;

use crate::{write_u16_at, PciConfigOffset, PciError, PCI_CONFIG_SPACE_SIZE};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_PM_CAPABILITY_ID: u8 = 0x01;
const PCI_PM_CAPABILITY_SIZE: u64 = 0x06;
const PCI_PM_CAPABILITIES_OFFSET: u16 = 0x02;
const PCI_PM_CONTROL_STATUS_OFFSET: u16 = 0x04;
const PCI_PM_SNAPSHOT_MAGIC: &[u8; 8] = b"R6PCPM01";
const PCI_PM_SNAPSHOT_VERSION: u16 = 1;
const U16_BYTES: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciPowerManagementCapabilitySpec {
    offset: PciConfigOffset,
    capabilities: u16,
    initial_control_status: u16,
}

impl PciPowerManagementCapabilitySpec {
    pub fn new(
        offset: PciConfigOffset,
        capabilities: u16,
        initial_control_status: u16,
    ) -> Result<Self, PciError> {
        let size = AccessSize::new(PCI_PM_CAPABILITY_SIZE).map_err(PciError::Memory)?;
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidPowerManagementCapabilityOffset { offset, size });
        }

        Ok(Self {
            offset,
            capabilities,
            initial_control_status,
        })
    }

    pub const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    pub const fn capabilities(self) -> u16 {
        self.capabilities
    }

    pub const fn initial_control_status(self) -> u16 {
        self.initial_control_status
    }

    pub const fn size(self) -> u64 {
        PCI_PM_CAPABILITY_SIZE
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciPowerManagementCapabilityState {
    spec: PciPowerManagementCapabilitySpec,
    control_status: u16,
}

impl PciPowerManagementCapabilityState {
    pub(crate) const fn new(spec: PciPowerManagementCapabilitySpec) -> Self {
        Self {
            spec,
            control_status: spec.initial_control_status(),
        }
    }

    pub(crate) const fn spec(&self) -> PciPowerManagementCapabilitySpec {
        self.spec
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(PCI_PM_SNAPSHOT_MAGIC);
        write_u16(&mut payload, PCI_PM_SNAPSHOT_VERSION);
        write_u16(&mut payload, self.spec.offset().get());
        write_u16(&mut payload, self.spec.capabilities());
        write_u16(&mut payload, self.spec.initial_control_status());
        write_u16(&mut payload, self.control_status);
        payload
    }

    pub(crate) fn from_bytes(payload: &[u8]) -> Result<Self, PciError> {
        decode_state(payload).ok_or(PciError::InvalidPowerManagementCapabilitySnapshot)
    }

    pub(crate) fn install_into(&self, config: &mut [u8; PCI_CONFIG_SPACE_SIZE]) {
        let base = self.spec.offset().as_usize();
        config[base] = PCI_PM_CAPABILITY_ID;
        config[base + 1] = 0;
        write_u16_at(
            config,
            base + PCI_PM_CAPABILITIES_OFFSET as usize,
            self.spec.capabilities(),
        );
        write_u16_at(
            config,
            base + PCI_PM_CONTROL_STATUS_OFFSET as usize,
            self.control_status,
        );
    }

    pub(crate) fn contains(&self, offset: PciConfigOffset, size: AccessSize) -> bool {
        let start = offset.get() as u64;
        let end = start + size.bytes();
        let cap_start = self.spec.offset().get() as u64;
        let cap_end = cap_start + PCI_PM_CAPABILITY_SIZE;
        start >= cap_start && end <= cap_end
    }

    pub(crate) fn write_config(
        &mut self,
        offset: PciConfigOffset,
        data: &[u8],
        config: &mut [u8; PCI_CONFIG_SPACE_SIZE],
    ) -> Result<(), PciError> {
        let size = AccessSize::new(data.len() as u64).map_err(PciError::Memory)?;
        let relative = offset.get() - self.spec.offset().get();
        match (relative, data.len()) {
            (PCI_PM_CONTROL_STATUS_OFFSET, 2) => {
                self.control_status = u16::from_le_bytes(data.try_into().unwrap());
                write_u16_at(config, offset.as_usize(), self.control_status);
                Ok(())
            }
            (PCI_PM_CONTROL_STATUS_OFFSET, _) => {
                Err(PciError::UnalignedPowerManagementCapabilityWrite { offset, size })
            }
            _ => Err(PciError::ReadOnlyPowerManagementCapabilityWrite { offset, size }),
        }
    }
}

fn decode_state(payload: &[u8]) -> Option<PciPowerManagementCapabilityState> {
    let mut cursor = 0;
    let magic = read_exact(payload, &mut cursor, PCI_PM_SNAPSHOT_MAGIC.len())?;
    if magic != PCI_PM_SNAPSHOT_MAGIC {
        return None;
    }
    if read_u16(payload, &mut cursor)? != PCI_PM_SNAPSHOT_VERSION {
        return None;
    }
    let offset = PciConfigOffset::new(read_u16(payload, &mut cursor)?).ok()?;
    let capabilities = read_u16(payload, &mut cursor)?;
    let initial_control_status = read_u16(payload, &mut cursor)?;
    let control_status = read_u16(payload, &mut cursor)?;
    if cursor != payload.len() {
        return None;
    }
    let spec =
        PciPowerManagementCapabilitySpec::new(offset, capabilities, initial_control_status).ok()?;
    Some(PciPowerManagementCapabilityState {
        spec,
        control_status,
    })
}

fn write_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_u16(payload: &[u8], cursor: &mut usize) -> Option<u16> {
    let bytes = read_exact(payload, cursor, U16_BYTES)?;
    Some(u16::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_exact<'a>(payload: &'a [u8], cursor: &mut usize, length: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(length)?;
    let bytes = payload.get(*cursor..end)?;
    *cursor = end;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pm_spec(offset: u16) -> PciPowerManagementCapabilitySpec {
        PciPowerManagementCapabilitySpec::new(PciConfigOffset::new(offset).unwrap(), 0x0003, 0x0001)
            .unwrap()
    }

    #[test]
    fn power_management_capability_state_codec_preserves_pmcsr() {
        let spec = pm_spec(0x44);
        let mut state = PciPowerManagementCapabilityState::new(spec);
        let mut config = [0; PCI_CONFIG_SPACE_SIZE];
        state.install_into(&mut config);
        state
            .write_config(
                PciConfigOffset::new(0x48).unwrap(),
                &0x8023_u16.to_le_bytes(),
                &mut config,
            )
            .unwrap();

        let decoded = PciPowerManagementCapabilityState::from_bytes(&state.to_bytes()).unwrap();
        let mut decoded_config = [0; PCI_CONFIG_SPACE_SIZE];
        decoded.install_into(&mut decoded_config);

        assert_eq!(decoded, state);
        assert_eq!(&decoded_config[0x44..0x4a], &config[0x44..0x4a]);
    }

    #[test]
    fn power_management_capability_state_codec_rejects_invalid_payloads() {
        let state = PciPowerManagementCapabilityState::new(pm_spec(0x44));
        let mut payload = state.to_bytes();

        assert_eq!(
            PciPowerManagementCapabilityState::from_bytes(&payload[..payload.len() - 1]),
            Err(PciError::InvalidPowerManagementCapabilitySnapshot)
        );

        payload.push(0);
        assert_eq!(
            PciPowerManagementCapabilityState::from_bytes(&payload),
            Err(PciError::InvalidPowerManagementCapabilitySnapshot)
        );

        let mut invalid_offset = state.to_bytes();
        invalid_offset[10] = 0x20;
        invalid_offset[11] = 0x00;
        assert_eq!(
            PciPowerManagementCapabilityState::from_bytes(&invalid_offset),
            Err(PciError::InvalidPowerManagementCapabilitySnapshot)
        );
    }
}
