// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

use super::{
    prelude::*,
    traits::{
        buffer::{self, Buffer},
        unbuffer::{self, check_expected, Output, OutputResultExtras, Source, Unbuffer},
        ConstantBufferSize,
    },
};
use bytes::{BufMut, Bytes};
use std::{num::ParseIntError, result};
use vrpn_base::{
    constants::{self, COOKIE_SIZE, MAGIC_PREFIX},
    cookie::{CookieData, Version},
    types::{LogFlags, LogMode},
};

const COOKIE_PADDING: &[u8] = b"\0\0\0\0\0";

impl ConstantBufferSize for CookieData {
    #[inline]
    fn constant_buffer_size() -> usize {
        COOKIE_SIZE
    }
}

impl Buffer for CookieData {
    fn buffer_ref<T: BufMut>(&self, buf: &mut T) -> buffer::Result {
        if buf.remaining_mut() < Self::constant_buffer_size() {
            return Err(buffer::Error::OutOfBuffer);
        }
        buf.put(self.to_string());
        buf.put(COOKIE_PADDING);
        Ok(())
    }
}

#[inline]
fn from_dec(input: &[u8]) -> result::Result<u8, ParseIntError> {
    u8::from_str_radix(&String::from_utf8_lossy(input), 10)
}

#[inline]
fn dec_digits(buf: &mut Bytes, n: usize) -> result::Result<u8, ParseIntError> {
    from_dec(&buf.split_to(n))
}

fn u8_to_log_mode(v: u8) -> LogMode {
    let mut mode = LogMode::none();
    if (v & *LogFlags::INCOMING) != 0 {
        mode.set(LogFlags::INCOMING);
    }
    if (v & *LogFlags::OUTGOING) != 0 {
        mode.set(LogFlags::OUTGOING);
    }
    mode
}

impl Unbuffer for CookieData {
    fn unbuffer_ref(buf: &mut Bytes) -> unbuffer::Result<Output<Self>> {
        // remove "vrpn: ver. "
        check_expected(buf, MAGIC_PREFIX)?;

        let major: u8 = dec_digits(buf, 2)?;

        // remove dot
        check_expected(buf, b".")?;

        let minor: u8 = dec_digits(buf, 2)?;

        // remove spaces
        check_expected(buf, b"  ")?;

        let log_mode: u8 = dec_digits(buf, 1)?;
        let log_mode = u8_to_log_mode(log_mode);

        // remove padding
        check_expected(buf, COOKIE_PADDING)?;

        Ok(Output(CookieData {
            version: Version { major, minor },
            log_mode: Some(log_mode),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn magic_size() {
        // Make sure the size is right.
        use super::{constants, Buffer, CookieData};

        let mut magic_cookie = CookieData::from(constants::MAGIC_DATA);
        magic_cookie.log_mode = Some(LogMode::none());
        assert_eq!(magic_cookie.required_buffer_size(), constants::COOKIE_SIZE);

        let mut buf = Vec::new();
        magic_cookie
            .buffer_ref(&mut buf)
            .expect("Buffering needs to succeed");
        assert_eq!(buf.len(), constants::COOKIE_SIZE);
    }

    #[test]
    fn roundtrip() {
        use super::{constants, Buffer, CookieData, Unbuffer};
        use bytes::BytesMut;

        let mut magic_cookie = CookieData::from(constants::MAGIC_DATA);
        magic_cookie.log_mode = Some(LogMode::none());
        let mut buf = BytesMut::with_capacity(magic_cookie.required_buffer_size());
        magic_cookie
            .buffer_ref(&mut buf)
            .expect("Buffering needs to succeed");
        let mut buf = buf.freeze();
        assert_eq!(
            CookieData::unbuffer_ref(&mut buf).unwrap().data(),
            magic_cookie
        );
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn roundtrip_bytesmut() {
        use super::{constants, Buffer, CookieData, Unbuffer};
        use bytes::BytesMut;

        let mut magic_cookie = CookieData::from(constants::MAGIC_DATA);
        magic_cookie.log_mode = Some(LogMode::none());

        let mut buf = BytesMut::new()
            .allocate_and_buffer(magic_cookie)
            .expect("Buffering needs to succeed")
            .freeze();
        assert_eq!(
            CookieData::unbuffer_ref(&mut buf).unwrap().data(),
            magic_cookie
        );
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn basics() {
        assert_eq!(from_dec(b"1"), Ok(1_u8));
        assert_eq!(from_dec(b"12"), Ok(12_u8));
    }
    #[test]
    fn dec_digits_fn() {
        {
            let mut buf = Bytes::from_static(b"1");
            assert_eq!(dec_digits(&mut buf, 1), Ok(1_u8));
            assert_eq!(buf.len(), 0);
        }
        {
            let mut buf = Bytes::from_static(b"12");
            assert_eq!(dec_digits(&mut buf, 2), Ok(12_u8));
            assert_eq!(buf.len(), 0);
        }
    }
    #[test]
    fn parse_decimal() {
        fn parse_decimal_u8(v: &'static [u8]) -> u8 {
            super::from_dec(v).unwrap()
        }
        assert_eq!(0_u8, parse_decimal_u8(b"0"));
        assert_eq!(0_u8, parse_decimal_u8(b"00"));
        assert_eq!(0_u8, parse_decimal_u8(b"000"));
        assert_eq!(1_u8, parse_decimal_u8(b"1"));
        assert_eq!(1_u8, parse_decimal_u8(b"01"));
        assert_eq!(1_u8, parse_decimal_u8(b"001"));
        assert_eq!(1_u8, parse_decimal_u8(b"0001"));
        assert_eq!(10_u8, parse_decimal_u8(b"10"));
        assert_eq!(10_u8, parse_decimal_u8(b"010"));
        assert_eq!(10_u8, parse_decimal_u8(b"0010"));
        assert_eq!(10_u8, parse_decimal_u8(b"00010"));
    }
}
