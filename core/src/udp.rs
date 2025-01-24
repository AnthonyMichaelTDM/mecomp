//! Implementation for the UDP stack used by the server to broadcast events to clients

use std::{
    marker::PhantomData,
    net::{Ipv4Addr, SocketAddr},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
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
pub struct Listener<T, const BUF_SIZE: usize> {
    socket: UdpSocket,
    buffer: [u8; BUF_SIZE],
    message_type: PhantomData<T>,
}

impl<T: DeserializeOwned + Send + Sync> Listener<T, MAX_MESSAGE_SIZE> {
    /// Create a new UDP listener bound to the given socket address.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound.
    pub async fn new() -> Result<Self> {
        Self::with_buffer_size().await
    }
}

impl<T: DeserializeOwned + Send + Sync, const B: usize> Listener<T, B> {
    /// Create a new UDP listener bound to the given socket address.
    /// With a custom buffer size (set with const generics).
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound.
    pub async fn with_buffer_size() -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;

        Ok(Self {
            socket,
            buffer: [0; B],
            message_type: PhantomData,
        })
    }

    /// Get the socket address of the listener
    ///
    /// # Errors
    ///
    /// Returns an error if the socket address cannot be retrieved.
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Receive a message from the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be deserialized or received.
    pub async fn recv(&mut self) -> Result<T> {
        let (size, _) = self.socket.recv_from(&mut self.buffer).await?;
        let message = ciborium::from_reader(&self.buffer[..size])?;

        Ok(message)
    }
}

#[derive(Debug)]
pub struct Sender<T> {
    socket: UdpSocket,
    buffer: Vec<u8>,
    /// List of subscribers to send messages to
    subscribers: Vec<SocketAddr>,
    message_type: PhantomData<T>,
}

impl<T: Serialize + Send + Sync> Sender<T> {
    /// Create a new UDP sender bound to an ephemeral port.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound or connected.
    pub async fn new() -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await?;

        Ok(Self {
            socket,
            buffer: Vec::with_capacity(MAX_MESSAGE_SIZE),
            subscribers: Vec::new(),
            message_type: PhantomData,
        })
    }

    /// Add a subscriber to the list of subscribers.
    pub fn add_subscriber(&mut self, subscriber: SocketAddr) {
        self.subscribers.push(subscriber);
    }

    /// Send a message to the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub async fn send(&mut self, message: impl Into<T> + Send + Sync) -> Result<()> {
        self.buffer.clear();

        ciborium::into_writer(&message.into(), &mut self.buffer)?;

        for subscriber in &self.subscribers {
            self.socket.send_to(&self.buffer, subscriber).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[rstest::rstest]
    #[case(Message::Event(Event::LibraryRescanFinished))]
    #[case(Message::Event(Event::LibraryAnalysisFinished))]
    #[case(Message::Event(Event::LibraryReclusterFinished))]
    #[tokio::test]
    #[timeout(std::time::Duration::from_secs(1))]
    async fn test_udp(#[case] message: Message, #[values(1, 2, 3)] num_listeners: usize) {
        let mut sender = Sender::<Message>::new().await.unwrap();

        let mut listeners = Vec::new();

        for _ in 0..num_listeners {
            let listener = Listener::new().await.unwrap();
            sender.add_subscriber(listener.local_addr().unwrap());
            listeners.push(listener);
        }

        sender.send(message.clone()).await.unwrap();

        for (i, listener) in listeners.iter_mut().enumerate() {
            let received_message: Message = listener.recv().await.unwrap();
            assert_eq!(received_message, message, "Listener {}", i);
        }
    }
}
