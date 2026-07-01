use crate::metadata_tables::BootElfSectionVersions;

const SHT_GNU_VERDEF: u32 = 0x6fff_fffd;
const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionVersionSummary {
    version_symbol_section_count: u64,
    version_symbol_bytes: u64,
    version_symbol_entry_count: u64,
    version_definition_section_count: u64,
    version_definition_bytes: u64,
    version_definition_entry_count: u64,
    version_needed_section_count: u64,
    version_needed_bytes: u64,
    version_needed_entry_count: u64,
}

impl ElfSectionVersionSummary {
    pub(crate) const fn into_metadata(self) -> BootElfSectionVersions {
        BootElfSectionVersions::new(
            self.version_symbol_section_count,
            self.version_symbol_bytes,
            self.version_symbol_entry_count,
            self.version_definition_section_count,
            self.version_definition_bytes,
            self.version_definition_entry_count,
            self.version_needed_section_count,
            self.version_needed_bytes,
            self.version_needed_entry_count,
        )
    }

    pub(crate) fn record_section(&mut self, kind: u32, size: u64, entry_size: u64, info: u32) {
        let entry_count = section_entry_count(size, entry_size);
        match kind {
            SHT_GNU_VERSYM => {
                self.version_symbol_section_count += 1;
                self.version_symbol_bytes = self.version_symbol_bytes.saturating_add(size);
                self.version_symbol_entry_count =
                    self.version_symbol_entry_count.saturating_add(entry_count);
            }
            SHT_GNU_VERDEF => {
                self.version_definition_section_count += 1;
                self.version_definition_bytes = self.version_definition_bytes.saturating_add(size);
                self.version_definition_entry_count = self
                    .version_definition_entry_count
                    .saturating_add(u64::from(info));
            }
            SHT_GNU_VERNEED => {
                self.version_needed_section_count += 1;
                self.version_needed_bytes = self.version_needed_bytes.saturating_add(size);
                self.version_needed_entry_count = self
                    .version_needed_entry_count
                    .saturating_add(u64::from(info));
            }
            _ => {}
        }
    }
}

fn section_entry_count(size: u64, entry_size: u64) -> u64 {
    if entry_size == 0 {
        0
    } else {
        size / entry_size
    }
}
