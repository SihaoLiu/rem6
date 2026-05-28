use rem6_memory::AccessSize;

use crate::{
    PciConfigOffset, PciEndpointConfig, PciError, PCI_CAPABILITY_PTR_OFFSET, PCI_CONFIG_SPACE_SIZE,
    PCI_STATUS_CAPABILITY_LIST, PCI_STATUS_OFFSET,
};

const PCI_CAPABILITY_MIN_OFFSET: u16 = 0x40;
const PCI_RAW_CAPABILITY_SNAPSHOT_MAGIC: &[u8; 8] = b"R6PCRAW1";
const PCI_RAW_CAPABILITY_SNAPSHOT_VERSION: u16 = 1;
const U16_BYTES: usize = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciRawCapabilitySpec {
    offset: PciConfigOffset,
    bytes: Vec<u8>,
    size: AccessSize,
}

impl PciRawCapabilitySpec {
    pub fn new(offset: PciConfigOffset, bytes: impl Into<Vec<u8>>) -> Result<Self, PciError> {
        let mut bytes = bytes.into();
        let size = AccessSize::new(bytes.len() as u64).map_err(PciError::Memory)?;
        if bytes.len() < 2 {
            return Err(PciError::InvalidRawCapabilitySize { offset, size });
        }
        let raw_offset = offset.get();
        let end = u64::from(raw_offset) + size.bytes();
        if raw_offset < PCI_CAPABILITY_MIN_OFFSET
            || !raw_offset.is_multiple_of(4)
            || end > PCI_CONFIG_SPACE_SIZE as u64
        {
            return Err(PciError::InvalidRawCapabilityOffset { offset, size });
        }
        bytes[1] = 0;
        Ok(Self {
            offset,
            bytes,
            size,
        })
    }

    pub const fn offset(&self) -> PciConfigOffset {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PciRawCapabilityState {
    spec: PciRawCapabilitySpec,
}

impl PciRawCapabilityState {
    pub(crate) const fn new(spec: PciRawCapabilitySpec) -> Self {
        Self { spec }
    }

    pub(crate) const fn spec(&self) -> &PciRawCapabilitySpec {
        &self.spec
    }

    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(PCI_RAW_CAPABILITY_SNAPSHOT_MAGIC);
        write_u16(&mut payload, PCI_RAW_CAPABILITY_SNAPSHOT_VERSION);
        write_u16(&mut payload, self.spec.offset().get());
        write_u16(&mut payload, self.spec.bytes().len() as u16);
        payload.extend_from_slice(self.spec.bytes());
        payload
    }

    pub(crate) fn from_bytes(payload: &[u8]) -> Result<Self, PciError> {
        decode_raw_capability_state(payload).ok_or(PciError::InvalidRawCapabilitySnapshot)
    }

    pub(crate) fn install_into(&self, config: &mut [u8]) {
        let start = self.spec.offset().as_usize();
        let end = start + self.spec.bytes().len();
        config[start..end].copy_from_slice(self.spec.bytes());
    }
}

fn decode_raw_capability_state(payload: &[u8]) -> Option<PciRawCapabilityState> {
    let mut cursor = 0;
    let magic = read_exact(
        payload,
        &mut cursor,
        PCI_RAW_CAPABILITY_SNAPSHOT_MAGIC.len(),
    )?;
    if magic != PCI_RAW_CAPABILITY_SNAPSHOT_MAGIC {
        return None;
    }
    if read_u16(payload, &mut cursor)? != PCI_RAW_CAPABILITY_SNAPSHOT_VERSION {
        return None;
    }
    let offset = PciConfigOffset::new(read_u16(payload, &mut cursor)?).ok()?;
    let size = read_u16(payload, &mut cursor)? as usize;
    let bytes = read_exact(payload, &mut cursor, size)?.to_vec();
    if cursor != payload.len() || bytes.get(1).copied() != Some(0) {
        return None;
    }
    let spec = PciRawCapabilitySpec::new(offset, bytes).ok()?;
    Some(PciRawCapabilityState { spec })
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

impl PciEndpointConfig {
    pub fn install_raw_capability(&mut self, spec: PciRawCapabilitySpec) -> Result<(), PciError> {
        self.register_capability_region(spec.offset(), spec.size().bytes())?;
        let state = PciRawCapabilityState::new(spec);
        state.install_into(&mut self.config);
        self.raw_capabilities.push(state);
        self.rebuild_capability_list();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_capability(offset: u16) -> PciRawCapabilitySpec {
        PciRawCapabilitySpec::new(
            PciConfigOffset::new(offset).unwrap(),
            [0x09, 0xff, 0x10, 0x08, 0xaa, 0xbb, 0xcc, 0xdd],
        )
        .unwrap()
    }

    #[test]
    fn raw_capability_state_codec_preserves_canonical_vendor_bytes() {
        let state = PciRawCapabilityState::new(raw_capability(0x60));
        let decoded = PciRawCapabilityState::from_bytes(&state.to_bytes()).unwrap();

        assert_eq!(decoded, state);
        assert_eq!(decoded.spec().offset(), PciConfigOffset::new(0x60).unwrap());
        assert_eq!(
            decoded.spec().bytes(),
            &[0x09, 0x00, 0x10, 0x08, 0xaa, 0xbb, 0xcc, 0xdd]
        );
    }

    #[test]
    fn raw_capability_state_codec_rejects_invalid_payloads() {
        let state = PciRawCapabilityState::new(raw_capability(0x60));
        let mut payload = state.to_bytes();

        assert_eq!(
            PciRawCapabilityState::from_bytes(&payload[..payload.len() - 1]),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        payload.push(0);
        assert_eq!(
            PciRawCapabilityState::from_bytes(&payload),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        let mut invalid_magic = state.to_bytes();
        invalid_magic[0] = 0;
        assert_eq!(
            PciRawCapabilityState::from_bytes(&invalid_magic),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        let mut invalid_version = state.to_bytes();
        invalid_version[8] = 0xff;
        assert_eq!(
            PciRawCapabilityState::from_bytes(&invalid_version),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        let mut invalid_offset = state.to_bytes();
        invalid_offset[10] = 0x3c;
        assert_eq!(
            PciRawCapabilityState::from_bytes(&invalid_offset),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        let mut invalid_size = state.to_bytes();
        invalid_size[12] = 1;
        invalid_size.truncate(15);
        assert_eq!(
            PciRawCapabilityState::from_bytes(&invalid_size),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );

        let mut invalid_next_pointer = state.to_bytes();
        invalid_next_pointer[15] = 0x80;
        assert_eq!(
            PciRawCapabilityState::from_bytes(&invalid_next_pointer),
            Err(PciError::InvalidRawCapabilitySnapshot)
        );
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PciEndpointCapabilityList {
    regions: Vec<PciCapabilityRegion>,
}

impl PciEndpointCapabilityList {
    pub(crate) fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub(crate) fn register(&mut self, offset: PciConfigOffset, size: u64) -> Result<(), PciError> {
        let requested_size = AccessSize::new(size).map_err(PciError::Memory)?;
        let requested = PciCapabilityRegion {
            offset,
            size: requested_size,
        };
        if let Some(existing) = self
            .regions
            .iter()
            .find(|existing| existing.overlaps(requested))
        {
            return Err(PciError::OverlappingCapability {
                existing_offset: existing.offset(),
                existing_size: existing.size(),
                requested_offset: requested.offset(),
                requested_size: requested.size(),
            });
        }
        self.regions.push(requested);
        Ok(())
    }

    pub(crate) fn rebuild(&self, config: &mut [u8]) {
        if self.regions.is_empty() {
            config[PCI_CAPABILITY_PTR_OFFSET] = 0;
            config[PCI_STATUS_OFFSET] &= !PCI_STATUS_CAPABILITY_LIST;
            return;
        }

        config[PCI_STATUS_OFFSET] |= PCI_STATUS_CAPABILITY_LIST;
        config[PCI_CAPABILITY_PTR_OFFSET] = self.regions[0].offset().get() as u8;
        for index in 0..self.regions.len() {
            let offset = self.regions[index].offset().as_usize();
            let next = self
                .regions
                .get(index + 1)
                .map_or(0, |capability| capability.offset().get() as u8);
            config[offset + 1] = next;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PciCapabilityRegion {
    offset: PciConfigOffset,
    size: AccessSize,
}

impl PciCapabilityRegion {
    const fn offset(self) -> PciConfigOffset {
        self.offset
    }

    const fn size(self) -> AccessSize {
        self.size
    }

    fn end(self) -> u64 {
        u64::from(self.offset.get()) + self.size.bytes()
    }

    fn overlaps(self, other: Self) -> bool {
        u64::from(self.offset.get()) < other.end() && u64::from(other.offset.get()) < self.end()
    }
}
