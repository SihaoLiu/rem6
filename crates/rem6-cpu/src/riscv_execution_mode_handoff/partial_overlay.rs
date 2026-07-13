use super::{
    Address, MemoryRequestId, RiscvIssuedScalarMemoryHandoff, RiscvO3LiveDataHandoffEntry,
    RiscvO3LiveDataHandoffError, RiscvO3LiveDataHandoffOperation, RiscvO3LiveDataHandoffOwnership,
    RiscvO3LiveDataHandoffTarget, Tick, MAX_ROWS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffPartialOverlaySource {
    pub(super) source_data_request: MemoryRequestId,
    pub(super) source_address: Address,
    pub(super) source_bytes: u32,
    pub(super) ownership_mask: u8,
    pub(super) source_data: [u8; 8],
}

impl RiscvO3LiveDataHandoffPartialOverlaySource {
    pub const fn source_data_request(self) -> MemoryRequestId {
        self.source_data_request
    }

    pub const fn source_address(self) -> Address {
        self.source_address
    }

    pub const fn source_bytes(self) -> u32 {
        self.source_bytes
    }

    pub const fn ownership_mask(self) -> u8 {
        self.ownership_mask
    }

    pub fn source_data(&self) -> &[u8] {
        &self.source_data[..self.source_bytes as usize]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffPartialOverlay {
    pub(super) load_data_request: MemoryRequestId,
    pub(super) address: Address,
    pub(super) bytes: u32,
    pub(super) forwarded_mask: u8,
    pub(super) data: [u8; 8],
    pub(super) sources: Vec<RiscvO3LiveDataHandoffPartialOverlaySource>,
}

impl RiscvO3LiveDataHandoffPartialOverlay {
    pub const fn load_data_request(&self) -> MemoryRequestId {
        self.load_data_request
    }

    pub fn sources(&self) -> &[RiscvO3LiveDataHandoffPartialOverlaySource] {
        &self.sources
    }

    pub fn source_data_request(&self) -> MemoryRequestId {
        self.first_source().source_data_request()
    }

    pub fn source_address(&self) -> Address {
        self.first_source().source_address()
    }

    pub fn source_bytes(&self) -> u32 {
        self.first_source().source_bytes()
    }

    pub fn source_data(&self) -> &[u8] {
        self.first_source().source_data()
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn bytes(&self) -> u32 {
        self.bytes
    }

    pub const fn forwarded_mask(&self) -> u8 {
        self.forwarded_mask
    }

    pub const fn response_owned_mask(&self) -> u8 {
        scalar_byte_mask(self.bytes) & !self.forwarded_mask
    }

    pub const fn forwarded_bytes(&self) -> u32 {
        self.forwarded_mask.count_ones()
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.bytes as usize]
    }

    fn first_source(&self) -> &RiscvO3LiveDataHandoffPartialOverlaySource {
        self.sources
            .first()
            .expect("validated partial overlay has a source")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffCompletedPartialOverlay {
    pub(super) fetch_request: MemoryRequestId,
    pub(super) load_data_request: MemoryRequestId,
    pub(super) issue_tick: Tick,
    pub(super) response_tick: Tick,
    pub(super) address: Address,
    pub(super) bytes: u32,
    pub(super) original_forwarded_mask: u8,
    pub(super) live_forwarded_mask: u8,
    pub(super) data: [u8; 8],
    pub(super) o3_sequence: u64,
    pub(super) trace_sequence: Option<u64>,
    pub(super) sources: Vec<RiscvO3LiveDataHandoffPartialOverlaySource>,
}

impl RiscvO3LiveDataHandoffCompletedPartialOverlay {
    pub const fn fetch_request(&self) -> MemoryRequestId {
        self.fetch_request
    }

    pub const fn load_data_request(&self) -> MemoryRequestId {
        self.load_data_request
    }

    pub const fn issue_tick(&self) -> Tick {
        self.issue_tick
    }

    pub const fn response_tick(&self) -> Tick {
        self.response_tick
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn bytes(&self) -> u32 {
        self.bytes
    }

    pub const fn original_forwarded_mask(&self) -> u8 {
        self.original_forwarded_mask
    }

    pub const fn original_response_mask(&self) -> u8 {
        scalar_byte_mask(self.bytes) & !self.original_forwarded_mask
    }

    pub const fn live_forwarded_mask(&self) -> u8 {
        self.live_forwarded_mask
    }

    pub const fn retired_forwarded_mask(&self) -> u8 {
        self.original_forwarded_mask & !self.live_forwarded_mask
    }

    pub const fn original_forwarded_bytes(&self) -> u32 {
        self.original_forwarded_mask.count_ones()
    }

    pub const fn live_forwarded_bytes(&self) -> u32 {
        self.live_forwarded_mask.count_ones()
    }

    pub const fn retired_forwarded_bytes(&self) -> u32 {
        self.retired_forwarded_mask().count_ones()
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.bytes as usize]
    }

    pub const fn o3_sequence(&self) -> u64 {
        self.o3_sequence
    }

    pub const fn trace_sequence(&self) -> Option<u64> {
        self.trace_sequence
    }

    pub fn sources(&self) -> &[RiscvO3LiveDataHandoffPartialOverlaySource] {
        &self.sources
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvPendingPartialScalarLoadHandoff {
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) forwarded_mask: u8,
    pub(crate) data: [u8; 8],
}

pub(super) fn compose_partial_overlay_sources(
    sources: &[RiscvIssuedScalarMemoryHandoff],
    overlay: RiscvPendingPartialScalarLoadHandoff,
) -> Option<Vec<RiscvO3LiveDataHandoffPartialOverlaySource>> {
    if sources.is_empty()
        || overlay.forwarded_mask == 0
        || overlay.forwarded_mask == scalar_byte_mask(overlay.bytes)
        || overlay.forwarded_mask & !scalar_byte_mask(overlay.bytes) != 0
        || overlay
            .data
            .iter()
            .copied()
            .enumerate()
            .any(|(index, value)| {
                (index >= overlay.bytes as usize || overlay.forwarded_mask & (1 << index) == 0)
                    && value != 0
            })
    {
        return None;
    }

    let physical_masks = sources
        .iter()
        .map(|source| {
            source.store_data.map(|_| {
                partial_overlay_mask(source.address, source.bytes, overlay.address, overlay.bytes)
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if physical_masks.contains(&0)
        || physical_masks.iter().fold(0_u8, |union, mask| union | mask) != overlay.forwarded_mask
    {
        return None;
    }

    let mut ownership_masks = vec![0_u8; sources.len()];
    for load_index in 0..overlay.bytes as usize {
        let bit = 1_u8 << load_index;
        if overlay.forwarded_mask & bit == 0 {
            continue;
        }
        let source_index = physical_masks.iter().rposition(|mask| mask & bit != 0)?;
        let source = sources[source_index];
        let source_data = source.store_data?;
        let address = overlay.address.get() + load_index as u64;
        let source_byte = (address - source.address.get()) as usize;
        if overlay.data[load_index] != source_data[source_byte] {
            return None;
        }
        ownership_masks[source_index] |= bit;
    }

    Some(
        sources
            .iter()
            .copied()
            .zip(ownership_masks)
            .map(
                |(source, ownership_mask)| RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request: source.data_request,
                    source_address: source.address,
                    source_bytes: source.bytes,
                    ownership_mask,
                    source_data: canonical_partial_overlay_source_data(
                        source.address,
                        overlay.address,
                        overlay.bytes,
                        ownership_mask,
                        &overlay.data,
                    ),
                },
            )
            .collect(),
    )
}

pub(super) fn compose_completed_partial_overlay_sources(
    sources: &[RiscvIssuedScalarMemoryHandoff],
    address: Address,
    bytes: u32,
    original_forwarded_mask: u8,
    final_data: &[u8; 8],
) -> Option<(u8, Vec<RiscvO3LiveDataHandoffPartialOverlaySource>)> {
    if !matches!(bytes, 1 | 2 | 4 | 8) {
        return None;
    }
    let scalar_mask = scalar_byte_mask(bytes);
    if original_forwarded_mask == 0
        || original_forwarded_mask == scalar_mask
        || original_forwarded_mask & !scalar_mask != 0
        || final_data[bytes as usize..].iter().any(|value| *value != 0)
    {
        return None;
    }

    let live_forwarded_mask = sources.iter().try_fold(0_u8, |mask, source| {
        let source_mask = source
            .store_data
            .map(|_| partial_overlay_mask(source.address, source.bytes, address, bytes))?;
        (source_mask != 0 && source_mask & !original_forwarded_mask == 0)
            .then_some(mask | source_mask)
    })?;
    if live_forwarded_mask == 0 || live_forwarded_mask == scalar_mask {
        return None;
    }

    let mut live_data = [0; 8];
    for index in 0..bytes as usize {
        if live_forwarded_mask & (1 << index) != 0 {
            live_data[index] = final_data[index];
        }
    }
    let sources = compose_partial_overlay_sources(
        sources,
        RiscvPendingPartialScalarLoadHandoff {
            address,
            bytes,
            forwarded_mask: live_forwarded_mask,
            data: live_data,
        },
    )?;
    sources
        .iter()
        .all(|source| source.ownership_mask != 0)
        .then_some((live_forwarded_mask, sources))
}

pub(super) fn completed_partial_overlay_is_valid(
    entries: &[RiscvO3LiveDataHandoffEntry],
    overlay: &RiscvO3LiveDataHandoffCompletedPartialOverlay,
) -> bool {
    if entries.is_empty() || entries.len() >= MAX_ROWS || !matches!(overlay.bytes, 1 | 2 | 4 | 8) {
        return false;
    }
    let scalar_mask = scalar_byte_mask(overlay.bytes);
    if overlay.response_tick < overlay.issue_tick
        || overlay.original_forwarded_mask == 0
        || overlay.original_forwarded_mask == scalar_mask
        || overlay.original_forwarded_mask & !scalar_mask != 0
        || overlay.live_forwarded_mask == 0
        || overlay.live_forwarded_mask == scalar_mask
        || overlay.live_forwarded_mask & !overlay.original_forwarded_mask != 0
        || overlay.data[overlay.bytes as usize..]
            .iter()
            .any(|value| *value != 0)
        || overlay.sources.len() != entries.len()
        || entries.windows(2).any(|pair| {
            pair[0].o3_sequence >= pair[1].o3_sequence
                || pair[1].ownership
                    != RiscvO3LiveDataHandoffOwnership::BufferedStore {
                        predecessor: pair[0].data_request,
                    }
        })
        || entries.iter().any(|entry| {
            entry.operation != RiscvO3LiveDataHandoffOperation::Store
                || !matches!(entry.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                || entry.partition != entries[0].partition
                || entry.target != entries[0].target
                || entry.o3_sequence >= overlay.o3_sequence
        })
        || entries[0].ownership != RiscvO3LiveDataHandoffOwnership::Transport
    {
        return false;
    }

    let physical_masks = overlay
        .sources
        .iter()
        .map(|source| {
            partial_overlay_mask(
                source.source_address,
                source.source_bytes,
                overlay.address,
                overlay.bytes,
            )
        })
        .collect::<Vec<_>>();
    if physical_masks.iter().any(|physical_mask| {
        *physical_mask == 0 || *physical_mask & !overlay.original_forwarded_mask != 0
    }) || physical_masks.iter().fold(0_u8, |union, mask| union | mask)
        != overlay.live_forwarded_mask
    {
        return false;
    }

    let mut expected_ownership_masks = vec![0_u8; overlay.sources.len()];
    for load_index in 0..overlay.bytes as usize {
        let bit = 1_u8 << load_index;
        if overlay.live_forwarded_mask & bit == 0 {
            continue;
        }
        let Some(source_index) = physical_masks.iter().rposition(|mask| mask & bit != 0) else {
            return false;
        };
        expected_ownership_masks[source_index] |= bit;
    }

    let mut owned = 0_u8;
    for (((entry, source), physical_mask), expected_ownership_mask) in entries
        .iter()
        .zip(&overlay.sources)
        .zip(physical_masks)
        .zip(expected_ownership_masks)
    {
        if source.ownership_mask != expected_ownership_mask
            || source.ownership_mask & !physical_mask != 0
            || entry.data_request != source.source_data_request
            || entry.address != source.source_address
            || entry.bytes != source.source_bytes
            || source.ownership_mask == 0
            || source.ownership_mask & !overlay.live_forwarded_mask != 0
            || owned & source.ownership_mask != 0
            || validate_partial_overlay_data(
                source.source_address,
                source.source_bytes,
                overlay.address,
                overlay.bytes,
                source.ownership_mask,
                &overlay.data,
                &source.source_data,
            )
            .is_err()
        {
            return false;
        }
        owned |= source.ownership_mask;
    }
    owned == overlay.live_forwarded_mask
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
    ownership_mask: u8,
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
    let canonical_source_data = canonical_partial_overlay_source_data(
        source_address,
        load_address,
        load_bytes,
        ownership_mask,
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

pub(super) fn validate_partial_overlay_payload(
    bytes: u32,
    forwarded_mask: u8,
    data: &[u8; 8],
) -> Result<(), RiscvO3LiveDataHandoffError> {
    for (index, value) in data.iter().copied().enumerate() {
        let forwarded = index < bytes as usize && forwarded_mask & (1 << index) != 0;
        if !forwarded && value != 0 {
            return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayData { index });
        }
    }
    Ok(())
}
