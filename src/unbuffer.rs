// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

//! Traits, etc. related to unbuffering types

use crate::{error::BufferUnbufferError, ConstantBufferSize, SizeRequirement, WrappedConstantSize};
use bytes::{Buf, Bytes};

pub type UnbufferResult<T> = std::result::Result<T, BufferUnbufferError>;

/// Trait for types that can be "unbuffered" (parsed from a byte buffer)
pub trait Unbuffer: Sized {
    /// Tries to unbuffer, advancing the buffer position only if successful.
    ///
    /// Returns `Err(BufferUnbufferError::NeedMoreData(n))` if not enough data.
    fn unbuffer_ref<T: Buf>(buf: &mut T) -> UnbufferResult<Self>;
}

/// Tries to unbuffer from a mutable reference to a buffer.
///
/// Delegates to `Unbuffer::unbuffer_ref()`.
/// Returns `Err(BufferUnbufferError::NeedMoreData(n))` if not enough data.
#[deprecated]
pub fn unbuffer_ref<T: Unbuffer, U: Buf>(buf: &mut U) -> UnbufferResult<T> {
    T::unbuffer_ref(buf)
}

/// Tries to unbuffer, consuming the buffer and returning what's left.
///
/// Should no longer be neccessary now that futures don't require you to consume and return streams
/// with every call.
///
/// Returns `Err(BufferUnbufferError::NeedMoreData(n))` if not enough data.
#[deprecated]
pub fn unbuffer_from<T: Unbuffer>(buf: Bytes) -> UnbufferResult<(T, Bytes)> {
    let mut buf = buf;
    let v = T::unbuffer_ref(&mut buf)?;
    Ok((v, buf))
}

/// Implementation trait for constant-buffer-size types,
/// used by the blanket implementation of Unbuffer.
pub trait UnbufferConstantSize: Sized + ConstantBufferSize {
    /// Perform the unbuffering: only called with at least as many bytes as needed.
    fn unbuffer_constant_size<T: Buf>(buf: &mut T) -> UnbufferResult<Self>;
}

/// Blanket impl for types implementing UnbufferConstantSize.
// TODO implement unbuffer_constant_size everywhere we're checking remaining against Self::constant_buffer_size
impl<T: UnbufferConstantSize> Unbuffer for T {
    fn unbuffer_ref<U: Buf>(buf: &mut U) -> UnbufferResult<Self> {
        let len = Self::constant_buffer_size();
        check_unbuffer_remaining(buf, len)?;
        let mut buf_subset = buf.take(len);
        let mut bytes_subset = buf_subset.copy_to_bytes(len);
        let result = Self::unbuffer_constant_size(&mut bytes_subset);
        // don't advance if we need more data
        if let Err(BufferUnbufferError::NeedMoreData(n)) = result {
            return Err(BufferUnbufferError::NeedMoreData(n));
        }
        buf.advance(len);
        result
    }
}

impl<T: WrappedConstantSize> UnbufferConstantSize for T {
    fn unbuffer_constant_size<U: Buf>(buf: &mut U) -> UnbufferResult<Self> {
        T::WrappedType::unbuffer_constant_size(buf).map(T::new)
    }
}

/// Check whether a buffer has enough bytes remaining to unbuffer a given length
pub fn check_unbuffer_remaining<T: Buf>(
    buf: &T,
    required_len: usize,
) -> std::result::Result<(), BufferUnbufferError> {
    let bytes_len = buf.remaining();
    if bytes_len < required_len {
        Err(SizeRequirement::Exactly(required_len - bytes_len).into())
    } else {
        Ok(())
    }
}

/// Consume the expected static byte string from the buffer.
///
/// ```
/// use vrpn::unbuffer::consume_expected;
/// use bytes::Buf;
/// let mut buf = &b"hello world"[..];
/// assert_eq!(buf.remaining(), 11);
/// assert!(consume_expected(&mut buf, &b"hello"[..]).is_ok());
/// assert_eq!(buf.remaining(), 6);
/// ```
pub fn consume_expected<T: Buf>(
    buf: &mut T,
    expected: &'static [u8],
) -> std::result::Result<(), BufferUnbufferError> {
    let expected_len = expected.len();
    check_unbuffer_remaining(buf, expected_len)?;

    let my_bytes = buf.copy_to_bytes(expected_len);
    if my_bytes == expected {
        Ok(())
    } else {
        Err(BufferUnbufferError::UnexpectedAsciiData {
            actual: my_bytes,
            expected: Bytes::from_static(expected),
        })
    }
}
