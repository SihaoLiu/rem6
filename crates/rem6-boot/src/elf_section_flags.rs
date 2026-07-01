use crate::metadata_tables::BootElfSectionFlags;

const SHF_MERGE: u64 = 1 << 4;
const SHF_STRINGS: u64 = 1 << 5;
const SHF_INFO_LINK: u64 = 1 << 6;
const SHF_LINK_ORDER: u64 = 1 << 7;
const SHF_OS_NONCONFORMING: u64 = 1 << 8;
const SHF_GROUP: u64 = 1 << 9;
const SHF_TLS: u64 = 1 << 10;
const SHF_COMPRESSED: u64 = 1 << 11;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionExtraFlagSummary {
    merge_count: u64,
    strings_count: u64,
    info_link_count: u64,
    link_order_count: u64,
    os_nonconforming_count: u64,
    group_count: u64,
    tls_count: u64,
    compressed_count: u64,
}

impl ElfSectionExtraFlagSummary {
    pub(crate) fn record(&mut self, flags: u64) -> bool {
        self.merge_count += u64::from(flags & SHF_MERGE != 0);
        self.strings_count += u64::from(flags & SHF_STRINGS != 0);
        self.info_link_count += u64::from(flags & SHF_INFO_LINK != 0);
        self.link_order_count += u64::from(flags & SHF_LINK_ORDER != 0);
        self.os_nonconforming_count += u64::from(flags & SHF_OS_NONCONFORMING != 0);
        self.group_count += u64::from(flags & SHF_GROUP != 0);
        self.tls_count += u64::from(flags & SHF_TLS != 0);
        self.compressed_count += u64::from(flags & SHF_COMPRESSED != 0);
        flags & SHF_TLS != 0
    }

    pub(crate) const fn into_metadata(
        self,
        allocated_count: u64,
        writable_count: u64,
        executable_count: u64,
        nobits_count: u64,
    ) -> BootElfSectionFlags {
        BootElfSectionFlags::with_extended_counts(
            allocated_count,
            writable_count,
            executable_count,
            nobits_count,
            self.merge_count,
            self.strings_count,
            self.info_link_count,
            self.link_order_count,
            self.os_nonconforming_count,
            self.group_count,
            self.tls_count,
            self.compressed_count,
        )
    }
}
