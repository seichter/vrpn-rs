// Copyright 2018, Collabora, Ltd.
// SPDX-License-Identifier: BSL-1.0
// Author: Ryan A. Pavlik <ryan.pavlik@collabora.com>

use crate::{ping::Client as RawClient, Connection, Error, LocalId, Result, SenderId, SenderName};
use futures::{Stream, StreamExt, ready};
use std::task::Poll;
use std::{sync::Arc, time::Duration};
use tokio::time::{Interval, interval};

pub struct Client<T: Connection + 'static> {
    client: RawClient<T>,
    interval: Interval,
}

impl<T: Connection + 'static> Client<T> {
    fn new_impl(client: RawClient<T>) -> Result<Client<T>> {
        Ok(Client {
            client,
            interval: interval(Duration::from_secs(1)),
        })
    }
    pub fn new(sender: LocalId<SenderId>, connection: Arc<T>) -> Result<Client<T>> {
        Client::new_impl(RawClient::new(sender, connection)?)
    }

    pub fn new_from_name(
        sender: impl Into<SenderName> + Clone,
        connection: Arc<T>,
    ) -> Result<Client<T>> {
        Client::new_impl(RawClient::new_from_name(sender, connection)?)
    }
}

impl<T: Connection + 'static> Stream for Client<T> {
    type Item = Result<()>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let _ = ready!(self
            .interval
            .poll_next_unpin(cx));

        if let Some(radio_silence) = self.client.check_ping_cycle()? {
            eprintln!(
                "It has been {} since the first unanswered ping was sent to the server!",
                radio_silence
            );
        }
        Poll::Ready(Some(Ok(())))
    }
}
