use crate::channel::block::SliceMessage;
use crate::channel::reliable::ReliableMessage;
use crate::error::DisconnectionReason;

use std::io::{Read, Write};

pub type Payload = Vec<u8>;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct ReliableChannelData {
    pub channel_id: u8,
    pub messages: Vec<ReliableMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct UnreliableChannelData {
    pub channel_id: u8,
    pub messages: Vec<Payload>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct BlockChannelData {
    pub channel_id: u8,
    pub messages: Vec<SliceMessage>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct AckData {
    pub ack: u16,
    pub ack_bits: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Packet {
    Normal(Normal),
    Fragment(Fragment),
    Heartbeat(HeartBeat),
    Disconnect(DisconnectionReason),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct Normal {
    pub sequence: u16,
    pub ack_data: AckData,
    pub channel_messages: ChannelMessages,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub(crate) struct Fragment {
    pub ack_data: AckData,
    pub sequence: u16,
    pub fragment_id: u8,
    pub num_fragments: u8,
    pub payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
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

pub fn disconnect_packet(reason: DisconnectionReason) -> Result<Payload, PacketError> {
    let mut packet = Packet::Disconnect(reason);
    packet.serialize()
}

use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum PacketError {
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

impl Serialize for HeartBeat {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u16(&mut self.ack_data.ack)?;
        s.serialize_u32(&mut self.ack_data.ack_bits)?;
        Ok(())
    }
}

impl DisconnectionReason {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), PacketError> {
        let mut reason_id = self.id();
        s.serialize_u8(&mut reason_id)?;
        let mut channel_id = 0;
        s.serialize_u8(&mut channel_id)?;
        match DisconnectionReason::try_from(reason_id, channel_id) {
            Some(reason) => *self = reason,
            None => return Err(PacketError::InvalidDisconnectReason),
        }
        Ok(())
    }
}

impl Serialize for Fragment {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u16(&mut self.sequence)?;
        s.serialize_u16(&mut self.ack_data.ack)?;
        s.serialize_u32(&mut self.ack_data.ack_bits)?;
        s.serialize_u8(&mut self.fragment_id)?;
        s.serialize_u8(&mut self.num_fragments)?;
        s.serialize_vec_len_as_u16(&mut self.payload)?;
        Ok(())
    }
}

impl Serialize for ReliableChannelData {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u8(&mut self.channel_id)?;
        let mut messages_len = self.messages.len() as u16;
        s.serialize_u16(&mut messages_len)?;
        if s.is_reading() {
            self.messages.resize(messages_len as usize, Default::default());
        }
        for message in self.messages.iter_mut() {
            message.serialize_stream(s)?;
        }
        Ok(())
    }
}

impl Serialize for UnreliableChannelData {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u8(&mut self.channel_id)?;
        let mut messages_len = self.messages.len() as u16;
        s.serialize_u16(&mut messages_len)?;
        if s.is_reading() {
            self.messages.resize(messages_len as usize, Default::default());
        }
        for mut message in self.messages.iter_mut() {
            s.serialize_vec_len_as_u16(&mut message)?;
        }
        Ok(())
    }
}

impl Serialize for BlockChannelData {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u8(&mut self.channel_id)?;
        let mut messages_len = self.messages.len() as u16;
        s.serialize_u16(&mut messages_len)?;
        if s.is_reading() {
            self.messages.resize(messages_len as usize, Default::default());
        }
        for message in self.messages.iter_mut() {
            message.serialize_stream(s)?;
        }
        Ok(())
    }
}

impl Serialize for ChannelMessages {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        let Self {
            reliable_channels_data,
            unreliable_channels_data,
            block_channels_data,
        } = self;

        let mut reliable_data_len = reliable_channels_data.len() as u8;
        s.serialize_u8(&mut reliable_data_len)?;
        if s.is_reading() {
            self.reliable_channels_data.resize(reliable_data_len as usize, Default::default());
        }

        for data in self.reliable_channels_data.iter_mut() {
            data.serialize_stream(s)?;
        }

        let mut unreliable_data_len = unreliable_channels_data.len() as u8;
        s.serialize_u8(&mut unreliable_data_len)?;
        if s.is_reading() {
            self.unreliable_channels_data
                .resize(unreliable_data_len as usize, Default::default());
        }
        for data in self.unreliable_channels_data.iter_mut() {
            data.serialize_stream(s)?;
        }

        let mut block_data_len = block_channels_data.len() as u8;
        s.serialize_u8(&mut block_data_len)?;
        if s.is_reading() {
            self.block_channels_data.resize(block_data_len as usize, Default::default());
        }
        for data in self.block_channels_data.iter_mut() {
            data.serialize_stream(s)?;
        }

        Ok(())
    }
}

impl Serialize for Normal {
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error> {
        s.serialize_u16(&mut self.sequence)?;
        s.serialize_u16(&mut self.ack_data.ack)?;
        s.serialize_u32(&mut self.ack_data.ack_bits)?;
        self.channel_messages.serialize_stream(s)?;

        Ok(())
    }
}

impl Packet {
    pub fn serialize(&mut self) -> Result<Vec<u8>, PacketError> {
        let mut buffer = vec![];
        let mut write_stream = WriteStream::new(&mut buffer);
        let mut packet_type: u8 = match self {
            Packet::Normal(_) => 0,
            Packet::Fragment(_) => 1,
            Packet::Heartbeat(_) => 2,
            Packet::Disconnect(_) => 3,
        };
        write_stream.serialize_u8(&mut packet_type)?;
        self.serialize_stream(&mut write_stream)?;
        Ok(buffer)
    }

    pub fn deserialize(buffer: &[u8]) -> Result<Self, PacketError> {
        let packet_type = buffer[0];
        let mut read_stream = ReadStream::new(&buffer[1..]);
        let mut packet = match packet_type {
            0 => Packet::Normal(Normal::default()),
            1 => Packet::Fragment(Fragment::default()),
            2 => Packet::Heartbeat(HeartBeat::default()),
            3 => Packet::Disconnect(DisconnectionReason::MaxConnections),
            _ => return Err(PacketError::InvalidPacketType),
        };
        packet.serialize_stream(&mut read_stream)?;
        Ok(packet)
    }

    pub fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), PacketError> {
        match self {
            Packet::Normal(inner) => inner.serialize_stream(s)?,
            Packet::Fragment(inner) => inner.serialize_stream(s)?,
            Packet::Heartbeat(inner) => inner.serialize_stream(s)?,
            Packet::Disconnect(inner) => inner.serialize_stream(s)?,
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_stream_serialize() {
        let mut heartbeat = HeartBeat {
            ack_data: AckData { ack_bits: 7, ack: 3 },
        };
        let bytes = heartbeat.serialize().unwrap();
        let result = HeartBeat::deserialize(&bytes).unwrap();

        assert_eq!(heartbeat, result);
        assert_eq!(bytes.len(), heartbeat.serialize_size().unwrap() as usize);

        let mut packet = Packet::Heartbeat(heartbeat);
        let bytes = packet.serialize().unwrap();
        let result = Packet::deserialize(&bytes).unwrap();
        assert_eq!(packet, result);
    }

    #[test]
    fn fragment_stream_serialize() {
        let mut buffer = vec![];
        let mut fargment = Fragment {
            sequence: 3,
            fragment_id: 1,
            num_fragments: 8,
            payload: vec![1, 2, 3, 4, 5, 6],
            ack_data: AckData { ack_bits: 7, ack: 3 },
        };
        {
            let mut write_stream = WriteStream { buffer: &mut buffer };
            fargment.serialize_stream(&mut write_stream).unwrap();
        }

        let mut result = Fragment::default();
        let mut read_stream = ReadStream { buffer: &buffer[..] };
        result.serialize_stream(&mut read_stream).unwrap();

        assert_eq!(fargment, result);
        let mut measure_stream = MeasureStream { size: 0 };
        fargment.serialize_stream(&mut measure_stream).unwrap();
        assert_eq!(measure_stream.size, buffer.len() as u64);
    }

    #[test]
    fn normal_stream_serialize() {
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
        let mut normal = Normal {
            sequence: 7,
            ack_data,
            channel_messages,
        };

        let mut buffer = vec![];
        {
            let mut write_stream = WriteStream { buffer: &mut buffer };
            normal.serialize_stream(&mut write_stream).unwrap();
        }
        let mut result = Normal::default();
        let mut read_stream = ReadStream { buffer: &buffer[..] };
        result.serialize_stream(&mut read_stream).unwrap();

        assert_eq!(normal, result);
        let mut measure_stream = MeasureStream { size: 0 };
        normal.serialize_stream(&mut measure_stream).unwrap();
        assert_eq!(measure_stream.size, buffer.len() as u64);
    }

    #[test]
    fn disconnect_stream_serialize() {
        let mut buffer = vec![];
        let mut disconnect_packet = DisconnectionReason::MaxConnections;
        {
            let mut write_stream = WriteStream { buffer: &mut buffer };
            disconnect_packet.serialize_stream(&mut write_stream).unwrap();
        }
        let mut result = DisconnectionReason::TimedOut;
        let mut read_stream = ReadStream { buffer: &buffer[..] };
        result.serialize_stream(&mut read_stream).unwrap();

        assert_eq!(disconnect_packet, result);
        let mut measure_stream = MeasureStream { size: 0 };
        disconnect_packet.serialize_stream(&mut measure_stream).unwrap();
        assert_eq!(measure_stream.size, buffer.len() as u64);
    }

    fn reliable(id: u16) -> ReliableMessage {
        let i = id as u8;
        ReliableMessage {
            id,
            payload: vec![i; i as usize],
        }
    }
}

pub trait Stream {
    fn is_writing(&self) -> bool;
    fn is_reading(&self) -> bool;
    fn serialize_u8(&mut self, value: &mut u8) -> Result<(), io::Error>;
    fn serialize_u16(&mut self, value: &mut u16) -> Result<(), io::Error>;
    fn serialize_u32(&mut self, value: &mut u32) -> Result<(), io::Error>;
    fn serialize_u64(&mut self, value: &mut u64) -> Result<(), io::Error>;
    fn serialize_bytes(&mut self, value: &mut [u8]) -> Result<(), io::Error>;
    fn serialize_vec_len_as_u16(&mut self, value: &mut Vec<u8>) -> Result<(), io::Error>;
}

pub struct WriteStream<'a> {
    buffer: &'a mut Vec<u8>,
}

impl<'a> WriteStream<'a> {
    pub fn new(buffer: &'a mut Vec<u8>) -> Self {
        Self { buffer }
    }
}

impl<'a> Stream for WriteStream<'a> {
    fn is_writing(&self) -> bool {
        true
    }
    fn is_reading(&self) -> bool {
        false
    }

    fn serialize_u8(&mut self, value: &mut u8) -> Result<(), io::Error> {
        self.buffer.write(&value.to_le_bytes())?;
        Ok(())
    }
    fn serialize_u16(&mut self, value: &mut u16) -> Result<(), io::Error> {
        self.buffer.write(&value.to_le_bytes())?;
        Ok(())
    }
    fn serialize_u32(&mut self, value: &mut u32) -> Result<(), io::Error> {
        self.buffer.write(&value.to_le_bytes())?;
        Ok(())
    }
    fn serialize_u64(&mut self, value: &mut u64) -> Result<(), io::Error> {
        self.buffer.write(&value.to_le_bytes())?;
        Ok(())
    }

    fn serialize_bytes(&mut self, value: &mut [u8]) -> Result<(), io::Error> {
        self.buffer.write_all(value)
    }

    fn serialize_vec_len_as_u16(&mut self, value: &mut Vec<u8>) -> Result<(), io::Error> {
        let len = value.len() as u16;
        self.buffer.write(&len.to_le_bytes())?;
        self.buffer.write_all(value)
    }
}

pub struct ReadStream<'a> {
    buffer: &'a [u8],
}

impl<'a> ReadStream<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        Self { buffer }
    }
}

impl<'a> Stream for ReadStream<'a> {
    fn is_writing(&self) -> bool {
        false
    }
    fn is_reading(&self) -> bool {
        true
    }

    fn serialize_u8(&mut self, value: &mut u8) -> Result<(), io::Error> {
        let mut buffer = [0u8; 1];
        self.buffer.read_exact(&mut buffer)?;
        *value = u8::from_le_bytes(buffer);
        Ok(())
    }
    fn serialize_u16(&mut self, value: &mut u16) -> Result<(), io::Error> {
        let mut buffer = [0u8; 2];
        self.buffer.read_exact(&mut buffer)?;
        *value = u16::from_le_bytes(buffer);
        Ok(())
    }
    fn serialize_u32(&mut self, value: &mut u32) -> Result<(), io::Error> {
        let mut buffer = [0u8; 4];
        self.buffer.read_exact(&mut buffer)?;
        *value = u32::from_le_bytes(buffer);
        Ok(())
    }
    fn serialize_u64(&mut self, value: &mut u64) -> Result<(), io::Error> {
        let mut buffer = [0u8; 8];
        self.buffer.read_exact(&mut buffer)?;
        *value = u64::from_le_bytes(buffer);
        Ok(())
    }

    fn serialize_bytes(&mut self, value: &mut [u8]) -> Result<(), io::Error> {
        self.buffer.read_exact(value)?;
        Ok(())
    }

    fn serialize_vec_len_as_u16(&mut self, value: &mut Vec<u8>) -> Result<(), io::Error> {
        let mut len = 0;
        self.serialize_u16(&mut len)?;
        value.resize(len as usize, 0);
        self.buffer.read_exact(value)?;
        Ok(())
    }
}

pub struct MeasureStream {
    size: u64,
}

impl MeasureStream {
    pub fn new() -> Self {
        Self { size: 0 }
    }
}

impl Stream for MeasureStream {
    fn is_writing(&self) -> bool {
        true
    }
    fn is_reading(&self) -> bool {
        false
    }

    fn serialize_u8(&mut self, _value: &mut u8) -> Result<(), io::Error> {
        self.size += 1;
        Ok(())
    }
    fn serialize_u16(&mut self, _value: &mut u16) -> Result<(), io::Error> {
        self.size += 2;
        Ok(())
    }
    fn serialize_u32(&mut self, _value: &mut u32) -> Result<(), io::Error> {
        self.size += 4;
        Ok(())
    }
    fn serialize_u64(&mut self, _value: &mut u64) -> Result<(), io::Error> {
        self.size += 8;
        Ok(())
    }

    fn serialize_bytes(&mut self, value: &mut [u8]) -> Result<(), io::Error> {
        self.size += value.len() as u64;
        Ok(())
    }

    fn serialize_vec_len_as_u16(&mut self, value: &mut Vec<u8>) -> Result<(), io::Error> {
        self.size += 2;
        self.size += value.len() as u64;
        Ok(())
    }
}

pub trait Serialize
where
    Self: Sized + Default,
{
    fn serialize_stream(&mut self, s: &mut impl Stream) -> Result<(), io::Error>;

    fn serialize_size(&mut self) -> Result<u64, io::Error> {
        let mut measure_stream = MeasureStream::new();
        self.serialize_stream(&mut measure_stream)?;

        Ok(measure_stream.size)
    }

    fn serialize(&mut self) -> Result<Vec<u8>, io::Error> {
        let mut buffer = vec![];
        let mut write_stream = WriteStream::new(&mut buffer);
        self.serialize_stream(&mut write_stream)?;
        Ok(buffer)
    }

    fn deserialize(buffer: &[u8]) -> Result<Self, io::Error> {
        let mut read_stream = ReadStream::new(buffer);
        let mut default = Self::default();
        default.serialize_stream(&mut read_stream)?;
        Ok(default)
    }
}
