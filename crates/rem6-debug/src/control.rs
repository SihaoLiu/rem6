use crate::{GdbRemoteResumeKind, GdbRemoteResumeRequest};

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
