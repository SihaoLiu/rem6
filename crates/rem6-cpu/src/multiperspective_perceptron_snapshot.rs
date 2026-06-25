use crate::multiperspective_perceptron::{
    allocate_table_entries, max_global_history, max_path_entries, max_recency_entries,
    MultiperspectivePerceptronConfig, MultiperspectivePerceptronError,
    MultiperspectivePerceptronSnapshot,
};

pub(crate) fn validate_snapshot_shape(
    config: &MultiperspectivePerceptronConfig,
    snapshot: &MultiperspectivePerceptronSnapshot,
) -> Result<(), MultiperspectivePerceptronError> {
    if snapshot.tables.len() != config.features.len()
        || snapshot.table_entries.len() != config.features.len()
        || snapshot.threads.len() != config.threads()
        || snapshot.mpreds.len() != config.features.len()
    {
        return Err(MultiperspectivePerceptronError::SnapshotShapeMismatch {
            expected_tables: config.features.len(),
            actual_tables: snapshot.tables.len(),
            expected_threads: config.threads(),
            actual_threads: snapshot.threads.len(),
        });
    }

    let expected_table_entries = allocate_table_entries(config)?;
    for (feature_index, (actual, expected)) in snapshot
        .table_entries
        .iter()
        .zip(&expected_table_entries)
        .enumerate()
    {
        if actual != expected {
            return Err(
                MultiperspectivePerceptronError::SnapshotTableEntriesMismatch {
                    feature_index,
                    expected: *expected,
                    actual: *actual,
                },
            );
        }
    }

    for (feature_index, (feature, table)) in
        config.features.iter().zip(&snapshot.tables).enumerate()
    {
        if table.len() != expected_table_entries[feature_index] {
            return Err(
                MultiperspectivePerceptronError::SnapshotTableEntriesMismatch {
                    feature_index,
                    expected: expected_table_entries[feature_index],
                    actual: table.len(),
                },
            );
        }
        let max_magnitude = max_magnitude(feature.width);
        for (table_index, weight) in table.iter().enumerate() {
            if weight.magnitude > max_magnitude
                || weight.sign_bits.len() != config.n_sign_bits as usize
            {
                return Err(MultiperspectivePerceptronError::InvalidCheckpointWeight {
                    feature_index,
                    table_index,
                    magnitude: weight.magnitude,
                    max_magnitude,
                    sign_bits: weight.sign_bits.len(),
                    expected_sign_bits: config.n_sign_bits as usize,
                });
            }
        }
    }

    let expected_global_history = max_global_history(config);
    let expected_path_entries = max_path_entries(config).max(1);
    let expected_recency_entries = max_recency_entries(config);
    let local_history_mask = bit_mask(config.local_history_length);
    for thread in &snapshot.threads {
        if thread.max_global_history != expected_global_history
            || thread.max_path_entries != expected_path_entries
            || thread.filter_table.len() != config.num_filter_entries
            || thread.global_history.len() != expected_global_history
            || thread.local_histories.len() != config.num_local_histories
            || thread.path_history.len() != expected_path_entries
            || thread.recency_stack.len() != expected_recency_entries
        {
            return Err(MultiperspectivePerceptronError::SnapshotShapeMismatch {
                expected_tables: config.features.len(),
                actual_tables: snapshot.tables.len(),
                expected_threads: config.threads,
                actual_threads: snapshot.threads.len(),
            });
        }
        for history in &thread.local_histories {
            if *history > local_history_mask {
                return Err(MultiperspectivePerceptronError::CheckpointValueTooLarge {
                    name: "local-history",
                    value: usize::try_from(*history).unwrap_or(usize::MAX),
                    max: usize::try_from(local_history_mask).unwrap_or(usize::MAX),
                });
            }
        }
    }

    Ok(())
}

fn max_magnitude(width: u8) -> u8 {
    (1u8 << (width - 1)) - 1
}

fn bit_mask(bits: u8) -> u64 {
    if bits >= u64::BITS as u8 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MultiperspectivePerceptron, MultiperspectivePerceptronFeature};

    fn tiny_config() -> MultiperspectivePerceptronConfig {
        MultiperspectivePerceptronConfig::with_options(
            1,
            0,
            4,
            4,
            4,
            -1,
            1,
            -5,
            5,
            -1,
            1,
            1,
            1,
            0,
            0,
            0,
            0,
            0,
            2,
            2,
            0,
            0xff,
            false,
            true,
            0,
            4,
            3,
            64,
            1,
            false,
            vec![MultiperspectivePerceptronFeature::bias(1, 4, 2)],
        )
        .unwrap()
    }

    #[test]
    fn restore_rejects_truncated_inner_table_without_mutating_predictor() {
        let mut predictor = MultiperspectivePerceptron::new(tiny_config()).unwrap();
        let before = predictor.snapshot();
        let mut malformed = before.clone();
        malformed.tables[0].pop();

        assert_eq!(
            predictor.restore(&malformed),
            Err(
                MultiperspectivePerceptronError::SnapshotTableEntriesMismatch {
                    feature_index: 0,
                    expected: before.table_entries[0],
                    actual: before.table_entries[0] - 1,
                }
            )
        );
        assert_eq!(predictor.snapshot(), before);
    }
}
