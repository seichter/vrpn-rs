// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

use std::fmt::{self, Display};
use std::mem::size_of;
use std::ops::Add;

pub mod buffer {
    use super::{BufferSize, WrappedConstantSize};
    use bytes::BufMut;
    quick_error! {
        #[derive(Debug)]
        pub enum Error {
            OutOfBuffer {
                description("ran out of buffer space")
            }
        }
    }

    pub type Result = std::result::Result<(), Error>;

    /// Trait for types that can be "buffered" (serialized to a byte buffer)
    pub trait Buffer: BufferSize {
        /// Serialize to a buffer.
        fn buffer<T: BufMut>(&self, buf: &mut T) -> Result;

        /// Get the number of bytes required to serialize this to a buffer.
        fn required_buffer_size(&self) -> usize {
            self.buffer_size()
        }
    }

    impl<T: WrappedConstantSize> Buffer for T {
        fn buffer<U: BufMut>(&self, buf: &mut U) -> Result {
            self.get().buffer(buf)
        }
    }

}

pub mod unbuffer {
    use super::{BytesRequired, ConstantBufferSize, WrappedConstantSize};
    use bytes::Bytes;
    use itertools;
    use std::num::ParseIntError;

    quick_error!{
    #[derive(Debug)]
    pub enum Error {
        NeedMoreData(needed: BytesRequired) {
            display("ran out of buffered bytes: need {}", needed)
        }
        InvalidDecimalDigit(chars: Vec<char>) {
            display(self_) -> ("got the following non-decimal-digit(s) {}", itertools::join(chars.iter().map(|x : &char| x.to_string()), ","))
        }
        UnexpectedAsciiData(actual: Bytes, expected: Bytes) {
            display("unexpected data: expected '{:?}', got '{:?}'", &expected[..], &actual[..])
        }
        ParseInt(err: ParseIntError) {
            cause(err)
            description(err.description())
            display("{}", err)
            from()
        }
        ParseError(msg: String) {
            description(msg)
        }
    }
    }

    pub type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug)]
    pub struct Output<T>(pub T);

    impl<T> Output<T> {
        pub fn new(data: T) -> Output<T> {
            Output(data)
        }
        pub fn data(self) -> T {
            self.0
        }

        pub fn borrow_data<'a>(&'a self) -> &'a T {
            &self.0
        }

        pub fn borrow_data_mut<'a>(&'a mut self) -> &'a mut T {
            &mut self.0
        }

        pub fn map<U, F>(self, f: F) -> Output<U>
        where
            U: Sized,
            F: FnOnce(T) -> U,
        {
            Output::new(f(self.0))
        }
    }

    impl<T> Default for Output<T>
    where
        T: Default,
    {
        fn default() -> Output<T> {
            Output(T::default())
        }
    }

    /// Trait for types that can be "unbuffered" (parsed from a byte buffer)
    pub trait Unbuffer: Sized {
        /// Tries to unbuffer.
        ///
        /// Returns Ok(None) if not enough data.
        fn unbuffer(buf: &mut Bytes) -> Result<Output<Self>>;
    }

    /// Implementation trait for constant-buffer-size types,
    /// used by the blanket implementation of Unbuffer.
    pub trait UnbufferConstantSize: Sized + ConstantBufferSize {
        /// Perform the unbuffering: only called with at least as many bytes as needed.
        fn unbuffer_constant_size(buf: Bytes) -> Result<Self>;
    }

    /// Blanket impl for types ipmlementing UnbufferConstantSize.
    impl<T: UnbufferConstantSize> Unbuffer for T {
        fn unbuffer(buf: &mut Bytes) -> Result<Output<Self>> {
            let len = Self::constant_buffer_size();
            if buf.len() < len {
                Err(Error::NeedMoreData(BytesRequired::Exactly(buf.len() - len)))
            } else {
                let my_buf = buf.split_to(len);
                match Self::unbuffer_constant_size(my_buf) {
                    Ok(v) => Ok(Output::new(v)),
                    Err(e) => Err(e),
                }
            }
        }
    }

    impl<T: WrappedConstantSize> UnbufferConstantSize for T {
        fn unbuffer_constant_size(buf: Bytes) -> Result<Self> {
            T::WrappedType::unbuffer_constant_size(buf).map(|v| T::create(v))
        }
    }

    /// Trait used to extend the methods of Result<Output<T>>
    pub trait OutputResultExtras<T> {
        /// Transforms the completed output's data, if successful
        fn and_then_map<U, F>(self, f: F) -> Result<Output<U>>
        where
            U: Sized,
            F: FnOnce(T) -> U;
        /// Map the result that "exactly" n additional bytes are
        /// required to "at least" n additional bytes are required.
        ///
        /// Used when a variable-buffer-size type begins its work by
        /// unbuffering a fixed-size type, like a "length" field.
        fn map_exactly_err_to_at_least(self) -> Self;
    }

    #[deprecated]
    impl<T> OutputResultExtras<T> for Result<Output<T>> {
        fn and_then_map<U, F>(self, f: F) -> Result<Output<U>>
        where
            U: Sized,
            F: FnOnce(T) -> U,
        {
            self.map(|Output(v)| Output(f(v)))
        }

        fn map_exactly_err_to_at_least(self) -> Self {
            match self {
                Ok(v) => Ok(v),
                Err(Error::NeedMoreData(BytesRequired::Exactly(n))) => {
                    Err(Error::NeedMoreData(BytesRequired::AtLeast(n)))
                }
                Err(e) => Err(e),
            }
        }
    }
    /// Check that the buffer begins with the expected string.
    pub fn check_expected(buf: &mut Bytes, expected: &'static [u8]) -> Result<()> {
        let len = expected.len();
        if buf.len() < len {
            return Err(Error::NeedMoreData(BytesRequired::Exactly(buf.len() - len)));
        }
        let my_bytes = buf.split_to(len);
        if my_bytes == expected {
            Ok(())
        } else {
            Err(Error::UnexpectedAsciiData(
                my_bytes,
                Bytes::from_static(expected),
            ))
        }
    }
}

/// Trait for computing the buffer size needed for types
/// that can be "buffered" (serialized to a byte buffer),
pub trait BufferSize {
    /// Indicates the number of bytes required in the buffer to store this.
    fn buffer_size(&self) -> usize;
}

impl<T: ConstantBufferSize> BufferSize for T {
    fn buffer_size(&self) -> usize {
        T::constant_buffer_size()
    }
}

pub trait WrappedConstantSize {
    type WrappedType: buffer::Buffer + unbuffer::UnbufferConstantSize + ConstantBufferSize;
    fn get<'a>(&'a self) -> &'a Self::WrappedType;
    fn create(v: Self::WrappedType) -> Self;
}

impl<T: WrappedConstantSize> ConstantBufferSize for T {
    fn constant_buffer_size() -> usize {
        T::WrappedType::constant_buffer_size()
    }
}

/// Optional trait for things that always take the same amount of space in a buffer.
///
/// Implementing this trait gets you implementations of a bunch of buffer/unbuffer-related traits for free.
pub trait ConstantBufferSize {
    /// Get the amount of space needed in a buffer.
    fn constant_buffer_size() -> usize
    where
        Self: Sized,
    {
        size_of::<Self>()
    }
}

#[derive(Debug)]
pub enum BytesRequired {
    Exactly(usize),
    AtLeast(usize),
    Unknown,
}

impl BytesRequired {
    pub fn satisfied_by(&self, buf_size: usize) -> Option<bool> {
        match *self {
            BytesRequired::Exactly(c) => Some(c <= buf_size),
            BytesRequired::AtLeast(c) => Some(c <= buf_size),
            BytesRequired::Unknown => None,
        }
    }
}

impl Add for BytesRequired {
    type Output = BytesRequired;
    fn add(self, other: BytesRequired) -> Self::Output {
        use self::BytesRequired::*;
        match (self, other) {
            (Exactly(a), Exactly(b)) => Exactly(a + b),
            (AtLeast(a), Exactly(b)) => AtLeast(a + b),
            (Exactly(a), AtLeast(b)) => AtLeast(a + b),
            (AtLeast(a), AtLeast(b)) => AtLeast(a + b),
            // Anything else has Unknown as one term.
            _ => Unknown,
        }
    }
}

impl Display for BytesRequired {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            BytesRequired::Exactly(n) => write!(f, "exactly {}", n),
            BytesRequired::AtLeast(n) => write!(f, "at least {}", n),
            BytesRequired::Unknown => write!(f, "unknown"),
        }
    }
}