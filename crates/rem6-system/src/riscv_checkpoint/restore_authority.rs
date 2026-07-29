use rem6_cpu::{
    PreparedRiscvCoreRestore, RiscvCoreCheckpointRestoreInput, RiscvO3LiveCheckpointProfile,
};

use super::*;

impl RiscvCoreCheckpointPort {
    pub(super) fn restore_record(
        &self,
        record: &RiscvCoreCheckpointRecord,
    ) -> Result<(), RiscvCoreCheckpointError> {
        let prepared = self.prepare_record(record)?;
        self.core.install_prepared_checkpoint_restore(prepared);
        Ok(())
    }

    pub(super) fn validate_low_level_restore_authority(
        &self,
        record: &RiscvCoreCheckpointRecord,
    ) -> Result<(), RiscvCoreCheckpointError> {
        if record
            .o3_live_checkpoint()
            .is_some_and(|live| live.profile == RiscvO3LiveCheckpointProfile::PendingDataAddress)
        {
            return Err(
                RiscvCoreCheckpointError::PendingDataAddressRestoreRequiresSchedulerAuthority {
                    component: self.component.clone(),
                },
            );
        }
        Ok(())
    }

    fn prepare_record(
        &self,
        record: &RiscvCoreCheckpointRecord,
    ) -> Result<PreparedRiscvCoreRestore, RiscvCoreCheckpointError> {
        let mut hart = record
            .o3_live_hart_state
            .clone()
            .unwrap_or_else(|| self.core.checkpoint_hart_state());
        if record.o3_live_hart_state.is_none() {
            hart.set_pc(record.pc().get());
            for (register, value) in record.registers() {
                hart.write(*register, *value);
            }
            for (register, value) in record.float_registers() {
                hart.write_float(*register, *value);
            }
            hart.restore_vector_architectural_state(record.vector_architectural_state());
        }
        self.core
            .prepare_checkpoint_restore(RiscvCoreCheckpointRestoreInput::new(
                hart,
                record.pmp_snapshot().clone(),
                record.hart_run_state(),
                record.in_order_pipeline_snapshot().clone(),
                record.branch_predictor_payload().clone(),
                record.gshare_branch_predictor_payload().clone(),
                record.bimode_branch_predictor_payload().clone(),
                record.tournament_branch_predictor_payload().clone(),
                record.tage_sc_l_branch_predictor_payload().clone(),
                record.multiperspective_perceptron_payload().clone(),
                record.o3_runtime_payload().clone(),
                record.o3_live_checkpoint().cloned(),
            ))
            .map_err(|error| RiscvCoreCheckpointError::InvalidPreparedRestore {
                component: self.component.clone(),
                error,
            })
    }
}

impl RiscvCoreCheckpointBank {
    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, RiscvCoreCheckpointError> {
        let decoded = self.decode_and_prepare_all(registry)?;
        for (port, record, _) in &decoded {
            port.validate_low_level_restore_authority(record)?;
        }
        Ok(Self::install_decoded_restores(decoded))
    }

    pub(crate) fn restore_all_from_with_scheduler_authority(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, RiscvCoreCheckpointError> {
        let decoded = self.decode_and_prepare_all(registry)?;
        Ok(Self::install_decoded_restores(decoded))
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), RiscvCoreCheckpointError> {
        self.decode_and_prepare_all(registry)?;
        Ok(())
    }

    fn install_decoded_restores(
        decoded: Vec<(
            &RiscvCoreCheckpointPort,
            RiscvCoreCheckpointRecord,
            PreparedRiscvCoreRestore,
        )>,
    ) -> Vec<RiscvCoreCheckpointRecord> {
        let mut restored = Vec::new();
        for (port, record, prepared) in decoded {
            port.core.install_prepared_checkpoint_restore(prepared);
            restored.push(record);
        }
        restored
    }

    fn decode_and_prepare_all<'a>(
        &'a self,
        registry: &CheckpointRegistry,
    ) -> Result<
        Vec<(
            &'a RiscvCoreCheckpointPort,
            RiscvCoreCheckpointRecord,
            PreparedRiscvCoreRestore,
        )>,
        RiscvCoreCheckpointError,
    > {
        let mut decoded = Vec::with_capacity(self.ports.len());
        for port in self.ports.values() {
            decoded.push((port, port.decode_from(registry)?));
        }
        decoded
            .into_iter()
            .map(|(port, record)| {
                let prepared = port.prepare_record(&record)?;
                Ok((port, record, prepared))
            })
            .collect()
    }
}
