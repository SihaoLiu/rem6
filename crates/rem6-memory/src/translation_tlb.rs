use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AccessSize, Address, AddressRange, TranslationError, TranslationFault, TranslationFaultKind,
    TranslationPageMap, TranslationPageMapping, TranslationPagePermissions, TranslationPageSize,
    TranslationRequest, TranslationResolution, TranslationSegmentedResolution,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TranslationAddressSpaceId(u16);

impl TranslationAddressSpaceId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn global() -> Self {
        Self(0)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TranslationTlbConfig {
    capacity: usize,
}

impl TranslationTlbConfig {
    pub fn new(capacity: usize) -> Result<Self, TranslationError> {
        if capacity == 0 {
            return Err(TranslationError::ZeroTlbCapacity);
        }

        Ok(Self { capacity })
    }

    pub const fn capacity(self) -> usize {
        self.capacity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationTlbLookupKind {
    Hit,
    Miss,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TranslationTlbEntryScope {
    Global,
    NonGlobal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TranslationTlbStats {
    hits: u64,
    misses: u64,
    faults: u64,
    inserts: u64,
    evictions: u64,
}

impl TranslationTlbStats {
    pub const fn new(hits: u64, misses: u64, faults: u64, inserts: u64, evictions: u64) -> Self {
        Self {
            hits,
            misses,
            faults,
            inserts,
            evictions,
        }
    }

    pub const fn hits(self) -> u64 {
        self.hits
    }

    pub const fn misses(self) -> u64 {
        self.misses
    }

    pub const fn faults(self) -> u64 {
        self.faults
    }

    pub const fn inserts(self) -> u64 {
        self.inserts
    }

    pub const fn evictions(self) -> u64 {
        self.evictions
    }

    fn record_hit(&mut self) {
        self.hits = self.hits.saturating_add(1);
    }

    fn record_miss(&mut self) {
        self.misses = self.misses.saturating_add(1);
    }

    fn record_fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }

    fn record_insert(&mut self) {
        self.inserts = self.inserts.saturating_add(1);
    }

    fn record_eviction(&mut self) {
        self.evictions = self.evictions.saturating_add(1);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationTlbEntrySnapshot {
    address_space: TranslationAddressSpaceId,
    virtual_page: Address,
    physical_page: Address,
    page_size: TranslationPageSize,
    permissions: TranslationPagePermissions,
    scope: TranslationTlbEntryScope,
    last_used: u64,
}

impl TranslationTlbEntrySnapshot {
    pub fn new(
        virtual_page: Address,
        physical_page: Address,
        page_size: TranslationPageSize,
        permissions: TranslationPagePermissions,
        last_used: u64,
    ) -> Self {
        Self::new_in_address_space(
            TranslationAddressSpaceId::global(),
            virtual_page,
            physical_page,
            page_size,
            permissions,
            last_used,
        )
    }

    pub fn new_in_address_space(
        address_space: TranslationAddressSpaceId,
        virtual_page: Address,
        physical_page: Address,
        page_size: TranslationPageSize,
        permissions: TranslationPagePermissions,
        last_used: u64,
    ) -> Self {
        Self {
            address_space,
            virtual_page,
            physical_page,
            page_size,
            permissions,
            scope: TranslationTlbEntryScope::NonGlobal,
            last_used,
        }
    }

    pub const fn with_scope(mut self, scope: TranslationTlbEntryScope) -> Self {
        self.scope = scope;
        self
    }

    pub const fn address_space(&self) -> TranslationAddressSpaceId {
        self.address_space
    }

    pub const fn virtual_page(&self) -> Address {
        self.virtual_page
    }

    pub const fn physical_page(&self) -> Address {
        self.physical_page
    }

    pub const fn page_size(&self) -> TranslationPageSize {
        self.page_size
    }

    pub const fn permissions(&self) -> TranslationPagePermissions {
        self.permissions
    }

    pub const fn scope(&self) -> TranslationTlbEntryScope {
        self.scope
    }

    pub const fn last_used(&self) -> u64 {
        self.last_used
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationTlbSnapshot {
    config: TranslationTlbConfig,
    entries: Vec<TranslationTlbEntrySnapshot>,
    next_lru: u64,
    stats: TranslationTlbStats,
}

impl TranslationTlbSnapshot {
    pub fn new(
        config: TranslationTlbConfig,
        entries: Vec<TranslationTlbEntrySnapshot>,
        next_lru: u64,
        stats: TranslationTlbStats,
    ) -> Self {
        Self {
            config,
            entries,
            next_lru,
            stats,
        }
    }

    pub const fn config(&self) -> TranslationTlbConfig {
        self.config
    }

    pub fn entries(&self) -> &[TranslationTlbEntrySnapshot] {
        &self.entries
    }

    pub const fn next_lru(&self) -> u64 {
        self.next_lru
    }

    pub const fn stats(&self) -> TranslationTlbStats {
        self.stats
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationTlbLookup {
    kind: TranslationTlbLookupKind,
    resolution: TranslationResolution,
}

impl TranslationTlbLookup {
    fn new(kind: TranslationTlbLookupKind, resolution: TranslationResolution) -> Self {
        Self { kind, resolution }
    }

    pub const fn kind(&self) -> TranslationTlbLookupKind {
        self.kind
    }

    pub const fn resolution(&self) -> &TranslationResolution {
        &self.resolution
    }

    pub const fn physical_address(&self) -> Option<Address> {
        self.resolution.physical_address()
    }

    pub const fn fault(&self) -> Option<&TranslationFault> {
        self.resolution.fault_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TranslationTlbEntry {
    address_space: TranslationAddressSpaceId,
    virtual_page: Address,
    physical_page: Address,
    page_size: TranslationPageSize,
    permissions: TranslationPagePermissions,
    scope: TranslationTlbEntryScope,
    last_used: u64,
}

impl TranslationTlbEntry {
    fn new(
        address_space: TranslationAddressSpaceId,
        virtual_page: Address,
        physical_page: Address,
        page_size: TranslationPageSize,
        permissions: TranslationPagePermissions,
        scope: TranslationTlbEntryScope,
        last_used: u64,
    ) -> Self {
        Self {
            address_space,
            virtual_page,
            physical_page,
            page_size,
            permissions,
            scope,
            last_used,
        }
    }

    fn from_snapshot(snapshot: &TranslationTlbEntrySnapshot) -> Result<Self, TranslationError> {
        if !snapshot.page_size().is_aligned(snapshot.virtual_page()) {
            return Err(TranslationError::UnalignedVirtualPage {
                address: snapshot.virtual_page(),
                page_size: snapshot.page_size(),
            });
        }
        if !snapshot.page_size().is_aligned(snapshot.physical_page()) {
            return Err(TranslationError::UnalignedPhysicalPage {
                address: snapshot.physical_page(),
                page_size: snapshot.page_size(),
            });
        }
        entry_range(snapshot.virtual_page(), snapshot.page_size())?;
        entry_range(snapshot.physical_page(), snapshot.page_size())?;

        Ok(Self::new(
            snapshot.address_space(),
            snapshot.virtual_page(),
            snapshot.physical_page(),
            snapshot.page_size(),
            snapshot.permissions(),
            snapshot.scope(),
            snapshot.last_used(),
        ))
    }

    fn snapshot(&self) -> TranslationTlbEntrySnapshot {
        TranslationTlbEntrySnapshot::new_in_address_space(
            self.address_space,
            self.virtual_page,
            self.physical_page,
            self.page_size,
            self.permissions,
            self.last_used,
        )
        .with_scope(self.scope)
    }

    fn contains_range(&self, range: AddressRange) -> Result<bool, TranslationError> {
        Ok(entry_range(self.virtual_page, self.page_size)?.contains_range(range))
    }

    fn contains_address(&self, address: Address) -> Result<bool, TranslationError> {
        Ok(entry_range(self.virtual_page, self.page_size)?.contains(address))
    }

    fn resolve(&self, request: &TranslationRequest) -> TranslationResolution {
        if !self.permissions.allows(request.access()) {
            return TranslationResolution::fault(TranslationFault::new(
                request.virtual_address(),
                TranslationFaultKind::PermissionFault,
            ));
        }

        let offset = self.page_size.page_offset(request.virtual_address());
        TranslationResolution::mapped(Address::new(self.physical_page.get() + offset))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct TranslationTlbKey {
    address_space: TranslationAddressSpaceId,
    virtual_page: Address,
}

impl TranslationTlbKey {
    const fn new(address_space: TranslationAddressSpaceId, virtual_page: Address) -> Self {
        Self {
            address_space,
            virtual_page,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationTlb {
    config: TranslationTlbConfig,
    entries: BTreeMap<TranslationTlbKey, TranslationTlbEntry>,
    next_lru: u64,
    stats: TranslationTlbStats,
}

impl TranslationTlb {
    pub fn new(config: TranslationTlbConfig) -> Self {
        Self {
            config,
            entries: BTreeMap::new(),
            next_lru: 0,
            stats: TranslationTlbStats::default(),
        }
    }

    pub fn from_snapshot(snapshot: &TranslationTlbSnapshot) -> Result<Self, TranslationError> {
        if snapshot.entries().len() > snapshot.config().capacity() {
            return Err(TranslationError::TlbCapacityExceeded {
                capacity: snapshot.config().capacity(),
            });
        }

        let mut keys = BTreeSet::new();
        let mut entries = BTreeMap::new();
        for snapshot_entry in snapshot.entries() {
            let key = TranslationTlbKey::new(
                snapshot_entry.address_space(),
                snapshot_entry.virtual_page(),
            );
            if !keys.insert(key) {
                return Err(TranslationError::DuplicateTlbEntry {
                    virtual_page: snapshot_entry.virtual_page(),
                });
            }
            let entry = TranslationTlbEntry::from_snapshot(snapshot_entry)?;
            if snapshot_entry.last_used() >= snapshot.next_lru() {
                return Err(TranslationError::SnapshotNextLruTooSmall {
                    next_lru: snapshot.next_lru(),
                    virtual_page: snapshot_entry.virtual_page(),
                    last_used: snapshot_entry.last_used(),
                });
            }
            entries.insert(key, entry);
        }

        Ok(Self {
            config: snapshot.config(),
            entries,
            next_lru: snapshot.next_lru(),
            stats: snapshot.stats(),
        })
    }

    pub fn restore(&mut self, snapshot: &TranslationTlbSnapshot) -> Result<(), TranslationError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub const fn config(&self) -> TranslationTlbConfig {
        self.config
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub const fn stats(&self) -> TranslationTlbStats {
        self.stats
    }

    pub fn contains_virtual_page(&self, virtual_page: Address) -> bool {
        self.contains_entry(TranslationAddressSpaceId::global(), virtual_page)
    }

    pub fn contains_entry(
        &self,
        address_space: TranslationAddressSpaceId,
        virtual_page: Address,
    ) -> bool {
        self.entries
            .contains_key(&TranslationTlbKey::new(address_space, virtual_page))
    }

    pub fn flush_all(&mut self) -> usize {
        let removed = self.entries.len();
        self.entries.clear();
        removed
    }

    pub fn flush_address_space(&mut self, address_space: TranslationAddressSpaceId) -> usize {
        let before = self.entries.len();
        self.entries
            .retain(|key, _| key.address_space != address_space);
        before - self.entries.len()
    }

    pub fn flush_non_global_address_space(
        &mut self,
        address_space: TranslationAddressSpaceId,
    ) -> usize {
        let before = self.entries.len();
        self.entries.retain(|key, entry| {
            key.address_space != address_space || entry.scope == TranslationTlbEntryScope::Global
        });
        before - self.entries.len()
    }

    pub fn demap_page(
        &mut self,
        address_space: TranslationAddressSpaceId,
        virtual_address: Address,
    ) -> usize {
        self.remove_matching_pages(|key, entry| {
            key.address_space == address_space
                && entry.contains_address(virtual_address).unwrap_or(false)
        })
    }

    pub fn demap_page_all_address_spaces(&mut self, virtual_address: Address) -> usize {
        self.remove_matching_pages(|_, entry| {
            entry.contains_address(virtual_address).unwrap_or(false)
        })
    }

    pub fn translate(
        &mut self,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationTlbLookup, TranslationError> {
        self.translate_in_address_space(TranslationAddressSpaceId::global(), request, page_map)
    }

    pub fn lookup_cached(
        &mut self,
        request: &TranslationRequest,
    ) -> Result<Option<TranslationTlbLookup>, TranslationError> {
        self.lookup_cached_in_address_space(TranslationAddressSpaceId::global(), request)
    }

    pub fn lookup_cached_in_address_space(
        &mut self,
        address_space: TranslationAddressSpaceId,
        request: &TranslationRequest,
    ) -> Result<Option<TranslationTlbLookup>, TranslationError> {
        let Some(key) = self.lookup_key(address_space, request.range())? else {
            self.stats.record_miss();
            return Ok(None);
        };

        self.stats.record_hit();
        let last_used = self.next_lru()?;
        let entry = self
            .entries
            .get_mut(&key)
            .expect("TLB lookup returned a missing entry");
        entry.last_used = last_used;

        let resolution = entry.resolve(request);
        if resolution.fault_ref().is_some() {
            self.stats.record_fault();
        }
        Ok(Some(TranslationTlbLookup::new(
            TranslationTlbLookupKind::Hit,
            resolution,
        )))
    }

    pub fn translate_in_address_space(
        &mut self,
        address_space: TranslationAddressSpaceId,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationTlbLookup, TranslationError> {
        if let Some(lookup) = self.lookup_cached_in_address_space(address_space, request)? {
            return Ok(lookup);
        }

        let resolution =
            self.fill_from_page_map_in_address_space(address_space, request, page_map)?;
        Ok(TranslationTlbLookup::new(
            TranslationTlbLookupKind::Miss,
            resolution,
        ))
    }

    pub fn fill_from_page_map(
        &mut self,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationResolution, TranslationError> {
        self.fill_from_page_map_in_address_space(
            TranslationAddressSpaceId::global(),
            request,
            page_map,
        )
    }

    pub fn fill_from_page_map_in_address_space(
        &mut self,
        address_space: TranslationAddressSpaceId,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationResolution, TranslationError> {
        let resolution = page_map.translate(request);
        match resolution {
            TranslationResolution::Mapped(physical_address) => {
                if let Some(mapping) = page_map
                    .mappings()
                    .iter()
                    .find(|mapping| mapping.virtual_range().contains_range(request.range()))
                {
                    self.insert_mapping(
                        address_space,
                        request,
                        physical_address,
                        mapping,
                        page_map.page_size(),
                    )?;
                }
                Ok(TranslationResolution::Mapped(physical_address))
            }
            TranslationResolution::Fault(fault) => {
                self.stats.record_fault();
                Ok(TranslationResolution::Fault(fault))
            }
        }
    }

    pub fn fill_segments_from_page_map(
        &mut self,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationSegmentedResolution, TranslationError> {
        self.fill_segments_from_page_map_in_address_space(
            TranslationAddressSpaceId::global(),
            request,
            page_map,
        )
    }

    pub fn fill_segments_from_page_map_in_address_space(
        &mut self,
        address_space: TranslationAddressSpaceId,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationSegmentedResolution, TranslationError> {
        let resolution = page_map.translate_segments(request);
        match &resolution {
            TranslationSegmentedResolution::Mapped(segments) => {
                for segment in segments {
                    let segment_request = TranslationRequest::new(
                        request.id(),
                        segment.virtual_start(),
                        segment.size(),
                        request.access(),
                    )?;
                    let segment_resolution = self.fill_from_page_map_in_address_space(
                        address_space,
                        &segment_request,
                        page_map,
                    )?;
                    if let TranslationResolution::Fault(fault) = segment_resolution {
                        return Ok(TranslationSegmentedResolution::Fault(fault));
                    }
                }
            }
            TranslationSegmentedResolution::Fault(_) => self.stats.record_fault(),
        }

        Ok(resolution)
    }

    pub fn snapshot(&self) -> TranslationTlbSnapshot {
        TranslationTlbSnapshot::new(
            self.config,
            self.entries
                .values()
                .map(TranslationTlbEntry::snapshot)
                .collect(),
            self.next_lru,
            self.stats,
        )
    }

    fn lookup_key(
        &self,
        address_space: TranslationAddressSpaceId,
        range: AddressRange,
    ) -> Result<Option<TranslationTlbKey>, TranslationError> {
        for (key, entry) in &self.entries {
            if key.address_space == address_space && entry.contains_range(range)? {
                return Ok(Some(*key));
            }
        }

        Ok(None)
    }

    fn insert_mapping(
        &mut self,
        address_space: TranslationAddressSpaceId,
        request: &TranslationRequest,
        physical_address: Address,
        mapping: &TranslationPageMapping,
        page_size: TranslationPageSize,
    ) -> Result<(), TranslationError> {
        let virtual_page = page_size.page_address(request.virtual_address());
        if !entry_range(virtual_page, page_size)?.contains_range(request.range()) {
            return Ok(());
        }

        let physical_page = page_size.page_address(physical_address);
        let last_used = self.next_lru()?;
        let key = TranslationTlbKey::new(address_space, virtual_page);
        if let Some(entry) = self.entries.get_mut(&key) {
            *entry = TranslationTlbEntry::new(
                address_space,
                virtual_page,
                physical_page,
                page_size,
                mapping.permissions(),
                TranslationTlbEntryScope::NonGlobal,
                last_used,
            );
            self.stats.record_insert();
            return Ok(());
        }

        if self.entries.len() >= self.config.capacity() {
            self.evict_lru();
        }

        self.entries.insert(
            key,
            TranslationTlbEntry::new(
                address_space,
                virtual_page,
                physical_page,
                page_size,
                mapping.permissions(),
                TranslationTlbEntryScope::NonGlobal,
                last_used,
            ),
        );
        self.stats.record_insert();
        Ok(())
    }

    fn remove_matching_pages<F>(&mut self, mut matches: F) -> usize
    where
        F: FnMut(&TranslationTlbKey, &TranslationTlbEntry) -> bool,
    {
        let before = self.entries.len();
        self.entries.retain(|key, entry| !matches(key, entry));
        before - self.entries.len()
    }

    fn evict_lru(&mut self) {
        let Some(victim) = self
            .entries
            .iter()
            .min_by_key(|(key, entry)| (entry.last_used, **key))
            .map(|(key, _)| *key)
        else {
            return;
        };

        self.entries.remove(&victim);
        self.stats.record_eviction();
    }

    fn next_lru(&mut self) -> Result<u64, TranslationError> {
        let next = self.next_lru;
        self.next_lru = self
            .next_lru
            .checked_add(1)
            .ok_or(TranslationError::TlbOrderOverflow)?;
        Ok(next)
    }
}

fn entry_range(
    virtual_page: Address,
    page_size: TranslationPageSize,
) -> Result<AddressRange, TranslationError> {
    let size = AccessSize::new(page_size.bytes()).map_err(|_| TranslationError::ZeroPageSize)?;
    AddressRange::new(virtual_page, size).map_err(|_| TranslationError::AddressOverflow {
        start: virtual_page,
        size,
    })
}
