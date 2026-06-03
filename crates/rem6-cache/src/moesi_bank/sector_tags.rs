use rem6_memory::Address;

use super::{MoesiCacheBank, MoesiCacheBankError, PendingBankFill};

impl MoesiCacheBank {
    pub(super) fn install_sector_tag_line(
        &mut self,
        line: Address,
    ) -> Result<(), MoesiCacheBankError> {
        let Some(sector_tags) = self.sector_tags.as_mut() else {
            return Ok(());
        };
        let snapshot = sector_tags.snapshot();
        let evicted_lines = sector_tags.insert(line)?.evicted_lines().to_vec();
        if let Err(error) = self.validate_clean_stable_victims(&evicted_lines) {
            self.sector_tags
                .as_mut()
                .expect("sector tags were present for installation")
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

    fn validate_clean_stable_victims(
        &self,
        evicted_lines: &[Address],
    ) -> Result<(), MoesiCacheBankError> {
        for evicted_line in evicted_lines {
            let Some(controller) = self.lines.get(evicted_line) else {
                continue;
            };
            let state = controller.state();
            if state.is_dirty_owner() {
                return Err(MoesiCacheBankError::DirtyReplacementRequiresWriteQueue {
                    line: *evicted_line,
                });
            }
            if state.is_transient() {
                return Err(
                    MoesiCacheBankError::TransientReplacementRequiresStableLine {
                        line: *evicted_line,
                    },
                );
            }
        }
        Ok(())
    }
}
