use crate::{
    GdbRemoteResumeKind, GdbRemoteResumeRequest, GdbRemoteTrapOperation, GdbRemoteTrapPoint,
    GdbRemoteTrapRequest,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteExecutionControl {
    state: GdbRemoteControlState,
    active_traps: Vec<GdbRemoteTrapPoint>,
}

impl GdbRemoteExecutionControl {
    pub fn new(state: GdbRemoteControlState, active_traps: Vec<GdbRemoteTrapPoint>) -> Self {
        Self {
            state,
            active_traps,
        }
    }

    pub const fn state(&self) -> &GdbRemoteControlState {
        &self.state
    }

    pub fn active_traps(&self) -> &[GdbRemoteTrapPoint] {
        &self.active_traps
    }

    pub(crate) fn set_stopped(&mut self) {
        self.state = GdbRemoteControlState::Stopped;
    }

    pub(crate) fn set_interrupted(&mut self) {
        self.state = GdbRemoteControlState::Interrupted;
    }

    pub(crate) fn set_disconnected(&mut self) {
        self.state = GdbRemoteControlState::Disconnected;
    }

    pub(crate) fn apply_resume_requests(&mut self, requests: Vec<GdbRemoteResumeRequest>) {
        self.state = GdbRemoteControlState::from_resume_requests(requests);
    }

    pub(crate) fn apply_trap_request(&mut self, request: GdbRemoteTrapRequest) {
        let point = request.point();
        match request.operation() {
            GdbRemoteTrapOperation::Insert => {
                if !self.active_traps.contains(&point) {
                    self.active_traps.push(point);
                }
            }
            GdbRemoteTrapOperation::Remove => {
                self.active_traps.retain(|active| *active != point);
            }
        }
    }
}

impl Default for GdbRemoteExecutionControl {
    fn default() -> Self {
        Self {
            state: GdbRemoteControlState::Stopped,
            active_traps: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteControlState {
    Stopped,
    Continue {
        requests: Vec<GdbRemoteResumeRequest>,
    },
    SingleInstruction {
        requests: Vec<GdbRemoteResumeRequest>,
    },
    Interrupted,
    Disconnected,
}

impl GdbRemoteControlState {
    pub(crate) fn from_resume_requests(requests: Vec<GdbRemoteResumeRequest>) -> Self {
        if requests.is_empty() {
            return Self::Stopped;
        }
        if requests
            .iter()
            .all(|request| request.kind() == GdbRemoteResumeKind::SingleInstruction)
        {
            return Self::SingleInstruction { requests };
        }
        Self::Continue { requests }
    }
}
