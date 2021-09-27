// Copyright 2018-2021, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

//! A simple, asynchronous-IO client for testing purposes made with async-std.
//! Doesn't use any of the async-io stuff in the vrpn crate,
//! so this is durable even if Tokio totally changes everything.
//!
//! However, this doesn't use any Connection structs - just an endpoint and a type dispatcher.
//! In normal usage, this would be bundled into a Connection.
extern crate bytes;
extern crate vrpn;

use std::convert::TryInto;
use std::pin::Pin;
use std::task::Poll;

use async_std::io::{self, Cursor};
use async_std::{
    io::BufReader,
    net::{SocketAddr, TcpStream},
    prelude::*,
    task,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures::io::Read;
use futures::stream::StreamFuture;
use futures::{ready, AsyncBufReadExt, AsyncRead, AsyncReadExt, FutureExt, StreamExt};
use vrpn::buffer_unbuffer::BufferUnbufferError;
use vrpn::vrpn_async_std::cookie::{read_and_check_nonfile_cookie, send_nonfile_cookie};
use vrpn::vrpn_async_std::{read_n_into_bytes_mut, BytesMutReader};
use vrpn::{
    buffer_unbuffer::{peek_u32, BufferTo, BytesMutExtras, UnbufferFrom},
    data_types::{
        cookie::check_ver_nonfile_compatible, CookieData, MessageSize, SequencedGenericMessage,
        TypedMessage,
    },
    handler::{HandlerCode, TypedHandler},
    tracker::PoseReport,
    vrpn_async_std::{read_cookie, AsyncReadMessagesExt},
    Result, TypeDispatcher, VrpnError,
};

#[derive(Debug)]
struct TrackerHandler {}
impl TypedHandler for TrackerHandler {
    type Item = PoseReport;
    fn handle_typed(&mut self, msg: &TypedMessage<PoseReport>) -> Result<HandlerCode> {
        println!("{:?}\n   {:?}", msg.header, msg.body);
        Ok(HandlerCode::ContinueProcessing)
    }
}

async fn async_main() -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:3883".parse().unwrap();
    let mut stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;

    // We first write our cookie, then read and check the server's cookie, before the loop.
    send_nonfile_cookie(&mut stream).await?;
    read_and_check_nonfile_cookie(&mut stream).await?;

    let mut msg_stream = AsyncReadMessagesExt::messages(stream);

    loop {
        match msg_stream.next().await {
            Some(Ok(msg)) => {
                eprintln!("{:?}", msg.into_inner());
            }
            Some(Err(e)) => {
                eprintln!("Got error {:?}", e);
                Err(e)?;
            }
            None => {
                eprintln!("EOF reached (?)");
                return Ok(());
            }
        }
    }
}

fn main() {
    task::block_on(async_main()).unwrap()
}
