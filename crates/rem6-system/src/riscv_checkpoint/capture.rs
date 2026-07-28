use super::*;

impl RiscvCoreCheckpointBank {
    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        let records = self.stage_all_into_impl(registry, None)?;
        self.commit_checkpoint_capture();
        Ok(records)
    }

    pub fn capture_all_into_at(
        &self,
        registry: &mut CheckpointRegistry,
        tick: u64,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        let records = self.stage_all_into_impl(registry, Some(tick))?;
        self.commit_checkpoint_capture();
        Ok(records)
    }

    pub(crate) fn stage_all_into_at(
        &self,
        registry: &mut CheckpointRegistry,
        tick: u64,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        self.stage_all_into_impl(registry, Some(tick))
    }

    fn stage_all_into_impl(
        &self,
        registry: &mut CheckpointRegistry,
        tick: Option<u64>,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        let captured = self
            .ports
            .values()
            .map(|port| {
                port.capture_checked_record(tick, true)
                    .map(|record| (port, record))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in &captured {
            port.write_record(registry, record)?;
        }
        Ok(captured.into_iter().map(|(_, record)| record).collect())
    }

    pub(crate) fn stage_all_for_execution_mode_switch_into(
        &self,
        registry: &mut CheckpointRegistry,
        tick: u64,
        accept_stable_rejection: bool,
    ) -> Result<(Vec<RiscvCoreCheckpointRecord>, bool), CheckpointError> {
        let captured = self
            .ports
            .values()
            .map(
                |port| match port.capture_checked_record(Some(tick), accept_stable_rejection) {
                    Ok(record) if record.o3_live_checkpoint().is_none() => Ok((port, record, true)),
                    Ok(_) => Err(CheckpointError::ComponentNotQuiescent {
                        component: port.component.clone(),
                    }),
                    Err(CheckpointError::ComponentNotQuiescent { .. }) => port
                        .capture_execution_mode_switch_record()
                        .map(|record| (port, record, false)),
                    Err(error) => Err(error),
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record, _restorable) in &captured {
            port.write_record(registry, record)?;
        }
        let restorable = captured
            .iter()
            .all(|(_port, _record, restorable)| *restorable);
        Ok((
            captured
                .into_iter()
                .map(|(_port, record, _restorable)| record)
                .collect(),
            restorable,
        ))
    }

    pub(crate) fn commit_checkpoint_capture(&self) {
        for port in self.ports.values() {
            port.commit_checkpoint_capture();
        }
    }
}
