use crate::metadata_tables::BootElfSectionTypeRanges;

const SHT_LOOS: u32 = 0x6000_0000;
const SHT_HIOS: u32 = 0x6fff_ffff;
const SHT_LOPROC: u32 = 0x7000_0000;
const SHT_HIPROC: u32 = 0x7fff_ffff;
const SHT_LOUSER: u32 = 0x8000_0000;
const SHT_HIUSER: u32 = 0x8fff_ffff;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionTypeRangeSummary {
    os_specific_count: u64,
    os_specific_bytes: u64,
    processor_specific_count: u64,
    processor_specific_bytes: u64,
    application_specific_count: u64,
    application_specific_bytes: u64,
}

impl ElfSectionTypeRangeSummary {
    pub(crate) fn record(&mut self, kind: u32, size: u64) {
        if (SHT_LOOS..=SHT_HIOS).contains(&kind) {
            self.os_specific_count += 1;
            self.os_specific_bytes = self.os_specific_bytes.saturating_add(size);
        } else if (SHT_LOPROC..=SHT_HIPROC).contains(&kind) {
            self.processor_specific_count += 1;
            self.processor_specific_bytes = self.processor_specific_bytes.saturating_add(size);
        } else if (SHT_LOUSER..=SHT_HIUSER).contains(&kind) {
            self.application_specific_count += 1;
            self.application_specific_bytes = self.application_specific_bytes.saturating_add(size);
        }
    }

    pub(crate) const fn into_metadata(self) -> BootElfSectionTypeRanges {
        BootElfSectionTypeRanges::new(
            self.os_specific_count,
            self.os_specific_bytes,
            self.processor_specific_count,
            self.processor_specific_bytes,
            self.application_specific_count,
            self.application_specific_bytes,
        )
    }
}
