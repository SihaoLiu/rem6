use rem6_memory::Address;

use crate::CacheCompressedTagsConfig;

use super::{MoesiCacheBank, MoesiCacheBankError, PendingBankFill};

impl MoesiCacheBank {
    pub(super) fn install_compressed_tag_line(
        &mut self,
        line: Address,
        compressed_size_bits: Option<usize>,
    ) -> Result<(), MoesiCacheBankError> {
        let Some(compressed_tags) = self.compressed_tags.as_mut() else {
            return Ok(());
        };
        if compressed_tags.find(line).is_some() {
            compressed_tags.access(line)?;
            return Ok(());
        }
        let compressed_size_bits = compressed_size_bits
            .unwrap_or_else(|| uncompressed_fill_size_bits(compressed_tags.config()));
        let snapshot = compressed_tags.snapshot();
        let evicted_lines = compressed_tags
            .insert(line, compressed_size_bits)?
            .evicted_lines()
            .to_vec();
        if let Err(error) = self.validate_clean_stable_victims(&evicted_lines) {
            self.compressed_tags
                .as_mut()
                .expect("compressed tags were present for installation")
                .restore(&snapshot)?;
            return Err(error);
        }
        for evicted_line in evicted_lines {
            if evicted_line == self.layout.line_address(line) {
                continue;
            }
            self.lines.remove(&evicted_line);
            self.pending_fills.retain(|_, pending| {
                !matches!(
                    pending,
                    PendingBankFill::Line { line, .. } if *line == evicted_line
                )
            });
        }
        Ok(())
    }
}

fn uncompressed_fill_size_bits(config: &CacheCompressedTagsConfig) -> usize {
    let line_bits = config.line_layout().bytes() as u128 * 8;
    usize::try_from(line_bits).unwrap_or(usize::MAX)
}
