use rem6_memory::Address;
use rem6_protocol_mesi::MesiState;

use super::{MesiCacheBank, MesiCacheBankError, PendingBankFill};

impl MesiCacheBank {
    pub(super) fn install_sector_tag_line(
        &mut self,
        line: Address,
    ) -> Result<(), MesiCacheBankError> {
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
    ) -> Result<(), MesiCacheBankError> {
        for evicted_line in evicted_lines {
            let Some(controller) = self.lines.get(evicted_line) else {
                continue;
            };
            match controller.state() {
                MesiState::Modified => {
                    return Err(MesiCacheBankError::DirtyReplacementRequiresWriteQueue {
                        line: *evicted_line,
                    });
                }
                MesiState::InvalidToShared
                | MesiState::InvalidToExclusive
                | MesiState::InvalidToModified
                | MesiState::SharedToModified => {
                    return Err(MesiCacheBankError::TransientReplacementRequiresStableLine {
                        line: *evicted_line,
                    });
                }
                MesiState::Invalid | MesiState::Shared | MesiState::Exclusive => {}
            }
        }
        Ok(())
    }
}
