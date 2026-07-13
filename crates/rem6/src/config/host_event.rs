use rem6_system::{ExecutionMode, ExecutionModeTarget};

use crate::execution_mode_lanes::execution_mode_from_name as parse_execution_mode;
use crate::Rem6CliError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceReplayHostEventSpec {
    tick: u64,
    label: String,
}

impl TraceReplayHostEventSpec {
    pub(crate) fn new(tick: u64, label: impl Into<String>) -> Self {
        Self {
            tick,
            label: label.into(),
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunHostExecutionModeSwitchSpec {
    tick: u64,
    cpu: usize,
    target: ExecutionModeTarget,
    mode: ExecutionMode,
}

impl RunHostExecutionModeSwitchSpec {
    pub(crate) const fn tick(&self) -> u64 {
        self.tick
    }

    pub(crate) const fn cpu(&self) -> usize {
        self.cpu
    }

    pub(crate) const fn target(&self) -> &ExecutionModeTarget {
        &self.target
    }

    pub(crate) const fn mode(&self) -> ExecutionMode {
        self.mode
    }
}

pub(super) fn run_host_events_from_file(
    values: Option<&[String]>,
) -> Result<Vec<TraceReplayHostEventSpec>, Rem6CliError> {
    values
        .unwrap_or_default()
        .iter()
        .map(|value| parse_run_host_event(value))
        .collect()
}

pub(super) fn parse_run_host_event(value: &str) -> Result<TraceReplayHostEventSpec, Rem6CliError> {
    let Some((tick, label)) = value.split_once(':') else {
        return Err(Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        });
    };
    let tick = tick
        .parse::<u64>()
        .map_err(|_| Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        })?;
    if label.is_empty() {
        return Err(Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        });
    }
    Ok(TraceReplayHostEventSpec::new(tick, label))
}

pub(super) fn run_host_execution_mode_switches_from_file(
    values: Option<&[String]>,
) -> Result<Vec<RunHostExecutionModeSwitchSpec>, Rem6CliError> {
    values
        .unwrap_or_default()
        .iter()
        .map(|value| parse_run_host_execution_mode_switch(value))
        .collect()
}

pub(super) fn parse_run_host_execution_mode_switch(
    value: &str,
) -> Result<RunHostExecutionModeSwitchSpec, Rem6CliError> {
    let mut fields = value.split(':');
    let (Some(tick), Some(target), Some(mode), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(invalid_execution_mode_switch(value));
    };
    let tick = tick
        .parse::<u64>()
        .map_err(|_| invalid_execution_mode_switch(value))?;
    let cpu = target
        .strip_prefix("cpu")
        .and_then(|cpu| cpu.parse::<usize>().ok())
        .filter(|cpu| target == format!("cpu{cpu}"))
        .ok_or_else(|| invalid_execution_mode_switch(value))?;
    let mode = parse_execution_mode(mode).ok_or_else(|| invalid_execution_mode_switch(value))?;
    Ok(RunHostExecutionModeSwitchSpec {
        tick,
        cpu,
        target: ExecutionModeTarget::new(target),
        mode,
    })
}

fn invalid_execution_mode_switch(value: &str) -> Rem6CliError {
    Rem6CliError::InvalidRunHostExecutionModeSwitch {
        value: value.to_string(),
    }
}
