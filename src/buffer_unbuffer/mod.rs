// Copyright 2021, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

//! Routines and traits to buffer/unbuffer to/from byte buffers.

mod buffer;
pub mod constants;
mod error;
mod primitives;
mod size;
mod size_requirement;
mod unbuffer;

#[doc(inline)]
pub use crate::buffer_unbuffer::{
    buffer::{Buffer, BytesMutExtras},
    error::BufferUnbufferError,
    primitives::*,
    size::{BufferSize, ConstantBufferSize, EmptyMessage, WrappedConstantSize},
    size_requirement::SizeRequirement,
    unbuffer::Unbuffer,
};
