// Copyright 2020 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

mod connecting;
mod stream;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub use connecting::Connecting;
use futures::{FutureExt, future::BoxFuture};
use libp2p_core::muxing::{StreamMuxer, StreamMuxerEvent};
pub use stream::Stream;

use crate::{ConnectionError, Error};

/// State for a single opened QUIC connection.
pub struct Connection {
    /// Underlying connection.
    connection: quinn::Connection,
    /// Future for accepting a new incoming bidirectional stream.
    incoming: Option<
        BoxFuture<'static, Result<(quinn::SendStream, quinn::RecvStream), quinn::ConnectionError>>,
    >,
    /// Future for opening a new outgoing bidirectional stream.
    outgoing: Option<
        BoxFuture<'static, Result<(quinn::SendStream, quinn::RecvStream), quinn::ConnectionError>>,
    >,
    /// Future to wait for the connection to be closed.
    closing: Option<BoxFuture<'static, quinn::ConnectionError>>,
    /// Fogata: notes the remote address as one that answers while the connection is open
    /// (see `on_link`).
    _answered: Option<crate::on_link::Answered>,
}

impl Connection {
    /// Build a [`Connection`] from raw components.
    ///
    /// This function assumes that the [`quinn::Connection`] is completely fresh and none of
    /// its methods has ever been called. Failure to comply might lead to logic errors and panics.
    fn new(connection: quinn::Connection, answered: Option<crate::on_link::Answered>) -> Self {
        Self {
            connection,
            incoming: None,
            outgoing: None,
            closing: None,
            _answered: answered,
        }
    }
}

impl StreamMuxer for Connection {
    type Substream = Stream;
    type Error = Error;

    fn poll_inbound(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Substream, Self::Error>> {
        let this = self.get_mut();

        let incoming = this.incoming.get_or_insert_with(|| {
            let connection = this.connection.clone();
            async move { connection.accept_bi().await }.boxed()
        });

        let (send, recv) = futures::ready!(incoming.poll_unpin(cx)).map_err(ConnectionError)?;
        this.incoming.take();
        let stream = Stream::new(send, recv);
        Poll::Ready(Ok(stream))
    }

    fn poll_outbound(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Self::Substream, Self::Error>> {
        let this = self.get_mut();

        let outgoing = this.outgoing.get_or_insert_with(|| {
            let connection = this.connection.clone();
            async move { connection.open_bi().await }.boxed()
        });

        let (send, recv) = futures::ready!(outgoing.poll_unpin(cx)).map_err(ConnectionError)?;
        this.outgoing.take();
        let stream = Stream::new(send, recv);
        Poll::Ready(Ok(stream))
    }

    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<StreamMuxerEvent, Self::Error>> {
        // TODO: If connection migration is enabled (currently disabled) address
        // change on the connection needs to be handled.
        Poll::Pending
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();

        let closing = this.closing.get_or_insert_with(|| {
            this.connection.close(From::from(0u32), &[]);
            let connection = this.connection.clone();
            async move { connection.closed().await }.boxed()
        });

        match futures::ready!(closing.poll_unpin(cx)) {
            // Expected error given that `connection.close` was called above.
            quinn::ConnectionError::LocallyClosed => {}
            error => return Poll::Ready(Err(Error::Connection(ConnectionError(error)))),
        };

        Poll::Ready(Ok(()))
    }
}

/// Fogata: the swarm drops a connection without `poll_close` when it closes it for being
/// idle, on an error, and when the swarm itself goes. Quinn closes a connection once its
/// last handle is dropped, but the datagram registry (`datagram.rs`) holds handles of its
/// own, so the connection would stay up, its keep-alives answered, until the other end gave
/// up. Closed here as `poll_close` closes it, unless it is closed already: one that ended
/// keeps the reason it ended with. The registry lets go of it once its datagram reader sees
/// it closed.
impl Drop for Connection {
    fn drop(&mut self) {
        if self.closing.is_none() && self.connection.close_reason().is_none() {
            self.connection.close(From::from(0u32), &[]);
        }
    }
}

#[cfg(test)]
#[cfg(feature = "tokio")]
mod tests {
    use std::time::Duration;

    use futures::future::poll_fn;
    use libp2p_core::{
        Endpoint, Transport,
        transport::{DialOpts, ListenerId, PortUse, TransportEvent},
    };

    use super::*;

    /// Fogata: a connection dropped without `poll_close`, as the swarm drops one it closes
    /// for being idle, is closed: the other end hears so at once, and the datagram registry
    /// lets go of it. The other end's connection, ended by that close, keeps the reason it
    /// ended with when dropped in turn, and is not closed again.
    #[tokio::test]
    async fn a_connection_dropped_without_poll_close_is_closed_once() {
        let listener_key = libp2p_identity::Keypair::generate_ed25519();
        let mut listener = crate::tokio::Transport::new(crate::Config::new(&listener_key));
        listener
            .listen_on(
                ListenerId::next(),
                "/ip4/127.0.0.1/udp/0/quic-v1".parse().unwrap(),
            )
            .unwrap();
        let listen_addr = match poll_fn(|cx| Pin::new(&mut listener).as_mut().poll(cx)).await {
            TransportEvent::NewAddress { listen_addr, .. } => listen_addr,
            e => panic!("Unexpected event: {e:?}"),
        };
        let dialer_key = libp2p_identity::Keypair::generate_ed25519();
        let mut dialer = crate::tokio::Transport::new(crate::Config::new(&dialer_key));
        let dial = dialer
            .dial(
                listen_addr,
                DialOpts {
                    role: Endpoint::Dialer,
                    port_use: PortUse::New,
                },
            )
            .unwrap();
        let accept = async {
            loop {
                if let TransportEvent::Incoming { upgrade, .. } =
                    poll_fn(|cx| Pin::new(&mut listener).as_mut().poll(cx)).await
                {
                    return upgrade.await.unwrap();
                }
            }
        };
        let (dialed, (_, mut accepted)) = futures::join!(dial, accept);
        let (listener_peer, dialed) = dialed.unwrap();
        let dialer_peer = dialer_key.public().to_peer_id();
        assert!(crate::datagram::holds(&listener_peer));

        drop(dialed);
        let ended = tokio::time::timeout(
            Duration::from_secs(2),
            poll_fn(|cx| Pin::new(&mut accepted).poll_inbound(cx)),
        )
        .await
        .expect("the other end hears the close at once");
        assert!(ended.is_err());
        let remote_close = accepted.connection.close_reason();
        assert!(
            matches!(
                remote_close,
                Some(quinn::ConnectionError::ApplicationClosed(_))
            ),
            "{remote_close:?}"
        );
        let handle = accepted.connection.clone();
        drop(accepted);
        assert_eq!(handle.close_reason(), remote_close);
        drop(handle);

        // Both readers see their connection closed and the registry lets go of both.
        tokio::time::timeout(Duration::from_secs(2), async {
            while crate::datagram::holds(&listener_peer) || crate::datagram::holds(&dialer_peer) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the registry lets go of closed connections");
    }
}
