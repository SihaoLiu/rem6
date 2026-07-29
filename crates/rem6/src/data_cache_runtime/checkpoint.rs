use super::*;

use rem6_checkpoint::CheckpointComponentId;
use rem6_system::{CheckpointRuntimeState, MsiBankCheckpointPort};

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliDataCacheRuntimeCheckpoint {
    records: Vec<RiscvDataCacheRunRecord>,
    prefetch: Option<CliDataCachePrefetchRuntime>,
    prefetch_fills: u64,
    line_ready_ticks: BTreeMap<Address, u64>,
    error: Option<String>,
}

impl CliCacheHierarchy {
    pub(crate) fn msi_checkpoint_unsupported_reason(&self) -> Option<&'static str> {
        self.levels
            .iter()
            .find_map(CliDataCacheRuntime::msi_checkpoint_unsupported_reason)
    }

    pub(crate) fn msi_checkpoint_ports(
        &self,
        component_prefix: &str,
    ) -> Result<Vec<MsiBankCheckpointPort>, Rem6CliError> {
        self.levels
            .iter()
            .enumerate()
            .map(|(level, runtime)| {
                let component = CheckpointComponentId::new(format!("{component_prefix}{level}"))
                    .map_err(execute_error)?;
                runtime.msi_checkpoint_port(component)
            })
            .collect()
    }
}

impl CliDataCacheRuntime {
    fn msi_checkpoint_unsupported_reason(&self) -> Option<&'static str> {
        match &*self.harness.lock().expect("CLI data cache lock") {
            CliDataCacheHarness::Msi(_) => None,
            CliDataCacheHarness::Mesi(_)
            | CliDataCacheHarness::Moesi(_)
            | CliDataCacheHarness::Chi(_) => {
                Some("host checkpoint actions require MSI cache protocols")
            }
        }
    }

    fn msi_checkpoint_port(
        &self,
        component: CheckpointComponentId,
    ) -> Result<MsiBankCheckpointPort, Rem6CliError> {
        let harness = {
            let harness = self.harness.lock().expect("CLI data cache lock");
            match &*harness {
                CliDataCacheHarness::Msi(harness) => Arc::clone(harness),
                CliDataCacheHarness::Mesi(_)
                | CliDataCacheHarness::Moesi(_)
                | CliDataCacheHarness::Chi(_) => {
                    return Err(execute_error(
                        "host checkpoint actions require MSI cache protocols",
                    ));
                }
            }
        };
        let capture_runtime = self.clone();
        let validate_runtime = self.clone();
        let restore_runtime = self.clone();
        let runtime_state = CheckpointRuntimeState::stored(
            move || capture_runtime.capture_checkpoint_state(),
            move |snapshot| validate_runtime.validate_checkpoint_state(snapshot),
            move |snapshot| {
                restore_runtime.restore_checkpoint_state(snapshot);
            },
        );
        Ok(MsiBankCheckpointPort::new(component, harness).with_runtime_state(runtime_state))
    }

    fn capture_checkpoint_state(&self) -> Result<CliDataCacheRuntimeCheckpoint, String> {
        Ok(CliDataCacheRuntimeCheckpoint {
            records: self
                .records
                .lock()
                .expect("CLI data cache record lock")
                .clone(),
            prefetch: self.prefetch.as_ref().map(|prefetch| {
                prefetch
                    .lock()
                    .expect("CLI data cache prefetch lock")
                    .clone()
            }),
            prefetch_fills: *self
                .prefetch_fills
                .lock()
                .expect("CLI data cache prefetch fill lock"),
            line_ready_ticks: self
                .line_ready_ticks
                .lock()
                .expect("CLI data cache ready-tick lock")
                .clone(),
            error: self
                .error
                .lock()
                .expect("CLI data cache error lock")
                .clone(),
        })
    }

    fn validate_checkpoint_state(
        &self,
        snapshot: &CliDataCacheRuntimeCheckpoint,
    ) -> Result<(), String> {
        if snapshot
            .line_ready_ticks
            .keys()
            .any(|line| self.layout.line_address(*line) != *line)
        {
            return Err("ready-tick entry is not cache-line aligned".to_string());
        }
        match (&self.prefetch, &snapshot.prefetch) {
            (Some(runtime), Some(snapshot)) => {
                let runtime = runtime.lock().expect("CLI data cache prefetch lock");
                if runtime.tagged.config() != snapshot.tagged.config()
                    || runtime.queue.config() != snapshot.queue.config()
                {
                    return Err("prefetch checkpoint configuration does not match".to_string());
                }
            }
            (None, None) => {}
            _ => return Err("prefetch checkpoint presence does not match".to_string()),
        }
        Ok(())
    }

    fn restore_checkpoint_state(&self, snapshot: CliDataCacheRuntimeCheckpoint) {
        match (&self.prefetch, snapshot.prefetch) {
            (Some(runtime), Some(snapshot)) => {
                *runtime.lock().expect("CLI data cache prefetch lock") = snapshot;
            }
            (None, None) => {}
            _ => unreachable!("validated prefetch checkpoint presence must match"),
        }
        *self.records.lock().expect("CLI data cache record lock") = snapshot.records;
        *self
            .prefetch_fills
            .lock()
            .expect("CLI data cache prefetch fill lock") = snapshot.prefetch_fills;
        *self
            .line_ready_ticks
            .lock()
            .expect("CLI data cache ready-tick lock") = snapshot.line_ready_ticks;
        *self.error.lock().expect("CLI data cache error lock") = snapshot.error;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CliCachePrefetcher;
    use rem6_memory::AgentId;

    #[test]
    fn runtime_checkpoint_restores_prefetch_state() {
        let layout = CacheLineLayout::new(32).unwrap();
        let runtime = CliDataCacheRuntime::new_msi_bank(
            layout,
            [AgentId::new(0)],
            Some(CliCachePrefetcher::TaggedNextLine),
        )
        .unwrap();
        let prefetch = runtime.prefetch.as_ref().unwrap();
        {
            let mut prefetch = prefetch.lock().unwrap();
            prefetch.next_sequence += 7;
            prefetch.issued_lines.insert(Address::new(0x1000));
            prefetch.pending_useful_lines.insert(Address::new(0x1000));
        }
        let snapshot = runtime.capture_checkpoint_state().unwrap();
        {
            let mut prefetch = prefetch.lock().unwrap();
            prefetch.next_sequence += 11;
            prefetch.issued_lines.clear();
            prefetch.pending_useful_lines.clear();
        }

        runtime.validate_checkpoint_state(&snapshot).unwrap();
        let expected = snapshot.prefetch.clone().unwrap();
        runtime.restore_checkpoint_state(snapshot);

        assert_eq!(*prefetch.lock().unwrap(), expected);
    }
}
