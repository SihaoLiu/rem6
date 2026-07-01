use crate::metadata_tables::BootElfSectionIndexTables;

const SHT_SYMTAB_SHNDX: u32 = 18;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionIndexTableSummary {
    section_count: u64,
    byte_size: u64,
    entry_count: u64,
}

impl ElfSectionIndexTableSummary {
    pub(crate) fn record(&mut self, kind: u32, size: u64, entry_size: u64) {
        if kind == SHT_SYMTAB_SHNDX {
            self.section_count += 1;
            self.byte_size = self.byte_size.saturating_add(size);
            self.entry_count = self
                .entry_count
                .saturating_add(section_entry_count(size, entry_size));
        }
    }

    pub(crate) const fn into_metadata(self) -> BootElfSectionIndexTables {
        BootElfSectionIndexTables::new(self.section_count, self.byte_size, self.entry_count)
    }
}

const fn section_entry_count(size: u64, entry_size: u64) -> u64 {
    if entry_size == 0 {
        0
    } else {
        size / entry_size
    }
}
