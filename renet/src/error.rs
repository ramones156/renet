use crate::{reassembly_fragment::FragmentError, packet::PacketError};

use std::fmt;
use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisconnectionReason {
    /// Server has exceeded maximum players capacity
    MaxConnections,
    TimedOut,
    DisconnectedByServer,
    DisconnectedByClient,
    ClientAlreadyConnected,
    InvalidChannelId(u8),
    MismatchingChannelType(u8),
    ReliableChannelOutOfSync(u8),
}
impl DisconnectionReason {
    pub fn id(&self) -> u8 {
        use DisconnectionReason::*;

        match self {
            MaxConnections => 0,
            TimedOut => 1,
            DisconnectedByServer => 2,
            DisconnectedByClient => 3,
            ClientAlreadyConnected => 4,
            InvalidChannelId(_) => 5,
            MismatchingChannelType(_) => 6,
            ReliableChannelOutOfSync(_) => 7,
        }
    }

    pub fn try_from(id: u8, channel_id: u8) -> Option<DisconnectionReason> {
        use DisconnectionReason::*;

        let reason = match id {
            0 => MaxConnections,
            1 => TimedOut,
            2 => DisconnectedByServer,
            3 => DisconnectedByClient,
            4 => ClientAlreadyConnected,
            5 => InvalidChannelId(channel_id),
            6 => MismatchingChannelType(channel_id),
            7 => ReliableChannelOutOfSync(channel_id),
            _ => return None
        };
        Some(reason)
    }
}

impl fmt::Display for DisconnectionReason {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use DisconnectionReason::*;

        match *self {
            MaxConnections => write!(fmt, "server has reached the limit of connections"),
            TimedOut => write!(fmt, "connection has timed out"),
            DisconnectedByServer => write!(fmt, "connection terminated by server"),
            DisconnectedByClient => write!(fmt, "connection terminated by client"),
            ClientAlreadyConnected => write!(fmt, "connection with same id alredy exists"),
            InvalidChannelId(id) => write!(fmt, "received message with invalid channel {}", id),
            MismatchingChannelType(id) => write!(fmt, "received message from channel {} with mismatching channel type", id),
            ReliableChannelOutOfSync(id) => write!(fmt, "reliable channel {} is out of sync", id),
        }
    }
}

// Error message not sent
#[derive(Debug)]
pub enum RenetError {
    MessageSizeAboveLimit,
    ChannelMaxMessagesLimit,
    AlreadySendingBlockMessage,
    ClientDisconnected(DisconnectionReason),
    ClientNotFound,
    FragmentError(FragmentError),
    PacketError(PacketError),
    IoError(io::Error),
}

impl std::error::Error for RenetError {}

impl fmt::Display for RenetError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use RenetError::*;

        match *self {
            MessageSizeAboveLimit => write!(fmt, "the message is above the limit size"),
            ChannelMaxMessagesLimit => write!(fmt, "the channel has reached the maximum messages"),
            AlreadySendingBlockMessage => {
                write!(fmt, "the connection is already sending a block message")
            }
            ClientNotFound => write!(fmt, "client with given id was not found"),
            ClientDisconnected(reason) => write!(fmt, "client is disconnected: {}", reason),
            PacketError(ref packet_err) => write!(fmt, "{}", packet_err),
            FragmentError(ref fragment_err) => write!(fmt, "{}", fragment_err),
            IoError(ref io_err) => write!(fmt, "{}", io_err),
        }
    }
}

impl From<PacketError> for RenetError {
    fn from(inner: PacketError) -> Self {
        RenetError::PacketError(inner)
    }
}

impl From<FragmentError> for RenetError {
    fn from(inner: FragmentError) -> Self {
        RenetError::FragmentError(inner)
    }
}

impl From<io::Error> for RenetError {
    fn from(inner: io::Error) -> Self {
        RenetError::IoError(inner)
    }
}
