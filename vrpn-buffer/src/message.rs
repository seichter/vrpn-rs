// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

use super::{
    length_prefixed::{self, LengthBehavior, NullTermination},
    prelude::*,
    traits::{
        buffer::{self, Buffer},
        unbuffer::{self, Output, OutputResultExtras, Source, Unbuffer},
        BufferSize, BytesRequired, ConstantBufferSize, WrappedConstantSize,
    },
};
use bytes::{Buf, BufMut, Bytes};
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
};
use vrpn_base::{
    constants::ALIGN,
    message::{GenericBody, InnerDescription, Message},
    time::TimeVal,
    types::{IdType, SenderId, SequenceNumber, TypeId},
};

impl WrappedConstantSize for SequenceNumber {
    type WrappedType = u32;
    fn get<'a>(&'a self) -> &'a Self::WrappedType {
        &self.0
    }
    fn new(v: Self::WrappedType) -> Self {
        SequenceNumber(v)
    }
}

impl WrappedConstantSize for SenderId {
    type WrappedType = IdType;
    fn get<'a>(&'a self) -> &'a Self::WrappedType {
        &self.0
    }
    fn new(v: Self::WrappedType) -> Self {
        SenderId(v)
    }
}

impl WrappedConstantSize for TypeId {
    type WrappedType = IdType;
    fn get<'a>(&'a self) -> &'a Self::WrappedType {
        &self.0
    }
    fn new(v: Self::WrappedType) -> Self {
        TypeId(v)
    }
}
#[inline]
fn compute_padding(len: usize) -> usize {
    ALIGN - (len % ALIGN)
}

#[inline]
fn padded(len: usize) -> usize {
    len + compute_padding(len)
}

/// Simple struct for wrapping all calculations related to Message<T> size.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MessageSize {
    // The unpadded size of a message body only
    pub unpadded_body_size: usize,
}
impl MessageSize {
    const UNPADDED_HEADER_SIZE: usize = 5 * 4;

    /// Get a MessageSize from the unpadded size of a message body only.
    #[inline]
    pub fn from_unpadded_body_size(unpadded_body_size: usize) -> MessageSize {
        MessageSize { unpadded_body_size }
    }

    /// Get a MessageSize from the total unpadded size of a message (header plus body)
    #[inline]
    pub fn from_unpadded_message_size(unpadded_message_size: usize) -> MessageSize {
        MessageSize::from_unpadded_body_size(
            unpadded_message_size - MessageSize::UNPADDED_HEADER_SIZE,
        )
    }

    /// The total unpadded size of a message (header plus body).
    ///
    /// This is the value put in the message header's length field.
    #[inline]
    pub fn unpadded_message_size(&self) -> usize {
        self.unpadded_body_size + MessageSize::UNPADDED_HEADER_SIZE
    }

    #[inline]
    pub fn padded_body_size(&self) -> usize {
        padded(self.unpadded_body_size)
    }

    /// The total padded size of a message (header plus body, padding applied individually).
    ///
    /// This is the size of buffer actually required for this message.
    #[inline]
    pub fn padded_message_size(&self) -> usize {
        self.padded_body_size() + padded(MessageSize::UNPADDED_HEADER_SIZE)
    }
}

/// Wraps a type implementing BufMut to also track the initial remaining length,
/// to allow for automatic padding of fields.
struct BufMutWrapper<T: BufMut>(T, usize);
impl<T: BufMut> BufMutWrapper<T> {
    fn new(buf: T) -> BufMutWrapper<T> {
        BufMutWrapper(buf, buf.remaining_mut())
    }
    fn pad_to_align(&mut self) {
        let buffered = self.1 - self.0.remaining_mut();

        for _ in 0..compute_padding(buffered) {
            self.0.put_u8(0)
        }
    }
    fn borrow_buf_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: BufMut> Deref for BufMutWrapper<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T: BufMut> DerefMut for BufMutWrapper<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

// Header is 5 i32s (padded to vrpn_ALIGN):
// - unpadded header size + unpadded body size
// - time stamp
// - sender
// - type
//
// The four bytes of "padding" are actually the sequence number,
// which are not "officially" part of the header.
//
// body is padded out to vrpn_ALIGN

impl<U: BufferSize> BufferSize for Message<U> {
    fn buffer_size(&self) -> usize {
        MessageSize::from_unpadded_body_size(self.data.buffer_size()).padded_message_size()
    }
}

impl<U: Buffer> Buffer for Message<U> {
    /// Serialize to a buffer.
    fn buffer_ref<T: BufMut>(&self, buf: &mut T) -> buffer::Result {
        let mut buf = BufMutWrapper::new(buf);
        let size = MessageSize::from_unpadded_body_size(self.data.buffer_size());
        if buf.remaining_mut() < size.padded_message_size() {
            return Err(buffer::Error::OutOfBuffer);
        }
        let unpadded_len: u32 = size.unpadded_message_size() as u32;

        Buffer::buffer_ref(&unpadded_len, buf.borrow_buf_mut())
            .and_then(|()| self.time.buffer_ref(buf.borrow_buf_mut()))
            .and_then(|()| self.sender.buffer_ref(buf.borrow_buf_mut()))
            .and_then(|()| self.message_type.buffer_ref(buf.borrow_buf_mut()))
            .and_then(|()| {
                self.sequence_number
                    .unwrap_or(SequenceNumber(0))
                    .buffer_ref(buf.borrow_buf_mut())
            })?;
        buf.pad_to_align();
        Buffer::buffer_ref(&self.data, buf.borrow_buf_mut()).and_then(|()| {
            buf.pad_to_align();
            Ok(())
        })
    }
}

impl<U: Unbuffer> Unbuffer for Message<U> {
    /// Deserialize from a buffer.
    fn unbuffer_ref(buf: &mut Bytes) -> unbuffer::Result<Output<Message<U>>> {
        let unpadded_len = u32::unbuffer_ref(buf).map_exactly_err_to_at_least()?.data();
        let unpadded_len = unpadded_len as usize;
        let unpadded_body_len = unpadded_len - MessageSize::UNPADDED_HEADER_SIZE;

        // Subtracting the length of the u32 we already unbuffered.
        let expected_remaining_bytes = padded_message_size(unpadded_body_len) - size_of::<u32>();

        if buf.len() < expected_remaining_bytes {
            return Err(unbuffer::Error::NeedMoreData(BytesRequired::Exactly(
                expected_remaining_bytes - buf.len(),
            )));
        }
        let time = Unbuffer::unbuffer_ref(buf)?.data();
        let sender = Unbuffer::unbuffer_ref(buf)?.data();
        let message_type = Unbuffer::unbuffer_ref(buf)?.data();
        let sequence_number = Unbuffer::unbuffer_ref(buf)?.data();

        // drop padding bytes
        buf.split_to(compute_padding(MessageSize::UNPADDED_HEADER_SIZE));

        let data;
        {
            let mut data_buf = buf.split_to(unpadded_body_len);
            data = Unbuffer::unbuffer_ref(&mut data_buf)
                .map_exactly_err_to_at_least()?
                .data();
            if data_buf.len() > 0 {
                return Err(unbuffer::Error::ParseError(format!(
                    "message body length was indicated as {}, but {} bytes remain unconsumed",
                    unpadded_body_len,
                    data_buf.len()
                )));
            }
        }

        // drop padding bytes
        buf.split_to(compute_padding(unpadded_body_len));
        Ok(Output(Message::new(
            Some(time),
            message_type,
            sender,
            data,
            Some(sequence_number),
        )))
    }
}

impl Unbuffer for GenericBody {
    fn unbuffer_ref(buf: &mut Bytes) -> unbuffer::Result<Output<GenericBody>> {
        let my_buf = buf.clone();
        buf.advance(my_buf.len());
        Ok(Output(GenericBody::new(my_buf)))
    }
}

impl BufferSize for InnerDescription {
    fn buffer_size(&self) -> usize {
        length_prefixed::buffer_size(self.name.as_ref(), NullTermination::AddTrailingNull)
    }
}

impl Buffer for InnerDescription {
    fn buffer_ref<T: BufMut>(&self, buf: &mut T) -> buffer::Result {
        length_prefixed::buffer_string(
            self.name.as_ref(),
            buf,
            NullTermination::AddTrailingNull,
            LengthBehavior::IncludeNull,
        )
    }
}

impl Unbuffer for InnerDescription {
    fn unbuffer_ref(buf: &mut Bytes) -> unbuffer::Result<Output<InnerDescription>> {
        length_prefixed::unbuffer_string(buf).map(|b| Output(InnerDescription::new(b)))
    }
}

fn unbuffer_typed_message_body<T: Unbuffer>(
    msg: Message<GenericBody>,
) -> unbuffer::Result<Message<T>> {
    let mut buf = msg.data.body_bytes.clone();
    let data = Unbuffer::unbuffer_ref(&mut buf)
        .map_need_more_err_to_generic_parse_err("parsing message body")?
        .data();
    if buf.len() > 0 {
        return Err(unbuffer::Error::ParseError(format!(
            "message body length was indicated as {}, but {} bytes remain unconsumed",
            msg.data.body_bytes.len(),
            buf.len()
        )));
    }
    Ok(Message::new(
        Some(msg.time),
        msg.message_type,
        msg.sender,
        data,
        msg.sequence_number,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constant() {

    // The size field is a u32.
    let len_size = size_of::<u32>();
    
        assert_eq!(MessageSize::UNPADDED_HEADER_SIZE, len_size
        + TimeVal::constant_buffer_size()
        + SenderId::constant_buffer_size()
        + TypeId::constant_buffer_size());
    }
}
