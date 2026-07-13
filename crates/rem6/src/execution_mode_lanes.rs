use rem6_system::ExecutionMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionModeLane {
    mode: ExecutionMode,
    name: &'static str,
    o3_trace_stat_suffix: &'static str,
    o3_checkpoint_restore_trace_stat_suffix: &'static str,
}

impl ExecutionModeLane {
    const fn new(
        mode: ExecutionMode,
        name: &'static str,
        o3_trace_stat_suffix: &'static str,
        o3_checkpoint_restore_trace_stat_suffix: &'static str,
    ) -> Self {
        Self {
            mode,
            name,
            o3_trace_stat_suffix,
            o3_checkpoint_restore_trace_stat_suffix,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn o3_trace_stat_suffix(self) -> &'static str {
        self.o3_trace_stat_suffix
    }

    pub(crate) const fn o3_checkpoint_restore_trace_stat_suffix(self) -> &'static str {
        self.o3_checkpoint_restore_trace_stat_suffix
    }
}

macro_rules! define_execution_mode_lanes {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        pub(crate) const EXECUTION_MODE_LANE_COUNT: usize =
            [$(ExecutionMode::$variant),+].len();

        pub(crate) const EXECUTION_MODE_LANES:
            [ExecutionModeLane; EXECUTION_MODE_LANE_COUNT] = [
                $(ExecutionModeLane::new(
                    ExecutionMode::$variant,
                    $name,
                    concat!("execution_mode.", $name),
                    concat!("checkpoint_restore.execution_mode_authority.mode.", $name),
                )),+
            ];

        pub(crate) const fn execution_mode_name(mode: ExecutionMode) -> &'static str {
            match mode {
                $(ExecutionMode::$variant => $name,)+
            }
        }
    };
}

define_execution_mode_lanes! {
    Functional => "functional",
    Timing => "timing",
    Detailed => "detailed",
}

pub(crate) fn execution_mode_from_name(name: &str) -> Option<ExecutionMode> {
    EXECUTION_MODE_LANES
        .iter()
        .find(|lane| lane.name == name)
        .map(|lane| lane.mode)
}

pub(crate) fn execution_mode_lane_index(name: &str) -> Option<usize> {
    EXECUTION_MODE_LANES
        .iter()
        .position(|lane| lane.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rem6_system::ExecutionMode;

    use super::{
        execution_mode_from_name, execution_mode_lane_index, execution_mode_name,
        EXECUTION_MODE_LANES, EXECUTION_MODE_LANE_COUNT,
    };

    #[test]
    fn execution_mode_lane_vocabulary_and_order_are_stable() {
        assert_eq!(EXECUTION_MODE_LANE_COUNT, EXECUTION_MODE_LANES.len());
        assert_eq!(
            EXECUTION_MODE_LANES.map(|lane| lane.name()),
            ["functional", "timing", "detailed"]
        );
        assert_eq!(
            EXECUTION_MODE_LANES.map(|lane| lane.o3_trace_stat_suffix()),
            [
                "execution_mode.functional",
                "execution_mode.timing",
                "execution_mode.detailed",
            ]
        );
        assert_eq!(
            EXECUTION_MODE_LANES.map(|lane| lane.o3_checkpoint_restore_trace_stat_suffix()),
            [
                "checkpoint_restore.execution_mode_authority.mode.functional",
                "checkpoint_restore.execution_mode_authority.mode.timing",
                "checkpoint_restore.execution_mode_authority.mode.detailed",
            ]
        );
    }

    #[test]
    fn execution_mode_lane_names_and_suffixes_are_unique() {
        for values in [
            EXECUTION_MODE_LANES.map(|lane| lane.name()),
            EXECUTION_MODE_LANES.map(|lane| lane.o3_trace_stat_suffix()),
            EXECUTION_MODE_LANES.map(|lane| lane.o3_checkpoint_restore_trace_stat_suffix()),
        ] {
            let expected_len = values.len();
            assert_eq!(
                values.into_iter().collect::<BTreeSet<_>>().len(),
                expected_len
            );
        }
    }

    #[test]
    fn execution_mode_names_round_trip_and_index_in_descriptor_order() {
        for (index, mode) in [
            ExecutionMode::Functional,
            ExecutionMode::Timing,
            ExecutionMode::Detailed,
        ]
        .into_iter()
        .enumerate()
        {
            let name = execution_mode_name(mode);
            assert_eq!(execution_mode_from_name(name), Some(mode));
            assert_eq!(execution_mode_lane_index(name), Some(index));
        }
        assert_eq!(execution_mode_from_name("unknown"), None);
        assert_eq!(execution_mode_lane_index("unknown"), None);
    }
}
