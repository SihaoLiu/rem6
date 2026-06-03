mod error;
mod linear;

pub use error::TrafficGeneratorError;
pub use linear::{
    LinearTrafficGenerator, TrafficGeneratorSummary, TrafficLinearConfig, TrafficLinearSnapshot,
    TrafficRequestEvent, TrafficRequestKind,
};
