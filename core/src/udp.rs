//! Implementation for the UDP stack used by the server to broadcast events to clients

use std::net::{Ipv4Addr, SocketAddr};

use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;

use crate::errors::UdpError;

pub type Result<T> = std::result::Result<T, UdpError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    LibraryRescanFinished,
    LibraryAnalysisFinished,
    LibraryReclusterFinished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Message {
    Event(Event),
}

impl From<Event> for Message {
    fn from(val: Event) -> Self {
        Self::Event(val)
    }
}

const MAX_MESSAGE_SIZE: usize = 1024;

#[derive(Debug)]
pub struct Listener {
    socket: UdpSocket,
    buffer: [u8; MAX_MESSAGE_SIZE],
}

impl Listener {
    /// Create a new UDP listener bound to the given socket address.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound.
    pub async fn new(socket_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(socket_addr).await?;

        Ok(Self {
            socket,
            buffer: [0; MAX_MESSAGE_SIZE],
        })
    }

    /// Receive a message from the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be deserialized or received.
    pub async fn recv(&mut self) -> Result<Message> {
        let (size, _) = self.socket.recv_from(&mut self.buffer).await?;
        let message = ciborium::from_reader(&self.buffer[..size])?;

        Ok(message)
    }
}

#[derive(Debug)]
pub struct Sender {
    socket: UdpSocket,
    buffer: Vec<u8>,
}

impl Sender {
    /// Create a new UDP sender set to broadcast on the given port.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound or connected.
    pub async fn new(port: u16) -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;

        socket.set_broadcast(true)?;

        socket.connect((Ipv4Addr::LOCALHOST, port)).await?;

        Ok(Self {
            socket,
            buffer: Vec::with_capacity(MAX_MESSAGE_SIZE),
        })
    }

    /// Send a message to the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub async fn send(&mut self, message: impl Into<Message> + Send + Sync) -> Result<()> {
        self.buffer.clear();

        ciborium::into_writer(&message.into(), &mut self.buffer)?;

        self.socket.send(&self.buffer).await?;

        Ok(())
    }

    /// Get the peer address of the socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the peer address cannot be determined.
    pub fn peer_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.peer_addr()?)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[rstest::rstest]
    #[tokio::test]
    #[timeout(std::time::Duration::from_secs(1))]
    async fn test_udp() {
        let port = 6600;
        let mut sender = Sender::new(port).await.unwrap();
        let peer = sender.peer_addr().unwrap();
        let mut listener = Listener::new(peer).await.unwrap();

        let message = Message::Event(Event::LibraryRescanFinished);

        sender.send(message.clone()).await.unwrap();

        let received_message = listener.recv().await.unwrap();
        assert_eq!(message, received_message);
    }
}
