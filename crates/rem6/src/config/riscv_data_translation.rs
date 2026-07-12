use rem6_memory::{
    Address, TranslationPageMap, TranslationPagePermissions, TranslationPageSize,
    TranslationQueueConfig, TranslationTlbConfig,
};
use serde::Deserialize;

use crate::Rem6CliError;

const DEFAULT_PAGE_SIZE_BYTES: u64 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunRiscvDataTranslationConfig {
    queue: TranslationQueueConfig,
    tlb: Option<TranslationTlbConfig>,
    page_map: TranslationPageMap,
}

impl RunRiscvDataTranslationConfig {
    pub(super) fn from_file(
        config: &RunRiscvDataTranslationFileConfig,
    ) -> Result<Self, Rem6CliError> {
        let queue = TranslationQueueConfig::new(config.queue_capacity, config.latency)
            .map_err(invalid_translation)?;
        let tlb = config
            .tlb_capacity
            .map(TranslationTlbConfig::new)
            .transpose()
            .map_err(invalid_translation)?;
        let page_size = TranslationPageSize::new(config.page_size).map_err(invalid_translation)?;
        if config.mappings.is_empty() {
            return Err(invalid_translation("at least one page mapping is required"));
        }

        let mut page_map = TranslationPageMap::new(page_size);
        for mapping in &config.mappings {
            if !mapping.read && !mapping.write {
                return Err(invalid_translation(format!(
                    "mapping at virtual base {:#x} must allow read or write access",
                    mapping.virtual_base
                )));
            }
            page_map
                .map(
                    Address::new(mapping.virtual_base),
                    Address::new(mapping.physical_base),
                    mapping.pages,
                    TranslationPagePermissions::new(mapping.read, mapping.write, false),
                )
                .map_err(invalid_translation)?;
        }

        Ok(Self {
            queue,
            tlb,
            page_map,
        })
    }

    pub(crate) const fn queue(&self) -> TranslationQueueConfig {
        self.queue
    }

    pub(crate) const fn tlb(&self) -> Option<TranslationTlbConfig> {
        self.tlb
    }

    pub(crate) const fn page_map(&self) -> &TranslationPageMap {
        &self.page_map
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunRiscvDataTranslationFileConfig {
    queue_capacity: usize,
    latency: u64,
    tlb_capacity: Option<usize>,
    #[serde(default = "default_page_size_bytes")]
    page_size: u64,
    #[serde(default)]
    mappings: Vec<RunRiscvDataTranslationMappingFileConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RunRiscvDataTranslationMappingFileConfig {
    virtual_base: u64,
    physical_base: u64,
    pages: u64,
    #[serde(default = "enabled")]
    read: bool,
    #[serde(default = "enabled")]
    write: bool,
}

const fn default_page_size_bytes() -> u64 {
    DEFAULT_PAGE_SIZE_BYTES
}

const fn enabled() -> bool {
    true
}

fn invalid_translation(error: impl std::fmt::Display) -> Rem6CliError {
    Rem6CliError::InvalidRiscvDataTranslation {
        error: error.to_string(),
    }
}
