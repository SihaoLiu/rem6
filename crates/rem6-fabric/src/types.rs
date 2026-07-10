use std::error::Error;
use std::fmt;

use rem6_kernel::ClockError;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FabricLinkId(String);

impl FabricLinkId {
    pub fn new(value: impl Into<String>) -> Result<Self, FabricError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FabricError::EmptyLinkId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FabricRouterId(String);

impl FabricRouterId {
    pub fn new(value: impl Into<String>) -> Result<Self, FabricError> {
        let value = value.into();
        if value.is_empty() {
            return Err(FabricError::EmptyRouterId);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FabricPacketId(u64);

impl FabricPacketId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtualNetworkId(u16);

impl VirtualNetworkId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FabricError {
    EmptyLinkId,
    EmptyRouterId,
    EmptyPath,
    ZeroPacketBytes,
    ZeroLinkLatency,
    ZeroLinkBandwidth,
    ZeroRouterLatency,
    ZeroCreditDepth,
    ZeroSerialLinkLanes,
    ZeroSerialLinkLaneSpeed,
    ZeroSerialLinkTicksPerNanosecond,
    SerialLinkBandwidthOverflow {
        lanes: u64,
        lane_speed_bits_per_cycle: u64,
    },
    SerialLinkNanosecondBandwidthOverflow {
        lanes: u64,
        lane_speed_bits_per_nanosecond: u64,
    },
    SerialLinkPacketBitOverflow {
        bytes: u64,
    },
    SerialLinkNanosecondTickOverflow {
        nanoseconds: u64,
        ticks_per_nanosecond: u64,
    },
    Clock(ClockError),
    DuplicatePacketInBatch {
        packet: FabricPacketId,
    },
    DuplicateLaneSnapshot {
        link: FabricLinkId,
        virtual_network: VirtualNetworkId,
    },
    DuplicateRouterInputVcSnapshot {
        router: FabricRouterId,
        input_port: u32,
        virtual_channel: u16,
    },
    DuplicateRouterOutputPortSnapshot {
        router: FabricRouterId,
        output_port: u32,
    },
    QosNoGrant,
    TickOverflow,
}

impl fmt::Display for FabricError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLinkId => write!(formatter, "fabric link id must not be empty"),
            Self::EmptyRouterId => write!(formatter, "fabric router id must not be empty"),
            Self::EmptyPath => write!(formatter, "fabric path must contain a hop"),
            Self::ZeroPacketBytes => write!(formatter, "fabric packet must contain bytes"),
            Self::ZeroLinkLatency => write!(formatter, "fabric link latency must be positive"),
            Self::ZeroLinkBandwidth => write!(formatter, "fabric link bandwidth must be positive"),
            Self::ZeroRouterLatency => write!(formatter, "fabric router latency must be positive"),
            Self::ZeroCreditDepth => write!(formatter, "fabric credit depth must be positive"),
            Self::ZeroSerialLinkLanes => {
                write!(formatter, "fabric serial link lane count must be positive")
            }
            Self::ZeroSerialLinkLaneSpeed => {
                write!(formatter, "fabric serial link lane speed must be positive")
            }
            Self::ZeroSerialLinkTicksPerNanosecond => write!(
                formatter,
                "fabric serial link nanosecond timebase must be positive"
            ),
            Self::SerialLinkBandwidthOverflow {
                lanes,
                lane_speed_bits_per_cycle,
            } => write!(
                formatter,
                "fabric serial link bandwidth overflows for {lanes} lanes at {lane_speed_bits_per_cycle} bits per cycle"
            ),
            Self::SerialLinkNanosecondBandwidthOverflow {
                lanes,
                lane_speed_bits_per_nanosecond,
            } => write!(
                formatter,
                "fabric serial link bandwidth overflows for {lanes} lanes at {lane_speed_bits_per_nanosecond} bits per nanosecond"
            ),
            Self::SerialLinkPacketBitOverflow { bytes } => write!(
                formatter,
                "fabric serial link packet bit count overflows for {bytes} bytes"
            ),
            Self::SerialLinkNanosecondTickOverflow {
                nanoseconds,
                ticks_per_nanosecond,
            } => write!(
                formatter,
                "fabric serial link {nanoseconds} nanoseconds overflows at {ticks_per_nanosecond} ticks per nanosecond"
            ),
            Self::Clock(error) => write!(formatter, "{error}"),
            Self::DuplicatePacketInBatch { packet } => {
                write!(formatter, "packet {} appears more than once", packet.get())
            }
            Self::DuplicateLaneSnapshot {
                link,
                virtual_network,
            } => write!(
                formatter,
                "fabric lane {} virtual network {} appears more than once in snapshot",
                link.as_str(),
                virtual_network.get()
            ),
            Self::DuplicateRouterInputVcSnapshot {
                router,
                input_port,
                virtual_channel,
            } => write!(
                formatter,
                "fabric router {} input port {input_port} virtual channel {virtual_channel} appears more than once in snapshot",
                router.as_str()
            ),
            Self::DuplicateRouterOutputPortSnapshot {
                router,
                output_port,
            } => write!(
                formatter,
                "fabric router {} output port {output_port} appears more than once in snapshot",
                router.as_str()
            ),
            Self::QosNoGrant => write!(formatter, "QoS arbiter did not select a queued request"),
            Self::TickOverflow => write!(formatter, "fabric tick calculation overflowed"),
        }
    }
}

impl Error for FabricError {}

impl From<ClockError> for FabricError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricPacket {
    id: FabricPacketId,
    bytes: u64,
    virtual_network: VirtualNetworkId,
}

impl FabricPacket {
    pub fn new(
        id: FabricPacketId,
        bytes: u64,
        virtual_network: VirtualNetworkId,
    ) -> Result<Self, FabricError> {
        if bytes == 0 {
            return Err(FabricError::ZeroPacketBytes);
        }

        Ok(Self {
            id,
            bytes,
            virtual_network,
        })
    }

    pub const fn id(&self) -> FabricPacketId {
        self.id
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn virtual_network(&self) -> VirtualNetworkId {
        self.virtual_network
    }
}
