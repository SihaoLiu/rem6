use super::{
    Address, MemoryRequestId, RiscvIssuedScalarMemoryHandoff, RiscvO3LiveDataHandoffError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffPartialOverlay {
    pub(super) load_data_request: MemoryRequestId,
    pub(super) source_data_request: MemoryRequestId,
    pub(super) source_address: Address,
    pub(super) source_bytes: u32,
    pub(super) address: Address,
    pub(super) bytes: u32,
    pub(super) forwarded_mask: u8,
    pub(super) data: [u8; 8],
    pub(super) source_data: [u8; 8],
}

impl RiscvO3LiveDataHandoffPartialOverlay {
    pub const fn load_data_request(self) -> MemoryRequestId {
        self.load_data_request
    }

    pub const fn source_data_request(self) -> MemoryRequestId {
        self.source_data_request
    }

    pub const fn source_address(self) -> Address {
        self.source_address
    }

    pub const fn source_bytes(self) -> u32 {
        self.source_bytes
    }

    pub fn source_data(&self) -> &[u8] {
        &self.source_data[..self.source_bytes as usize]
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn forwarded_mask(self) -> u8 {
        self.forwarded_mask
    }

    pub const fn response_owned_mask(self) -> u8 {
        scalar_byte_mask(self.bytes) & !self.forwarded_mask
    }

    pub const fn forwarded_bytes(self) -> u32 {
        self.forwarded_mask.count_ones()
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.bytes as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingPartialScalarLoadHandoff {
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) forwarded_mask: u8,
    pub(crate) data: [u8; 8],
}

pub(super) fn partial_overlay_matches_source(
    source: RiscvIssuedScalarMemoryHandoff,
    overlay: RiscvPendingPartialScalarLoadHandoff,
) -> bool {
    let Some(source_data) = source.store_data else {
        return false;
    };
    if partial_overlay_mask(source.address, source.bytes, overlay.address, overlay.bytes)
        != overlay.forwarded_mask
    {
        return false;
    }
    for load_index in 0..overlay.bytes as usize {
        if overlay.forwarded_mask & (1 << load_index) == 0 {
            if overlay.data[load_index] != 0 {
                return false;
            }
            continue;
        }
        let address = overlay.address.get() + load_index as u64;
        let source_index = (address - source.address.get()) as usize;
        if overlay.data[load_index] != source_data[source_index] {
            return false;
        }
    }
    overlay.data[overlay.bytes as usize..]
        .iter()
        .all(|byte| *byte == 0)
}

pub(super) const fn scalar_byte_mask(bytes: u32) -> u8 {
    if bytes >= 8 {
        u8::MAX
    } else {
        ((1_u16 << bytes) - 1) as u8
    }
}

pub(super) fn partial_overlay_mask(
    source_address: Address,
    source_bytes: u32,
    load_address: Address,
    load_bytes: u32,
) -> u8 {
    let source_start = u128::from(source_address.get());
    let source_end = source_start + u128::from(source_bytes);
    let load_start = u128::from(load_address.get());
    let mut mask = 0_u8;
    for load_index in 0..load_bytes {
        let address = load_start + u128::from(load_index);
        if source_start <= address && address < source_end {
            mask |= 1 << load_index;
        }
    }
    mask
}

pub(super) fn canonical_partial_overlay_source_data(
    source_address: Address,
    load_address: Address,
    load_bytes: u32,
    forwarded_mask: u8,
    data: &[u8; 8],
) -> [u8; 8] {
    let mut source_data = [0; 8];
    for (load_index, value) in data.iter().copied().enumerate().take(load_bytes as usize) {
        if forwarded_mask & (1 << load_index) == 0 {
            continue;
        }
        let address = load_address.get() + load_index as u64;
        let source_index = (address - source_address.get()) as usize;
        source_data[source_index] = value;
    }
    source_data
}

pub(super) fn validate_partial_overlay_data(
    source_address: Address,
    source_bytes: u32,
    load_address: Address,
    load_bytes: u32,
    forwarded_mask: u8,
    data: &[u8; 8],
    source_data: &[u8; 8],
) -> Result<(), RiscvO3LiveDataHandoffError> {
    if let Some((index, value)) = source_data
        .iter()
        .copied()
        .enumerate()
        .skip(source_bytes as usize)
        .find(|(_, value)| *value != 0)
    {
        return Err(
            RiscvO3LiveDataHandoffError::NonZeroPartialOverlaySourceDataPadding { index, value },
        );
    }
    for (index, value) in data.iter().copied().enumerate() {
        let forwarded = index < load_bytes as usize && forwarded_mask & (1 << index) != 0;
        if !forwarded {
            if value != 0 {
                return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayData { index });
            }
            continue;
        }
        let address = load_address.get() + index as u64;
        let source_index = (address - source_address.get()) as usize;
        if value != source_data[source_index] {
            return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayData { index });
        }
    }
    let canonical_source_data = canonical_partial_overlay_source_data(
        source_address,
        load_address,
        load_bytes,
        forwarded_mask,
        data,
    );
    if let Some(index) = source_data[..source_bytes as usize]
        .iter()
        .zip(&canonical_source_data[..source_bytes as usize])
        .position(|(actual, expected)| actual != expected)
    {
        return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlaySourceData { index });
    }
    Ok(())
}
