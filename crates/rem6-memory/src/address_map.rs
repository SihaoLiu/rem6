use crate::request::MemoryRequest;
use crate::{AccessSize, Address, AddressRange, MemoryError, MemoryTargetId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AddressInterleave {
    granularity: AccessSize,
    stripes: u32,
    match_index: u32,
}

impl AddressInterleave {
    pub fn modulo(
        granularity: AccessSize,
        stripes: u32,
        match_index: u32,
    ) -> Result<Self, MemoryError> {
        if stripes == 0 {
            return Err(MemoryError::ZeroInterleaveStripes);
        }
        if match_index >= stripes {
            return Err(MemoryError::InterleaveMatchOutOfRange {
                stripes,
                match_index,
            });
        }

        Ok(Self {
            granularity,
            stripes,
            match_index,
        })
    }

    pub const fn granularity(self) -> AccessSize {
        self.granularity
    }

    pub const fn stripes(self) -> u32 {
        self.stripes
    }

    pub const fn match_index(self) -> u32 {
        self.match_index
    }

    fn contains_offset(self, offset: u64) -> bool {
        let stripe = (offset / self.granularity.bytes()) % u64::from(self.stripes);
        stripe == u64::from(self.match_index)
    }

    fn local_offset(self, offset: u64) -> Option<u64> {
        if !self.contains_offset(offset) {
            return None;
        }

        let granularity = self.granularity.bytes();
        let chunk = offset / granularity;
        let within_chunk = offset % granularity;
        Some((chunk / u64::from(self.stripes)) * granularity + within_chunk)
    }

    fn disjoint_from(self, other: Self) -> bool {
        self.granularity == other.granularity
            && self.stripes == other.stripes
            && self.match_index != other.match_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressMapRegion {
    base: AddressRange,
    holes: Vec<AddressRange>,
    interleave: Option<AddressInterleave>,
}

impl AddressMapRegion {
    pub fn new(base: AddressRange) -> Self {
        Self {
            base,
            holes: Vec::new(),
            interleave: None,
        }
    }

    pub fn with_holes(mut self, holes: Vec<AddressRange>) -> Result<Self, MemoryError> {
        self.holes = normalize_sparse_holes(self.base, holes)?;
        Ok(self)
    }

    pub fn with_interleave(mut self, interleave: AddressInterleave) -> Result<Self, MemoryError> {
        self.interleave = Some(interleave);
        Ok(self)
    }

    pub const fn base_range(&self) -> AddressRange {
        self.base
    }

    pub const fn start(&self) -> Address {
        self.base.start()
    }

    pub const fn size(&self) -> AccessSize {
        self.base.size()
    }

    pub const fn end(&self) -> Address {
        self.base.end()
    }

    pub fn holes(&self) -> &[AddressRange] {
        &self.holes
    }

    pub const fn interleave(&self) -> Option<AddressInterleave> {
        self.interleave
    }

    pub const fn is_interleaved(&self) -> bool {
        self.interleave.is_some()
    }

    pub fn is_sparse(&self) -> bool {
        !self.holes.is_empty()
    }

    pub fn contains(&self, address: Address) -> bool {
        self.offset(address).is_some()
    }

    pub fn contains_range(&self, range: AddressRange) -> bool {
        let Some(start_offset) = self.offset(range.start()) else {
            return false;
        };
        let last_address = Address::new(range.end().get() - 1);
        let Some(last_offset) = self.offset(last_address) else {
            return false;
        };

        last_offset
            .checked_sub(start_offset)
            .and_then(|delta| delta.checked_add(1))
            == Some(range.size().bytes())
    }

    pub fn offset(&self, address: Address) -> Option<u64> {
        if !self.base.contains(address) || self.holes.iter().any(|hole| hole.contains(address)) {
            return None;
        }

        let packed = self.sparse_offset(address);
        self.interleave
            .map_or(Some(packed), |interleave| interleave.local_offset(packed))
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        if !self.base.overlaps(other.base) {
            return false;
        }
        if let (Some(left), Some(right)) = (self.interleave, other.interleave) {
            if left.disjoint_from(right) {
                return false;
            }
        }

        true
    }

    fn sparse_offset(&self, address: Address) -> u64 {
        let mut offset = address.get() - self.base.start().get();
        for hole in &self.holes {
            if address.get() >= hole.end().get() {
                offset -= hole.size().bytes();
            } else {
                break;
            }
        }
        offset
    }
}

fn normalize_sparse_holes(
    base: AddressRange,
    mut holes: Vec<AddressRange>,
) -> Result<Vec<AddressRange>, MemoryError> {
    holes.sort_by_key(|hole| (hole.start(), hole.end()));
    let mut normalized: Vec<AddressRange> = Vec::with_capacity(holes.len());
    for hole in holes {
        if !base.contains_range(hole) {
            return Err(MemoryError::SparseHoleOutsideRange { base, hole });
        }
        if let Some(existing) = normalized.last().copied() {
            if existing.overlaps(hole) {
                return Err(MemoryError::OverlappingSparseHole {
                    existing,
                    requested: hole,
                });
            }
        }
        normalized.push(hole);
    }
    Ok(normalized)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddressDecode {
    target: MemoryTargetId,
    region: AddressMapRegion,
    offset: u64,
}

impl AddressDecode {
    fn new(target: MemoryTargetId, region: AddressMapRegion, offset: u64) -> Self {
        Self {
            target,
            region,
            offset,
        }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn region(&self) -> &AddressMapRegion {
        &self.region
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AddressDecoder {
    regions: Vec<(MemoryTargetId, AddressMapRegion)>,
}

impl AddressDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<(), MemoryError> {
        let requested = AddressRange::new(start, size)?;
        self.insert_region(target, AddressMapRegion::new(requested))
    }

    pub fn insert_region(
        &mut self,
        target: MemoryTargetId,
        requested: AddressMapRegion,
    ) -> Result<(), MemoryError> {
        if let Some((_, existing)) = self
            .regions
            .iter()
            .find(|(_, existing)| existing.overlaps(&requested))
        {
            return Err(MemoryError::OverlappingAddressRegion {
                existing: existing.base_range(),
                requested: requested.base_range(),
            });
        }

        self.regions.push((target, requested));
        self.regions.sort_by_key(|(target, range)| {
            (
                range.start(),
                range.end(),
                range
                    .interleave()
                    .map_or(u32::MAX, |interleave| interleave.match_index()),
                target.get(),
            )
        });
        Ok(())
    }

    pub fn decode(&self, address: Address) -> Result<MemoryTargetId, MemoryError> {
        self.decode_detail(address).map(|decode| decode.target())
    }

    pub fn decode_detail(&self, address: Address) -> Result<AddressDecode, MemoryError> {
        self.regions
            .iter()
            .find_map(|(target, region)| {
                region
                    .offset(address)
                    .map(|offset| AddressDecode::new(*target, region.clone(), offset))
            })
            .ok_or(MemoryError::UnmappedAddress { address })
    }

    pub fn decode_request(&self, request: &MemoryRequest) -> Result<MemoryTargetId, MemoryError> {
        self.decode_request_detail(request)
            .map(|decode| decode.target())
    }

    pub fn decode_request_detail(
        &self,
        request: &MemoryRequest,
    ) -> Result<AddressDecode, MemoryError> {
        let range = request.range();
        let Some((target, region)) = self
            .regions
            .iter()
            .find(|(_, region)| region.contains(range.start()))
        else {
            return Err(MemoryError::UnmappedAddress {
                address: range.start(),
            });
        };

        if !region.contains_range(range) {
            return Err(MemoryError::RequestCrossesAddressRegion {
                request: request.id(),
                range,
            });
        }

        let offset = region.offset(range.start()).expect("checked region start");
        Ok(AddressDecode::new(*target, region.clone(), offset))
    }

    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    pub fn regions(&self) -> &[(MemoryTargetId, AddressMapRegion)] {
        &self.regions
    }
}
