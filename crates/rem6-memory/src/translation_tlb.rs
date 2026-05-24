use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AccessSize, Address, AddressRange, TranslationError, TranslationFault, TranslationFaultKind,
    TranslationPageMap, TranslationPageMapping, TranslationPagePermissions, TranslationPageSize,
    TranslationRequest, TranslationResolution,
};

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
    virtual_page: Address,
    physical_page: Address,
    page_size: TranslationPageSize,
    permissions: TranslationPagePermissions,
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
        Self {
            virtual_page,
            physical_page,
            page_size,
            permissions,
            last_used,
        }
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
    virtual_page: Address,
    physical_page: Address,
    page_size: TranslationPageSize,
    permissions: TranslationPagePermissions,
    last_used: u64,
}

impl TranslationTlbEntry {
    fn new(
        virtual_page: Address,
        physical_page: Address,
        page_size: TranslationPageSize,
        permissions: TranslationPagePermissions,
        last_used: u64,
    ) -> Self {
        Self {
            virtual_page,
            physical_page,
            page_size,
            permissions,
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
            snapshot.virtual_page(),
            snapshot.physical_page(),
            snapshot.page_size(),
            snapshot.permissions(),
            snapshot.last_used(),
        ))
    }

    fn snapshot(&self) -> TranslationTlbEntrySnapshot {
        TranslationTlbEntrySnapshot::new(
            self.virtual_page,
            self.physical_page,
            self.page_size,
            self.permissions,
            self.last_used,
        )
    }

    fn contains_range(&self, range: AddressRange) -> Result<bool, TranslationError> {
        Ok(entry_range(self.virtual_page, self.page_size)?.contains_range(range))
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationTlb {
    config: TranslationTlbConfig,
    entries: BTreeMap<Address, TranslationTlbEntry>,
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

        let mut virtual_pages = BTreeSet::new();
        let mut entries = BTreeMap::new();
        let mut minimum_next_lru = 0;
        for snapshot_entry in snapshot.entries() {
            if !virtual_pages.insert(snapshot_entry.virtual_page()) {
                return Err(TranslationError::DuplicateTlbEntry {
                    virtual_page: snapshot_entry.virtual_page(),
                });
            }
            let entry = TranslationTlbEntry::from_snapshot(snapshot_entry)?;
            minimum_next_lru = minimum_next_lru.max(
                snapshot_entry
                    .last_used()
                    .checked_add(1)
                    .ok_or(TranslationError::TlbOrderOverflow)?,
            );
            entries.insert(snapshot_entry.virtual_page(), entry);
        }

        Ok(Self {
            config: snapshot.config(),
            entries,
            next_lru: snapshot.next_lru().max(minimum_next_lru),
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
        self.entries.contains_key(&virtual_page)
    }

    pub fn translate(
        &mut self,
        request: &TranslationRequest,
        page_map: &TranslationPageMap,
    ) -> Result<TranslationTlbLookup, TranslationError> {
        if let Some(virtual_page) = self.lookup_virtual_page(request.range())? {
            self.stats.record_hit();
            let last_used = self.next_lru()?;
            let entry = self
                .entries
                .get_mut(&virtual_page)
                .expect("TLB lookup returned a missing entry");
            entry.last_used = last_used;

            let resolution = entry.resolve(request);
            if resolution.fault_ref().is_some() {
                self.stats.record_fault();
            }
            return Ok(TranslationTlbLookup::new(
                TranslationTlbLookupKind::Hit,
                resolution,
            ));
        }

        self.stats.record_miss();
        let resolution = page_map.translate(request);
        match resolution {
            TranslationResolution::Mapped(physical_address) => {
                if let Some(mapping) = page_map
                    .mappings()
                    .iter()
                    .find(|mapping| mapping.virtual_range().contains_range(request.range()))
                {
                    self.insert_mapping(request, physical_address, mapping, page_map.page_size())?;
                }
                Ok(TranslationTlbLookup::new(
                    TranslationTlbLookupKind::Miss,
                    TranslationResolution::Mapped(physical_address),
                ))
            }
            TranslationResolution::Fault(fault) => {
                self.stats.record_fault();
                Ok(TranslationTlbLookup::new(
                    TranslationTlbLookupKind::Miss,
                    TranslationResolution::Fault(fault),
                ))
            }
        }
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

    fn lookup_virtual_page(
        &self,
        range: AddressRange,
    ) -> Result<Option<Address>, TranslationError> {
        for (virtual_page, entry) in &self.entries {
            if entry.contains_range(range)? {
                return Ok(Some(*virtual_page));
            }
        }

        Ok(None)
    }

    fn insert_mapping(
        &mut self,
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
        if let Some(entry) = self.entries.get_mut(&virtual_page) {
            *entry = TranslationTlbEntry::new(
                virtual_page,
                physical_page,
                page_size,
                mapping.permissions(),
                last_used,
            );
            self.stats.record_insert();
            return Ok(());
        }

        if self.entries.len() >= self.config.capacity() {
            self.evict_lru();
        }

        self.entries.insert(
            virtual_page,
            TranslationTlbEntry::new(
                virtual_page,
                physical_page,
                page_size,
                mapping.permissions(),
                last_used,
            ),
        );
        self.stats.record_insert();
        Ok(())
    }

    fn evict_lru(&mut self) {
        let Some(victim) = self
            .entries
            .values()
            .min_by_key(|entry| (entry.last_used, entry.virtual_page))
            .map(|entry| entry.virtual_page)
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
