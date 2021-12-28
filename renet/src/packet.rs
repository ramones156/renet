use crate::channel::block::SliceMessage;
use crate::channel::reliable::ReliableMessage;
use crate::error::DisconnectionReason;

use bincode::Options;
use serde::{Deserialize, Serialize};

pub type Payload = Vec<u8>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReliableChannelData {
    pub channel_id: u8,
    pub messages: Vec<ReliableMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct UnreliableChannelData {
    pub channel_id: u8,
    pub messages: Vec<Payload>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BlockChannelData {
    pub channel_id: u8,
    pub messages: Vec<SliceMessage>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct AckData {
    pub ack: u16,
    pub ack_bits: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum Packet {
    Normal(Normal),
    Fragment(Fragment),
    Heartbeat(HeartBeat),
    Disconnect(DisconnectionReason),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ChannelMessages {
    pub block_channels_data: Vec<BlockChannelData>,
    pub unreliable_channels_data: Vec<UnreliableChannelData>,
    pub reliable_channels_data: Vec<ReliableChannelData>,
}

impl ChannelMessages {
    pub fn is_empty(&self) -> bool {
        self.block_channels_data.is_empty() && self.unreliable_channels_data.is_empty() && self.reliable_channels_data.is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Normal {
    pub sequence: u16,
    pub ack_data: AckData,
    pub channel_messages: ChannelMessages,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Fragment {
    pub ack_data: AckData,
    pub sequence: u16,
    pub fragment_id: u8,
    pub num_fragments: u8,
    pub payload: Payload,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct HeartBeat {
    pub ack_data: AckData,
}

impl From<Normal> for Packet {
    fn from(value: Normal) -> Self {
        Self::Normal(value)
    }
}

impl From<Fragment> for Packet {
    fn from(value: Fragment) -> Self {
        Self::Fragment(value)
    }
}

impl From<HeartBeat> for Packet {
    fn from(value: HeartBeat) -> Self {
        Self::Heartbeat(value)
    }
}

impl From<DisconnectionReason> for Packet {
    fn from(value: DisconnectionReason) -> Self {
        Self::Disconnect(value)
    }
}

pub fn disconnect_packet(reason: DisconnectionReason) -> Result<Payload, bincode::Error> {
    let packet = Packet::Disconnect(reason);
    let packet = bincode::options().serialize(&packet)?;
    Ok(packet)
}

use std::io;

impl Packet {
    fn serialize_size(&self) -> usize {
        match self {
            Packet::Normal(Normal { channel_messages, .. }) => {
                let mut size = 0;
                size += 1; // packet_type
                size += 2; // sequence
                size += 6; // ack_data
                
                size += 1; // channel_size
                for data in channel_messages.reliable_channels_data.iter() {
                    size += 1; // channel_id
                    size += 2; // message_len
                    for message in data.messages.iter() {
                        size += 2; // id
                        size += 2; // message_size
                        size += message.payload.len();
                    }
                }

                size += 1; // channel_size
                for data in channel_messages.unreliable_channels_data.iter() {
                    size += 1; // channel_id
                    size += 2; // message_len
                    for message in data.messages.iter() {
                        size += 2; // message_size
                        size += message.len();
                    }
                }

                size += 1; // channel_size
                for data in channel_messages.block_channels_data.iter() {
                    size += 1; // channel_id
                    size += 2; // message_len
                    for message in data.messages.iter() {
                        size += 2; // chunk_id
                        size += 4; // slice_id
                        size += 4; // num_slices
                        size += 2; // message_size
                        size += message.payload.len();
                    }
                }
                size
            }
            _ => 0,
        }
    }
    fn write(&self, out: &mut impl io::Write) -> Result<(), io::Error> {
        match self {
            Packet::Normal(Normal {
                sequence,
                ack_data,
                channel_messages,
            }) => {
                out.write(&[0])?;
                out.write(&sequence.to_le_bytes())?;
                out.write(&ack_data.ack.to_le_bytes())?;
                out.write(&ack_data.ack_bits.to_le_bytes())?;

                let reliable_channel_len = channel_messages.reliable_channels_data.len() as u8;
                out.write(&reliable_channel_len.to_le_bytes())?;
                for data in channel_messages.reliable_channels_data.iter() {
                    out.write(&data.channel_id.to_le_bytes())?;
                    // TODO: limit size to u16::MAX
                    let messages_len = data.messages.len() as u16;
                    out.write(&messages_len.to_le_bytes())?;
                    for message in data.messages.iter() {
                        out.write(&message.id.to_le_bytes())?;
                        // TODO: limit size to u16::MAX
                        let size = message.payload.len() as u16;
                        out.write(&size.to_le_bytes())?;
                        out.write_all(&message.payload)?;
                    }
                }

                let unreliable_channel_len = channel_messages.unreliable_channels_data.len() as u8;
                out.write(&unreliable_channel_len.to_le_bytes())?;
                for data in channel_messages.unreliable_channels_data.iter() {
                    out.write(&data.channel_id.to_le_bytes())?;
                    // TODO: limit size to u16::MAX
                    let messages_len = data.messages.len() as u16;
                    out.write(&messages_len.to_le_bytes())?;
                    for message in data.messages.iter() {
                        // TODO: limit size to u16::MAX
                        let size = message.len() as u16;
                        out.write(&size.to_le_bytes())?;
                        out.write_all(&message)?;
                    }
                }

                let block_channel_len = channel_messages.block_channels_data.len() as u8;
                out.write(&block_channel_len.to_le_bytes())?;
                for data in channel_messages.block_channels_data.iter() {
                    out.write(&data.channel_id.to_le_bytes())?;
                    // TODO: limit size to u16::MAX
                    let messages_len = data.messages.len() as u16;
                    out.write(&messages_len.to_le_bytes())?;
                    for message in data.messages.iter() {
                        out.write(&message.chunk_id.to_le_bytes())?;
                        out.write(&message.slice_id.to_le_bytes())?;
                        out.write(&message.num_slices.to_le_bytes())?;
                        // TODO: limit size to u16::MAX
                        let size = message.payload.len() as u16;
                        out.write(&size.to_le_bytes())?;
                        out.write_all(&message.payload)?;
                    }
                }
            }
            Packet::Fragment(Fragment {
                sequence,
                ack_data,
                fragment_id,
                num_fragments,
                payload,
            }) => {
                out.write(&[1])?;
                out.write(&sequence.to_le_bytes())?;
                out.write(&ack_data.ack.to_le_bytes())?;
                out.write(&ack_data.ack_bits.to_le_bytes())?;
                out.write(&fragment_id.to_le_bytes())?;
                out.write(&num_fragments.to_le_bytes())?;
                let size = payload.len() as u16;
                out.write(&size.to_le_bytes())?;
                out.write_all(&payload)?;
            }
            Packet::Heartbeat(HeartBeat { ack_data }) => {
                out.write(&[2])?;
                out.write(&ack_data.ack.to_le_bytes())?;
                out.write(&ack_data.ack_bits.to_le_bytes())?;
            }
            Packet::Disconnect(reason) => {
                out.write(&[3])?;
                out.write(&reason.id().to_le_bytes())?;
                match reason {
                    DisconnectionReason::InvalidChannelId(channel_id)
                    | DisconnectionReason::MismatchingChannelType(channel_id)
                    | DisconnectionReason::ReliableChannelOutOfSync(channel_id) => {
                        out.write(&channel_id.to_le_bytes())?;
                    }
                    _ => {
                        out.write(&[0])?;
                    }
                };
            }
        }

        Ok(())
    }

    fn read(src: &mut impl io::Read) -> Result<Self, PacketError> {
        let packet_type = read_u8(src)?;
        match packet_type {
            0 => {
                // Normal
                let sequence = read_u16(src)?;
                let ack = read_u16(src)?;
                let ack_bits = read_u32(src)?;
                let ack_data = AckData { ack, ack_bits };

                let reliable_channel_len = read_u8(src)? as usize;
                let mut reliable_channels_data = Vec::with_capacity(reliable_channel_len);
                for _ in 0..reliable_channel_len {
                    let channel_id = read_u8(src)?;
                    let messages_len = read_u16(src)? as usize;
                    let mut messages = Vec::with_capacity(messages_len);
                    for _ in 0..messages_len {
                        let id = read_u16(src)?;
                        let size = read_u16(src)? as usize;
                        let mut payload = vec![0; size];
                        src.read_exact(&mut payload)?;
                        messages.push(ReliableMessage { id, payload });
                    }
                    reliable_channels_data.push(ReliableChannelData { channel_id, messages });
                }

                let unreliable_channel_len = read_u8(src)? as usize;
                let mut unreliable_channels_data = Vec::with_capacity(unreliable_channel_len);
                for _ in 0..unreliable_channel_len {
                    let channel_id = read_u8(src)?;
                    let messages_len = read_u16(src)? as usize;
                    let mut messages = Vec::with_capacity(messages_len);
                    for _ in 0..messages_len {
                        let size = read_u16(src)? as usize;
                        let mut payload = vec![0; size];
                        src.read_exact(&mut payload)?;
                        messages.push(payload);
                    }
                    unreliable_channels_data.push(UnreliableChannelData { channel_id, messages });
                }

                let block_channel_len = read_u8(src)? as usize;
                let mut block_channels_data = Vec::with_capacity(block_channel_len);
                for _ in 0..block_channel_len {
                    let channel_id = read_u8(src)?;
                    let messages_len = read_u16(src)? as usize;
                    let mut messages = Vec::with_capacity(messages_len);
                    for _ in 0..messages_len {
                        let chunk_id = read_u16(src)?;
                        let slice_id = read_u32(src)?;
                        let num_slices = read_u32(src)?;
                        let size = read_u16(src)? as usize;
                        let mut payload = vec![0; size];
                        src.read_exact(&mut payload)?;
                        messages.push(SliceMessage {
                            chunk_id,
                            slice_id,
                            num_slices,
                            payload,
                        });
                    }
                    block_channels_data.push(BlockChannelData { channel_id, messages });
                }

                let channel_messages = ChannelMessages {
                    reliable_channels_data,
                    unreliable_channels_data,
                    block_channels_data,
                };

                Ok(Packet::Normal(Normal {
                    sequence,
                    ack_data,
                    channel_messages,
                }))
            }
            1 => {
                let sequence = read_u16(src)?;
                let ack = read_u16(src)?;
                let ack_bits = read_u32(src)?;
                let ack_data = AckData { ack, ack_bits };
                let fragment_id = read_u8(src)?;
                let num_fragments = read_u8(src)?;
                let size = read_u16(src)? as usize;
                let mut payload = vec![0; size];
                src.read_exact(&mut payload)?;

                Ok(Packet::Fragment(Fragment {
                    sequence,
                    ack_data,
                    fragment_id,
                    num_fragments,
                    payload,
                }))
            }
            2 => {
                let ack = read_u16(src)?;
                let ack_bits = read_u32(src)?;
                let ack_data = AckData { ack, ack_bits };
                Ok(Packet::Heartbeat(HeartBeat { ack_data }))
            }
            3 => {
                let reason_id = read_u8(src)?;
                let channel_id = read_u8(src)?;
                match DisconnectionReason::try_from(reason_id, channel_id) {
                    Some(reason) => Ok(Packet::Disconnect(reason)),
                    None => Err(PacketError::InvalidDisconnectReason),
                }
            }
            _ => Err(PacketError::InvalidPacketType),
        }
    }
}

use std::error;
use std::fmt;

#[derive(Debug)]
enum PacketError {
    InvalidPacketType,
    InvalidDisconnectReason,
    IoError(io::Error),
}

impl error::Error for PacketError {}

impl fmt::Display for PacketError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use PacketError::*;

        match *self {
            InvalidPacketType => write!(fmt, "invalid packet type"),
            InvalidDisconnectReason => write!(fmt, "invalid disconnect reason id"),
            IoError(ref io_err) => write!(fmt, "{}", io_err),
        }
    }
}

impl From<io::Error> for PacketError {
    fn from(inner: io::Error) -> Self {
        PacketError::IoError(inner)
    }
}

#[inline]
fn read_u64(src: &mut impl io::Read) -> Result<u64, io::Error> {
    let mut buffer = [0u8; 8];
    src.read_exact(&mut buffer)?;
    Ok(u64::from_le_bytes(buffer))
}

#[inline]
fn read_u32(src: &mut impl io::Read) -> Result<u32, io::Error> {
    let mut buffer = [0u8; 4];
    src.read_exact(&mut buffer)?;
    Ok(u32::from_le_bytes(buffer))
}

#[inline]
fn read_u16(src: &mut impl io::Read) -> Result<u16, io::Error> {
    let mut buffer = [0u8; 2];
    src.read_exact(&mut buffer)?;
    Ok(u16::from_le_bytes(buffer))
}

#[inline]
fn read_u8(src: &mut impl io::Read) -> Result<u8, io::Error> {
    let mut buffer = [0u8; 1];
    src.read_exact(&mut buffer)?;
    Ok(u8::from_le_bytes(buffer))
}

#[inline]
fn read_bytes<const N: usize>(src: &mut impl io::Read) -> Result<[u8; N], io::Error> {
    let mut data = [0u8; N];
    src.read_exact(&mut data)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reliable(id: u16) -> ReliableMessage {
        let i = id as u8;
        ReliableMessage {
            id,
            payload: vec![i; i as usize],
        }
    }

    #[test]
    fn packet_normal_serialize() {
        let reliable_channels_data = vec![
            ReliableChannelData {
                channel_id: 0,
                messages: vec![reliable(0), reliable(1)],
            },
            ReliableChannelData {
                channel_id: 1,
                messages: vec![reliable(2), reliable(3)],
            },
        ];
        let unreliable_channels_data = vec![
            UnreliableChannelData {
                channel_id: 2,
                messages: vec![vec![1]],
            },
            UnreliableChannelData {
                channel_id: 3,
                messages: vec![vec![2]],
            },
            UnreliableChannelData {
                channel_id: 4,
                messages: vec![vec![3]],
            },
        ];
        let block_channels_data = vec![BlockChannelData {
            channel_id: 5,
            messages: vec![SliceMessage {
                slice_id: 0,
                num_slices: 4,
                chunk_id: 0,
                payload: vec![1, 2, 3],
            }],
        }];

        let channel_messages = ChannelMessages {
            reliable_channels_data,
            unreliable_channels_data,
            block_channels_data,
        };
        let ack_data = AckData { ack_bits: 3, ack: 0 };
        let normal_packet = Packet::Normal(Normal {
            sequence: 7,
            ack_data,
            channel_messages,
        });

        let mut buffer: Vec<u8> = vec![];
        normal_packet.write(&mut buffer).unwrap();
        assert_eq!(normal_packet.serialize_size(), buffer.len());

        let result = Packet::read(&mut buffer.as_slice()).unwrap();
        assert_eq!(normal_packet, result);
    }

    #[test]
    fn packet_fragment_serialize() {
        let fragment_packet = Packet::Fragment(Fragment {
            sequence: 0,
            ack_data: AckData { ack: 0, ack_bits: 1 },
            fragment_id: 1,
            num_fragments: 8,
            payload: vec![7, 7, 7, 7],
        });

        let mut buffer: Vec<u8> = vec![];
        fragment_packet.write(&mut buffer).unwrap();

        let result = Packet::read(&mut buffer.as_slice()).unwrap();
        assert_eq!(fragment_packet, result);
    }

    #[test]
    fn packet_heartbeat_serialize() {
        let heartbeat_packet = Packet::Heartbeat(HeartBeat {
            ack_data: AckData { ack: 3, ack_bits: 7 },
        });

        let mut buffer: Vec<u8> = vec![];
        heartbeat_packet.write(&mut buffer).unwrap();

        let result = Packet::read(&mut buffer.as_slice()).unwrap();
        assert_eq!(heartbeat_packet, result);
    }

    #[test]
    fn packet_disconnect_serialize() {
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::MaxConnections);
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::TimedOut);
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::DisconnectedByServer);
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::DisconnectedByClient);
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::ClientAlreadyConnected);
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::InvalidChannelId(1));
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::MismatchingChannelType(2));
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
        {
            let disconnect_packet = Packet::Disconnect(DisconnectionReason::ReliableChannelOutOfSync(3));
            let mut buffer: Vec<u8> = vec![];
            disconnect_packet.write(&mut buffer).unwrap();
            let result = Packet::read(&mut buffer.as_slice()).unwrap();
            assert_eq!(disconnect_packet, result);
        }
    }
}

