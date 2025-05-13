use std::sync::{Arc, atomic::AtomicBool};

#[cfg(unix)]
use tokio::signal::unix::signal;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupted {
    OsSigInt,
    OsSigQuit,
    OsSigTerm,
    UserInt,
}

const FORCE_QUIT_THRESHOLD: u8 = 3;

#[derive(Debug)]
/// Used to handle the termination of the application.
///
/// A struct that handles listening for interrupt signals, and/or tracking whether an interrupt signal has been received.
///
/// Essentially, the receiving side of the broadcast channel.
pub struct InterruptReceiver {
    interrupt_rx: broadcast::Receiver<Interrupted>,
    stopped: Arc<AtomicBool>,
}

impl InterruptReceiver {
    #[must_use]
    #[inline]
    pub fn new(interrupt_rx: broadcast::Receiver<Interrupted>) -> Self {
        Self {
            interrupt_rx,
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    #[inline]
    /// Create a dummy receiver that doesn't receive any signals
    ///
    /// Attempting to wait on this receiver will wait indefinitely.
    pub fn dummy() -> Self {
        let (tx, rx) = broadcast::channel(1);

        // forget the sender so it's dropped w/o calling its destructor
        std::mem::forget(tx);

        Self {
            interrupt_rx: rx,
            stopped: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wait for an interrupt signal to be received.
    ///
    /// # Errors
    ///
    /// Fails if the interrupt signal cannot be received (e.g. the sender has been dropped)
    #[inline]
    pub async fn wait(&mut self) -> Result<Interrupted, tokio::sync::broadcast::error::RecvError> {
        let interrupted = self.interrupt_rx.recv().await?;

        // Set the stopped flag to true
        self.stopped
            .store(true, std::sync::atomic::Ordering::SeqCst);

        Ok(interrupted)
    }

    /// Re-subscribe to the broadcast channel.
    ///
    /// Gives you a new receiver that can be used to receive interrupt signals.
    #[must_use]
    #[inline]
    pub fn resubscribe(&self) -> Self {
        // Resubscribe to the broadcast channel
        Self {
            interrupt_rx: self.interrupt_rx.resubscribe(),
            stopped: self.stopped.clone(),
        }
    }

    /// Check if an interrupt signal has been received previously.
    #[must_use]
    #[inline]
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[derive(Debug, Clone)]
/// Used to handle the termination of the application.
///
/// A struct that handles sending interrupt signals to the application.
///
/// Essentially, the sending side of the broadcast channel.
pub struct Terminator {
    interrupt_tx: broadcast::Sender<Interrupted>,
}

impl Terminator {
    #[must_use]
    #[inline]
    pub const fn new(interrupt_tx: broadcast::Sender<Interrupted>) -> Self {
        Self { interrupt_tx }
    }

    /// Send an interrupt signal to the application.
    ///
    /// # Errors
    ///
    /// Fails if the interrupt signal cannot be sent (e.g. the receiver has been dropped)
    #[inline]
    pub fn terminate(&self, interrupted: Interrupted) -> anyhow::Result<()> {
        self.interrupt_tx.send(interrupted)?;

        Ok(())
    }
}

#[cfg(unix)]
#[inline]
async fn terminate_by_signal(terminator: Terminator) {
    let mut interrupt_signal = signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to create interrupt signal stream");
    let mut term_signal = signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to create terminate signal stream");
    let mut quit_signal = signal(tokio::signal::unix::SignalKind::quit())
        .expect("failed to create quit signal stream");

    let mut signal_tick = tokio::time::interval(std::time::Duration::from_secs(1));
    signal_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut kill_count = 0;

    loop {
        // if we've received 3 signals, we should forcefully terminate the application
        if kill_count >= FORCE_QUIT_THRESHOLD {
            log::warn!(
                "Received {FORCE_QUIT_THRESHOLD} signals, forcefully terminating the application"
            );
            std::process::exit(1);
        }

        tokio::select! {
            _ = signal_tick.tick() => {
                // If we receive a signal within 1 second, we can ignore it
                // and wait for the next signal.
            }
            _ = interrupt_signal.recv() => {
                if let Err(e) = terminator.terminate(Interrupted::OsSigInt) {
                    log::warn!("failed to send interrupt signal: {e}");
                }
                kill_count += 1;
            }
            _ = term_signal.recv() => {
                if let Err(e) = terminator.terminate(Interrupted::OsSigTerm) {
                    log::warn!("failed to send terminate signal: {e}");
                }
                kill_count += 1;
            }
            _ = quit_signal.recv() => {
                if let Err(e) = terminator.terminate(Interrupted::OsSigQuit) {
                    log::warn!("failed to send quit signal: {e}");
                }
                kill_count += 1;
            }
            _ = tokio::signal::ctrl_c() => {
                if let Err(e) = terminator.terminate(Interrupted::UserInt) {
                    log::warn!("failed to send interrupt signal: {e}");
                }
                kill_count += 1;
            }
        }
    }
}

#[cfg(not(unix))]
async fn terminate_by_signal(terminator: Terminator) {
    // On non-unix systems, we don't have any signals to handle.
    // We can still use the ctrl_c signal to terminate the application.

    let mut signal_tick = tokio::time::interval(std::time::Duration::from_secs(1));
    signal_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let mut kill_count = 0;

    loop {
        // if we've received 3 signals, we should forcefully terminate the application
        if kill_count >= FORCE_QUIT_THRESHOLD {
            log::warn!(
                "Received {FORCE_QUIT_THRESHOLD} signals, forcefully terminating the application"
            );
            std::process::exit(1);
        }

        tokio::select! {
            _ = signal_tick.tick() => {
                // If we receive a signal within 1 second, we can ignore it
                // and wait for the next signal.
            }
            _ = tokio::signal::ctrl_c() => {
                if let Err(e) = terminator.terminate(Interrupted::UserInt) {
                    log::warn!("failed to send interrupt signal: {e}");
                }
                kill_count += 1;
            }
        }
    }
}

/// create a broadcast channel for retrieving the application kill signal
///
/// # Panics
///
/// This function will panic if the tokio runtime cannot be created.
#[allow(clippy::module_name_repetitions)]
#[must_use]
#[inline]
pub fn create_termination() -> (Terminator, InterruptReceiver) {
    let (tx, rx) = broadcast::channel(2);
    let terminator = Terminator::new(tx);
    let interrupt = InterruptReceiver::new(rx);

    // runtime for the terminator
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .thread_name("mecomp-terminator")
        .build()
        .unwrap();
    let terminator_clone = terminator.clone();

    std::thread::spawn(move || {
        rt.block_on(async {
            terminate_by_signal(terminator_clone).await;
        });
    });

    (terminator, interrupt)
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[rstest]
    #[timeout(Duration::from_secs(1))]
    #[tokio::test]
    async fn test_terminate() {
        let (terminator, mut rx) = create_termination();

        terminator
            .terminate(Interrupted::UserInt)
            .expect("failed to send interrupt signal");

        assert_eq!(rx.wait().await, Ok(Interrupted::UserInt));
    }
}
