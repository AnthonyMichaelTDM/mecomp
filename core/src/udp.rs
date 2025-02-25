//! Implementation for the UDP stack used by the server to broadcast events to clients

use std::{
    fmt::Debug,
    marker::PhantomData,
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use mecomp_storage::db::schemas::Thing;
use object_pool::Pool;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::net::UdpSocket;

use crate::{
    errors::UdpError,
    state::{RepeatMode, Status},
};

pub type Result<T> = std::result::Result<T, UdpError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Event {
    LibraryRescanFinished,
    LibraryAnalysisFinished,
    LibraryReclusterFinished,
    DaemonShutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateChange {
    /// The player has been muted
    Muted,
    /// The player has been unmuted
    Unmuted,
    /// The player volume has changed
    VolumeChanged(f32),
    /// The current track has changed
    TrackChanged(Option<Thing>),
    /// The repeat mode has changed
    RepeatModeChanged(RepeatMode),
    /// Seeked to a new position in the track
    Seeked(Duration),
    /// Playback Status has changes
    StatusChanged(Status),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Message {
    Event(Event),
    StateChange(StateChange),
}

impl From<Event> for Message {
    #[inline]
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
    #[inline]
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
    #[inline]
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
    #[inline]
    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    /// Receive a message from the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be deserialized or received.
    #[inline]
    pub async fn recv(&mut self) -> Result<T> {
        let (size, _) = self.socket.recv_from(&mut self.buffer).await?;
        let message = ciborium::from_reader(&self.buffer[..size])?;

        Ok(message)
    }
}

pub struct Sender<T> {
    socket: UdpSocket,
    buffer_pool: Pool<Vec<u8>>,
    /// List of subscribers to send messages to
    subscribers: Vec<SocketAddr>,
    message_type: PhantomData<T>,
}

impl<T> std::fmt::Debug for Sender<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender")
            .field("socket", &self.socket)
            .field("subscribers", &self.subscribers)
            .field("message_type", &self.message_type)
            .field("buffer_pool.len", &self.buffer_pool.len())
            .finish()
    }
}

impl<T: Serialize + Send + Sync> Sender<T> {
    /// Create a new UDP sender bound to an ephemeral port.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound or connected.
    #[inline]
    pub async fn new() -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await?;

        Ok(Self {
            socket,
            buffer_pool: Pool::new(1, || Vec::with_capacity(MAX_MESSAGE_SIZE)),
            subscribers: Vec::new(),
            message_type: PhantomData,
        })
    }

    /// Add a subscriber to the list of subscribers.
    #[inline]
    pub fn add_subscriber(&mut self, subscriber: SocketAddr) {
        self.subscribers.push(subscriber);
    }

    /// Send a message to the UDP socket.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    #[inline]
    pub async fn send(&self, message: impl Into<T> + Send + Sync + Debug) -> Result<()> {
        log::info!(
            "Forwarding state change: {message:?} to {} subscribers",
            self.subscribers.len()
        );

        let (pool, mut buffer) = self.buffer_pool.pull(Vec::new).detach();
        buffer.clear();

        ciborium::into_writer(&message.into(), &mut buffer)?;

        for subscriber in &self.subscribers {
            self.socket.send_to(&buffer, subscriber).await?;
        }

        pool.attach(buffer);

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
            assert_eq!(received_message, message, "Listener {i}");
        }
    }

    #[rstest::rstest]
    #[case(Message::Event(Event::LibraryRescanFinished))]
    #[case(Message::Event(Event::LibraryAnalysisFinished))]
    #[case(Message::Event(Event::LibraryReclusterFinished))]
    #[case(Message::StateChange(StateChange::Muted))]
    #[case(Message::StateChange(StateChange::Unmuted))]
    #[case(Message::StateChange(StateChange::VolumeChanged(1. / 3.)))]
    #[case(Message::StateChange(StateChange::TrackChanged(None)))]
    #[case(Message::StateChange(StateChange::RepeatModeChanged(RepeatMode::None)))]
    #[case(Message::StateChange(StateChange::Seeked(Duration::from_secs(3))))]
    #[case(Message::StateChange(StateChange::StatusChanged(Status::Paused)))]
    #[case(Message::StateChange(StateChange::StatusChanged(Status::Playing)))]
    #[case(Message::StateChange(StateChange::StatusChanged(Status::Stopped)))]
    #[case(Message::StateChange(StateChange::TrackChanged(Some(
        mecomp_storage::db::schemas::song::Song::generate_id().into()
    ))))]
    fn test_message_encoding_length(#[case] message: Message) {
        let mut buffer = Vec::new();
        ciborium::into_writer(&message, &mut buffer).unwrap();

        assert!(buffer.len() <= MAX_MESSAGE_SIZE);
    }
}
