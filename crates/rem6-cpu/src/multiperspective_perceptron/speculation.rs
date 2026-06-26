use super::*;

impl MultiperspectivePerceptron {
    pub(crate) fn predict_with_thread_snapshot(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
        thread_before: MultiperspectivePerceptronThreadSnapshot,
    ) -> Result<MultiperspectivePerceptronPrediction, MultiperspectivePerceptronError> {
        self.thread_index(cpu)?;
        self.lookup_count += 1;
        self.predict_with_thread_snapshot_and_count(
            cpu,
            pc,
            conditional,
            thread_before,
            self.lookup_count,
        )
    }

    pub(crate) fn shifted_thread_snapshot(
        &self,
        mut thread: MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        taken: bool,
        target: Address,
    ) -> MultiperspectivePerceptronThreadSnapshot {
        Self::update_snapshot_thread_history(&self.config, &mut thread, pc, taken, target);
        thread
    }

    pub(super) fn predict_with_thread_snapshot_and_count(
        &self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
        thread_before: MultiperspectivePerceptronThreadSnapshot,
        lookup_count: u64,
    ) -> Result<MultiperspectivePerceptronPrediction, MultiperspectivePerceptronError> {
        let pc2 = pc2(pc);
        let hpc = hash_pc(pc.get() as u32, self.config.pc_shift);
        if !conditional {
            let history = MultiperspectivePerceptronHistory::unconditional(
                cpu,
                pc,
                hpc,
                thread_before,
                lookup_count,
            );
            return Ok(MultiperspectivePerceptronPrediction { history });
        }

        let filter_index = self.filter_index(&thread_before, hpc);
        let filter_before = filter_index.map(|index| thread_before.filter_table[index].clone());
        let mut filtered = false;
        let mut used_static_prediction = false;
        let mut predicted_taken = false;
        let mut linear_sum = 0;
        let mut best_sum = 0;
        let feature_indices;
        let feature_values;

        if let Some(filter) = &filter_before {
            if filter.always_not_taken_so_far() {
                filtered = true;
                predicted_taken = false;
            } else if filter.always_taken_so_far() {
                filtered = true;
                predicted_taken = true;
            } else if filter.never_seen() {
                used_static_prediction = true;
            }
        }

        if !filtered {
            let output = self.compute_output(&thread_before, pc, pc2, hpc);
            linear_sum = output.linear_sum;
            best_sum = output.best_sum;
            feature_indices = output.feature_indices;
            feature_values = output.feature_values;
            if !used_static_prediction {
                predicted_taken = if linear_sum.abs() <= self.config.threshold {
                    best_sum >= 1
                } else {
                    linear_sum >= 1
                };
            }
        } else {
            feature_indices = self.feature_indices(&thread_before, pc, pc2, hpc);
            feature_values = Vec::new();
        }

        let history = MultiperspectivePerceptronHistory {
            cpu,
            pc,
            conditional,
            predicted_taken,
            filtered,
            used_static_prediction,
            linear_sum,
            best_sum,
            feature_indices,
            feature_values,
            hpc,
            filter_index,
            filter_before,
            thread_before,
            lookup_count,
        };
        Ok(MultiperspectivePerceptronPrediction { history })
    }

    pub(super) fn update_snapshot_thread_history(
        config: &MultiperspectivePerceptronConfig,
        thread: &mut MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        taken: bool,
        target: Address,
    ) {
        let pc2 = pc2(pc);
        let hashed_taken = if config.hash_taken {
            taken ^ (((pc.get() >> config.pcbit) & 1) != 0)
        } else {
            taken
        };

        if !thread.global_history.is_empty() {
            thread.global_history.insert(0, hashed_taken);
            thread.global_history.truncate(thread.max_global_history);
        }
        if !thread.path_history.is_empty() {
            thread.path_history.insert(0, pc2);
            thread.path_history.truncate(thread.max_path_entries);
        }
        let local_index = thread.local_index(pc);
        let mask = bit_mask(config.local_history_length);
        thread.local_histories[local_index] =
            ((thread.local_histories[local_index] << 1) | u64::from(hashed_taken)) & mask;

        if !thread.recency_stack.is_empty() {
            thread.insert_recency(pc2);
        }
        let backward = target.get() < pc.get();
        if backward {
            if taken {
                thread.imli_counters[0] = thread.imli_counters[0].saturating_add(1);
            } else {
                thread.imli_counters[0] = 0;
            }
            if !taken {
                thread.imli_counters[1] = thread.imli_counters[1].saturating_add(1);
            } else {
                thread.imli_counters[1] = 0;
            }
        } else {
            if taken {
                thread.imli_counters[2] = thread.imli_counters[2].saturating_add(1);
            } else {
                thread.imli_counters[2] = 0;
            }
            if !taken {
                thread.imli_counters[3] = thread.imli_counters[3].saturating_add(1);
            } else {
                thread.imli_counters[3] = 0;
            }
        }
        thread.last_ghist_bit = taken;
    }
}
