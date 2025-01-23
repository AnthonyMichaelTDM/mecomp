use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use futures::{future, prelude::*};
use mecomp_core::rpc::{Application, Event, MusicPlayerClient};
use state::action::{Action, PopupAction};
use tarpc::{
    context::Context,
    server::{incoming::Incoming as _, BaseChannel, Channel as _},
    tokio_serde::formats::Json,
};
use termination::Interrupted;
use tokio::sync::{broadcast, mpsc};
use ui::widgets::popups::PopupType;

pub mod state;
pub mod termination;
#[cfg(test)]
mod test_utils;
pub mod ui;

#[derive(Clone, Debug)]
pub struct Subscriber {
    action_tx: mpsc::UnboundedSender<Action>,
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
}

impl Subscriber {
    const fn new(action_tx: mpsc::UnboundedSender<Action>) -> Self {
        Self { action_tx }
    }

    /// Start the subscriber and register with daemon
    ///
    /// # Errors
    ///
    /// If fail to create the server, or fail to register with daemon
    ///
    /// # Panics
    ///
    /// Panics if the peer address of the underlying TCP transport cannot be determined.
    pub async fn connect(
        daemon: Arc<MusicPlayerClient>,
        action_tx: mpsc::UnboundedSender<Action>,
        mut interrupt_rx: broadcast::Receiver<Interrupted>,
    ) -> anyhow::Result<Interrupted> {
        let application_addr = (IpAddr::V4(Ipv4Addr::LOCALHOST), 0);

        let mut listener =
            tarpc::serde_transport::tcp::listen(application_addr, Json::default).await?;
        let application_addr = listener.local_addr();
        listener.config_mut().max_frame_length(usize::MAX);

        let server = Self::new(action_tx.clone());

        let (handler, abort_handle) = future::abortable(
            listener
                .filter_map(|r| future::ready(r.ok()))
                .map(BaseChannel::with_defaults)
                .max_channels_per_key(10, |t| t.transport().peer_addr().unwrap().ip())
                .map(move |channel| channel.execute(server.clone().serve()).for_each(spawn))
                .buffer_unordered(10)
                .for_each(|()| async {}),
        );

        daemon
            .clone()
            .subscribe_application(Context::current(), application_addr.port())
            .await??;

        tokio::spawn(async move {
            if handler.await == Err(future::Aborted) {
                let _ = daemon
                    .clone()
                    .unsubscribe_application(Context::current(), application_addr.port())
                    .await;
            }
        });

        let interrupted = interrupt_rx.recv().await;

        abort_handle.abort();

        Ok(interrupted?)
    }
}

impl Application for Subscriber {
    async fn notify_event(self, _: Context, event: Event) {
        let notification = match event {
            Event::LibraryRescanFinished => "Library rescan finished",
            Event::LibraryAnalysisFinished => "Library analysis finished",
            Event::LibraryReclusterFinished => "Library recluster finished",
        };

        self.action_tx
            .send(Action::Popup(PopupAction::Open(PopupType::Notification(
                notification.into(),
            ))))
            .unwrap();
    }
}
