// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

//! Types related to the `vrpn_Tracker` device class

use crate::{
    buffer_unbuffer::{
        buffer::{check_buffer_remaining, Buffer, BufferResult},
        unbuffer::{check_unbuffer_remaining, Unbuffer, UnbufferResult},
        ConstantBufferSize,
    },
    data_types::{
        id_types::Sensor,
        message::{MessageTypeIdentifier, TypedMessageBody},
        name_types::StaticTypeName,
        Quat, Vec3,
    },
};
use bytes::{Buf, BufMut};

/// Position and orientation for trackers.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseReport {
    /// Sensor id
    pub sensor: Sensor,
    /// Position
    pub pos: Vec3,
    /// Orientation
    pub quat: Quat,
}

impl TypedMessageBody for PoseReport {
    const MESSAGE_IDENTIFIER: MessageTypeIdentifier =
        MessageTypeIdentifier::UserMessageName(StaticTypeName(b"vrpn_Tracker Pos_Quat"));
}

impl ConstantBufferSize for PoseReport {
    fn constant_buffer_size() -> usize {
        Sensor::constant_buffer_size() * 2
            + Vec3::constant_buffer_size()
            + Quat::constant_buffer_size()
    }
}

impl Buffer for PoseReport {
    fn buffer_ref<T: BufMut>(&self, buf: &mut T) -> BufferResult {
        check_buffer_remaining(buf, Self::constant_buffer_size())?;
        self.sensor.buffer_ref(buf)?;
        // padding
        self.sensor.buffer_ref(buf)?;
        self.pos.buffer_ref(buf)?;
        self.quat.buffer_ref(buf)?;
        Ok(())
    }
}

impl Unbuffer for PoseReport {
    fn unbuffer_ref<T: Buf>(buf: &mut T) -> UnbufferResult<Self> {
        check_unbuffer_remaining(buf, Self::constant_buffer_size())?;
        let sensor = Sensor::unbuffer_ref(buf)?;
        let _ = Sensor::unbuffer_ref(buf)?;
        let pos = Vec3::unbuffer_ref(buf)?;
        let quat = Quat::unbuffer_ref(buf)?;
        Ok(PoseReport { sensor, pos, quat })
    }
}

/// Linear and angular velocity for trackers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VelocityReport {
    pub sensor: Sensor,
    pub vel: Vec3,
    pub vel_quat: Quat,
    pub vel_quat_dt: f64,
}

impl TypedMessageBody for VelocityReport {
    const MESSAGE_IDENTIFIER: MessageTypeIdentifier =
        MessageTypeIdentifier::UserMessageName(StaticTypeName(b"vrpn_Tracker Velocity"));
}

/// Linear and angular acceleration for trackers.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AccelReport {
    pub sensor: Sensor,
    pub acc: Vec3,
    pub acc_quat: Quat,
    pub acc_quat_dt: f64,
}

impl TypedMessageBody for AccelReport {
    const MESSAGE_IDENTIFIER: MessageTypeIdentifier =
        MessageTypeIdentifier::UserMessageName(StaticTypeName(b"vrpn_Tracker Acceleration"));
}
