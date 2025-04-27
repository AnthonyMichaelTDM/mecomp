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

#[derive(Debug, Clone)]
pub struct Terminator {
    interrupt_tx: broadcast::Sender<Interrupted>,
}

impl Terminator {
    #[must_use]
    pub const fn new(interrupt_tx: broadcast::Sender<Interrupted>) -> Self {
        Self { interrupt_tx }
    }

    /// Send an interrupt signal to the application.
    ///
    /// # Errors
    ///
    /// Fails if the interrupt signal cannot be sent (e.g. the receiver has been dropped)
    pub fn terminate(&mut self, interrupted: Interrupted) -> anyhow::Result<()> {
        self.interrupt_tx.send(interrupted)?;

        Ok(())
    }
}

#[cfg(unix)]
async fn terminate_by_unix_signal(mut terminator: Terminator) {
    let mut interrupt_signal = signal(tokio::signal::unix::SignalKind::interrupt())
        .expect("failed to create interrupt signal stream");
    let mut term_signal = signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to create terminate signal stream");
    let mut quit_signal = signal(tokio::signal::unix::SignalKind::quit())
        .expect("failed to create quit signal stream");

    tokio::select! {
        _ = interrupt_signal.recv() => {
            terminator
                .terminate(Interrupted::OsSigInt)
                .expect("failed to send interrupt signal");
        }
        _ = term_signal.recv() => {
            terminator
                .terminate(Interrupted::OsSigTerm)
                .expect("failed to send terminate signal");
        }
        _ = quit_signal.recv() => {
            terminator
                .terminate(Interrupted::OsSigQuit)
                .expect("failed to send quit signal");
        }
    }
}

// create a broadcast channel for retrieving the application kill signal
#[allow(clippy::module_name_repetitions)]
#[must_use]
pub fn create_termination() -> (Terminator, broadcast::Receiver<Interrupted>) {
    let (tx, rx) = broadcast::channel(1);
    let terminator = Terminator::new(tx);

    tokio::spawn(terminate_by_unix_signal(terminator.clone()));

    (terminator, rx)
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
        let (mut terminator, mut rx) = create_termination();

        terminator
            .terminate(Interrupted::UserInt)
            .expect("failed to send interrupt signal");

        assert_eq!(rx.recv().await, Ok(Interrupted::UserInt));
    }
}
