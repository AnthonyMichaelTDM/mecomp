//! Currently, the daemon needs to hold an Arc<Mutex<Sender>> to send messages to clients, the reason the mutex is necessary
//! is because the sender's `.send` method takes a mutable reference to self (needed since senders hold their own buffers and we need to write to the buffer before sending).
//! A question I have is whether the overhead of having the mutex and having to clear the buffer before every send is worth not having to allocate a new buffer for every send.
//!
//! This file contains benchmarks to compare the performance of the UDP sender with the performance of a new buffer for every send, with varying numbers of: subscribers, server threads, message sizes, and message frequencies/concurrency.
//!
//! # Results
//!
//! Note, the two benchmarks that are currently unused were used to find that performance was not affected in any interesting way by the size of the messages or the number of subscribers.
//!
//! ## When threads send a constant number of total messages (e.g., 8 messages per thread if 8 threads and 64 total messages)
//!
//! ### Mutex
//!
//! unaffected by the number of threads
//!
//! ### `RwLock`
//!
//! performance scales linearly with the number of threads
//!
//! ### Insights
//!
//! - The `RwLock` implementation is able to take advantage of additional threads to improve performance, while the Mutex cannot.
//!
//! ## When threads send a constant number of messages per thread (e.g., 64 messages per thread if 8 threads)
//!
//! ### Mutex
//!
//! throughput is unaffected by the number of threads
//!
//! ### `RwLock`
//!
//! throughput scales linearly with the number of threads
//!
//! ### Insights
//!
//! - Again, the `RwLock` implementation is able to take advantage of additional threads to improve performance, while the Mutex cannot.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use criterion::{Criterion, criterion_group, criterion_main};
use object_pool::Pool;
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinSet;

#[derive(Debug)]
pub struct MockListener<const BUF_SIZE: usize> {
    socket: UdpSocket,
    buffer: [u8; BUF_SIZE],
}

impl<const B: usize> MockListener<B> {
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
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be received.
    pub async fn recv(&mut self) -> Result<(usize, SocketAddr), std::io::Error> {
        self.socket.recv_from(&mut self.buffer).await
    }
}

pub struct MockSender {
    socket: UdpSocket,
    buffer: Vec<u8>,
    buffer_pool: Pool<Vec<u8>>,
    /// List of subscribers to send messages to
    subscribers: Vec<SocketAddr>,
}

impl MockSender {
    /// Create a new UDP sender bound to an ephemeral port.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be bound or connected.
    pub async fn new(message_size: usize) -> Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await?;

        Ok(Self {
            socket,
            buffer: Vec::with_capacity(message_size),
            buffer_pool: Pool::new(16, || Vec::with_capacity(message_size)),
            subscribers: Vec::new(),
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
    pub async fn send(&mut self, message: &[u8]) -> Result<()> {
        self.buffer.clear();

        self.buffer.extend_from_slice(message);

        for subscriber in &self.subscribers {
            self.socket.send_to(&self.buffer, subscriber).await?;
        }

        Ok(())
    }

    /// Send a message to the UDP socket.
    /// Cancel safe.
    ///
    /// creates a new buffer every time
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub async fn send_with_temporary_buffer<const B: usize>(&self, message: &[u8]) -> Result<()> {
        let mut buffer = Vec::with_capacity(B);
        buffer.extend_from_slice(message);

        for subscriber in &self.subscribers {
            self.socket.send_to(&buffer, subscriber).await?;
        }

        Ok(())
    }

    /// Send a message to the UDP socket using a buffer pool.
    /// Cancel safe.
    ///
    /// # Errors
    ///
    /// Returns an error if the message cannot be serialized or sent.
    pub async fn send_with_buffer_pool(&self, message: &[u8]) -> Result<()> {
        let mut buffer = self.buffer_pool.pull(|| Vec::with_capacity(message.len()));
        buffer.clear();

        // write to the buffer
        buffer.extend_from_slice(message);

        for subscriber in &self.subscribers {
            self.socket
                .send_to(&buffer[..message.len()], subscriber)
                .await?;
        }

        Ok(())
    }
}

fn _bench_mutex(c: &mut Criterion) {
    // The number of messages to send in each test
    const MESSAGE_COUNT: usize = 64;

    // the sizes of messages, in bytes, to test
    let message_sizes = vec![1, 10, 100];

    // The number of subscribers to send messages to in each test
    let subscriber_counts = vec![2, 4];

    // The number of "server threads" sending messages concurrently
    let server_thread_counts = vec![1, 2, 4, 8];

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("udp_sender_mutex");
    group.warm_up_time(Duration::from_secs(1));

    for message_size in &message_sizes {
        for subscriber_count in &subscriber_counts {
            // create a sender with the given message size
            let sender = Arc::new(Mutex::new(
                rt.block_on(MockSender::new(*message_size)).unwrap(),
            ));

            // create the listeners
            #[allow(clippy::collection_is_never_read)]
            let mut listeners = Vec::with_capacity(*subscriber_count);

            for _ in 0..*subscriber_count {
                let listener = rt
                    .block_on(MockListener::<1024>::with_buffer_size())
                    .unwrap();
                rt.block_on(sender.lock())
                    .add_subscriber(listener.local_addr().unwrap());
                listeners.push(listener);
            }

            for server_thread_count in &server_thread_counts {
                group.throughput(criterion::Throughput::Elements(
                    (MESSAGE_COUNT * *server_thread_count * *subscriber_count) as u64,
                ));
                // benchmark the sender
                group.bench_with_input(
                    format!(
                        "message_size_{message_size}_subscribers_{subscriber_count}_server_threads_{server_thread_count}",
                    )
                    .as_str(),
                    &(*message_size, *server_thread_count),
                    |b, &(message_size, server_thread_count)| {
                        b.to_async(Runtime::new().unwrap()).iter_with_setup(
                            || {
                                (
                                    sender.clone(),
                                    (0..message_size)
                                        .map(|_| rand::random::<u8>())
                                        .collect::<Vec<u8>>(),
                                )
                            },
                            async |(sender, message)| {
                                let mut handles = JoinSet::new();

                                for _ in 0..server_thread_count {
                                    let sender = sender.clone();
                                    let message = message.clone();
                                    handles.spawn(async move {
                                        for _ in 0..MESSAGE_COUNT  {
                                            sender.lock().await.send(&message).await.unwrap();
                                        }
                                    });
                                }

                                handles.join_all().await;
                            },
                        );
                    },
                );
            }
        }
    }

    group.finish();
}

fn _bench_rwlock(c: &mut Criterion) {
    // The number of messages to send in each test
    const MESSAGE_COUNT: usize = 64;

    // the sizes of messages, in bytes, to test
    let message_sizes = vec![1, 10, 100];

    // The number of subscribers to send messages to in each test
    let subscriber_counts = vec![2, 4];

    // The number of "server threads" sending messages concurrently
    let server_thread_counts = vec![1, 2, 4, 8];

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("udp_sender_rwlock");
    group.warm_up_time(Duration::from_secs(1));

    for message_size in &message_sizes {
        for subscriber_count in &subscriber_counts {
            // create a sender with the given message size
            let sender = Arc::new(RwLock::new(
                rt.block_on(MockSender::new(*message_size)).unwrap(),
            ));

            // create the listeners
            #[allow(clippy::collection_is_never_read)]
            let mut listeners = Vec::with_capacity(*subscriber_count);

            for _ in 0..*subscriber_count {
                let listener = rt
                    .block_on(MockListener::<1024>::with_buffer_size())
                    .unwrap();
                rt.block_on(sender.write())
                    .add_subscriber(listener.local_addr().unwrap());
                listeners.push(listener);
            }

            for server_thread_count in &server_thread_counts {
                group.throughput(criterion::Throughput::Elements(
                    (MESSAGE_COUNT * *server_thread_count * *subscriber_count) as u64,
                ));
                // benchmark the sender
                group.bench_with_input(
                    format!(
                        "message_size_{message_size}_subscribers_{subscriber_count}_server_threads_{server_thread_count}",
                    )
                    .as_str(),
                    &(*message_size, *server_thread_count),
                    |b, &(message_size, server_thread_count)| {
                        b.to_async(Runtime::new().unwrap()).iter_with_setup(
                            || {
                                (
                                    sender.clone(),
                                    (0..message_size)
                                        .map(|_| rand::random::<u8>())
                                        .collect::<Vec<u8>>(),
                                )
                            },
                            async |(sender, message)| {
                                let mut handles = JoinSet::new();

                                for _ in 0..server_thread_count {
                                    let sender = sender.clone();
                                    let message = message.clone();
                                    handles.spawn(async move {
                                        for _ in 0..MESSAGE_COUNT  {
                                            sender.read().await.send_with_temporary_buffer::<1024>(&message).await.unwrap();
                                        }
                                    });
                                }

                                handles.join_all().await;
                            },
                        );
                    },
                );
            }
        }
    }

    group.finish();
}

/// Benchmark designed to compare the runtime performance of using a Mutex vs `RwLock` for the UDP sender.
///
/// does this by splitting the messages to send evenly among the threads
fn bench_performance(c: &mut Criterion) {
    // the sizes of messages, in bytes, to test
    const MESSAGE_SIZE: usize = 10;

    // The number of subscribers to send messages to in each test
    const SUBSCRIBER_COUNT: usize = 3;

    // The number of messages to send in each test
    const MESSAGE_COUNT: usize = 64;

    // The number of "server threads" sending messages concurrently
    let server_thread_counts = vec![1, 2, 4, 8, 16];

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("mutex vs rwlock performance");

    // create a sender with the given message size
    let mut mutex_sender = rt.block_on(MockSender::new(MESSAGE_SIZE)).unwrap();
    let mut rwlock_sender = rt.block_on(MockSender::new(MESSAGE_SIZE)).unwrap();

    // create the listeners
    #[allow(clippy::collection_is_never_read)]
    let mut listeners = Vec::with_capacity(SUBSCRIBER_COUNT);

    for _ in 0..SUBSCRIBER_COUNT {
        let listener = rt
            .block_on(MockListener::<1024>::with_buffer_size())
            .unwrap();
        mutex_sender.add_subscriber(listener.local_addr().unwrap());
        rwlock_sender.add_subscriber(listener.local_addr().unwrap());
        listeners.push(listener);
    }

    let mutex_sender = Arc::new(Mutex::new(mutex_sender));
    let rwlock_sender = Arc::new(RwLock::new(rwlock_sender));

    for server_thread_count in &server_thread_counts {
        // benchmark the sender with a mutex
        group.throughput(criterion::Throughput::Bytes(
            (MESSAGE_SIZE * MESSAGE_COUNT * SUBSCRIBER_COUNT) as u64,
        ));
        group.bench_with_input(
            format!("mutex__server_threads_{server_thread_count:02}"),
            server_thread_count,
            |b, &server_thread_count| {
                b.to_async(Runtime::new().unwrap()).iter_with_setup(
                    || {
                        (
                            mutex_sender.clone(),
                            (0..MESSAGE_SIZE)
                                .map(|_| rand::random::<u8>())
                                .collect::<Vec<u8>>(),
                        )
                    },
                    async |(sender, message)| {
                        let mut handles = JoinSet::new();

                        for _ in 0..server_thread_count {
                            let sender = sender.clone();
                            let message = message.clone();
                            handles.spawn(async move {
                                for _ in 0..MESSAGE_COUNT / server_thread_count {
                                    sender.lock().await.send(&message).await.unwrap();
                                }
                            });
                        }

                        handles.join_all().await;
                    },
                );
            },
        );

        // benchmark the sender with a rwlock
        group.throughput(criterion::Throughput::Bytes(
            (MESSAGE_SIZE * MESSAGE_COUNT * SUBSCRIBER_COUNT) as u64,
        ));
        group.bench_with_input(
            format!("rwlock__server_threads_{server_thread_count:02}"),
            server_thread_count,
            |b, &server_thread_count| {
                b.to_async(Runtime::new().unwrap()).iter_with_setup(
                    || {
                        (
                            rwlock_sender.clone(),
                            (0..MESSAGE_SIZE)
                                .map(|_| rand::random::<u8>())
                                .collect::<Vec<u8>>(),
                        )
                    },
                    async |(sender, message)| {
                        let mut handles = JoinSet::new();

                        for _ in 0..server_thread_count {
                            let sender = sender.clone();
                            let message = message.clone();
                            handles.spawn(async move {
                                for _ in 0..MESSAGE_COUNT / server_thread_count {
                                    sender
                                        .read()
                                        .await
                                        .send_with_buffer_pool(&message)
                                        .await
                                        .unwrap();
                                }
                            });
                        }

                        handles.join_all().await;
                    },
                );
            },
        );
    }

    group.finish();
}

/// Benchmark designed to compare the throughput performance of using a Mutex vs `RwLock` for the UDP sender.
///
/// does this by giving each thread 64 messages to send, regardless of the number of threads
fn bench_throughput(c: &mut Criterion) {
    // the sizes of messages, in bytes, to test
    const MESSAGE_SIZE: usize = 10;

    // The number of subscribers to send messages to in each test
    const SUBSCRIBER_COUNT: usize = 3;

    // The number of messages to send in each test
    const MESSAGE_COUNT: usize = 64;

    // The number of "server threads" sending messages concurrently
    let server_thread_counts = vec![1, 2, 4, 8, 16];

    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("mutex vs rwlock throughput");

    // create a sender with the given message size
    let mut mutex_sender = rt.block_on(MockSender::new(MESSAGE_SIZE)).unwrap();
    let mut rwlock_sender = rt.block_on(MockSender::new(MESSAGE_SIZE)).unwrap();

    // create the listeners
    #[allow(clippy::collection_is_never_read)]
    let mut listeners = Vec::with_capacity(SUBSCRIBER_COUNT);

    for _ in 0..SUBSCRIBER_COUNT {
        let listener = rt
            .block_on(MockListener::<1024>::with_buffer_size())
            .unwrap();
        mutex_sender.add_subscriber(listener.local_addr().unwrap());
        rwlock_sender.add_subscriber(listener.local_addr().unwrap());
        listeners.push(listener);
    }

    let mutex_sender = Arc::new(Mutex::new(mutex_sender));
    let rwlock_sender = Arc::new(RwLock::new(rwlock_sender));

    for server_thread_count in &server_thread_counts {
        // benchmark the sender with a mutex
        group.throughput(criterion::Throughput::Bytes(
            (MESSAGE_SIZE * MESSAGE_COUNT * SUBSCRIBER_COUNT * *server_thread_count) as u64,
        ));
        group.bench_with_input(
            format!("mutex__server_threads_{server_thread_count:02}"),
            server_thread_count,
            |b, &server_thread_count| {
                b.to_async(Runtime::new().unwrap()).iter_with_setup(
                    || {
                        (
                            mutex_sender.clone(),
                            (0..MESSAGE_SIZE)
                                .map(|_| rand::random::<u8>())
                                .collect::<Vec<u8>>(),
                        )
                    },
                    async |(sender, message)| {
                        let mut handles = JoinSet::new();

                        for _ in 0..server_thread_count {
                            let sender = sender.clone();
                            let message = message.clone();
                            handles.spawn(async move {
                                for _ in 0..MESSAGE_COUNT {
                                    sender.lock().await.send(&message).await.unwrap();
                                }
                            });
                        }

                        handles.join_all().await;
                    },
                );
            },
        );

        // benchmark the sender with a rwlock
        group.throughput(criterion::Throughput::Bytes(
            (MESSAGE_SIZE * MESSAGE_COUNT * SUBSCRIBER_COUNT * *server_thread_count) as u64,
        ));
        group.bench_with_input(
            format!("rwlock__server_threads_{server_thread_count:02}"),
            server_thread_count,
            |b, &server_thread_count| {
                b.to_async(Runtime::new().unwrap()).iter_with_setup(
                    || {
                        (
                            rwlock_sender.clone(),
                            (0..MESSAGE_SIZE)
                                .map(|_| rand::random::<u8>())
                                .collect::<Vec<u8>>(),
                        )
                    },
                    async |(sender, message)| {
                        let mut handles = JoinSet::new();

                        for _ in 0..server_thread_count {
                            let sender = sender.clone();
                            let message = message.clone();
                            handles.spawn(async move {
                                for _ in 0..MESSAGE_COUNT {
                                    sender
                                        .read()
                                        .await
                                        .send_with_buffer_pool(&message)
                                        .await
                                        .unwrap();
                                }
                            });
                        }

                        handles.join_all().await;
                    },
                );
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().with_plots();
    targets = bench_throughput, bench_performance
);
criterion_main!(benches);
