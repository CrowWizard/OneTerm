//! Same-connection SFTP task owned and observed by the native SSH owner.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::oneshot;

use oneterm_terminal::SharedState;
use tokio_util::sync::CancellationToken;

use crate::{
    counting_stream::CountingStream,
    handler::SshClientHandler,
    sftp::{SftpCmd, SftpSession},
};

pub(crate) async fn prepare(
    handle: &russh::client::Handle<SshClientHandler>,
    state: &SharedState,
) -> oneterm_core::Result<Prepared> {
    let error = |e| crate::session::phase_error(oneterm_core::ConnectPhase::SftpSetup, e);
    let channel = handle.channel_open_session().await.map_err(error)?;
    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(error)?;
    let (stream, transport) =
        owned_stream(CountingStream::new(channel.into_stream(), state.clone()));
    let channel =
        russh_sftp::client::SftpSession::new_with_config(stream, crate::session::sftp_config())
            .await
            .map_err(|e| crate::session::phase_error(oneterm_core::ConnectPhase::SftpSetup, e))?;
    Ok(Prepared { channel, transport })
}

pub(crate) struct Prepared {
    channel: russh_sftp::client::SftpSession,
    transport: TransportOwner,
}

struct TransportOwner {
    stop: CancellationToken,
    finished: oneshot::Receiver<()>,
}

impl TransportOwner {
    async fn complete(mut self) {
        self.stop.cancel();
        if (&mut self.finished).await.is_err() {
            log::error!("native SFTP stream destruction observation lost");
        }
    }
}

impl Drop for TransportOwner {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

struct OwnedStream<S> {
    stream: S,
    read_cancelled: Pin<Box<dyn Future<Output = ()> + Send>>,
    write_cancelled: Pin<Box<dyn Future<Output = ()> + Send>>,
    // Fields drop in declaration order: signal only after the transport drops.
    _finished: Finished,
}
struct Finished(Option<oneshot::Sender<()>>);

fn owned_stream<S>(stream: S) -> (OwnedStream<S>, TransportOwner) {
    let stop = CancellationToken::new();
    let (finished, observed) = oneshot::channel();
    (
        OwnedStream {
            stream,
            read_cancelled: Box::pin(stop.clone().cancelled_owned()),
            write_cancelled: Box::pin(stop.clone().cancelled_owned()),
            _finished: Finished(Some(finished)),
        },
        TransportOwner {
            stop,
            finished: observed,
        },
    )
}

impl Drop for Finished {
    fn drop(&mut self) {
        if let Some(finished) = self.0.take() {
            let _ = finished.send(());
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for OwnedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.read_cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for OwnedStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.write_cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "native SFTP closed",
            )));
        }
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if self.write_cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if self.write_cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

pub(crate) struct Owner {
    pub(crate) backend: Arc<SftpSession>,
    pub(crate) shutdown: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}

impl Owner {
    pub(crate) fn spawn(prepared: Prepared) -> Self {
        let Prepared { channel, transport } = prepared;
        let (tx, rx) = async_channel::bounded::<SftpCmd>(64);
        let alive = Arc::new(AtomicBool::new(true));
        let backend = SftpSession::new(tx, alive.clone());
        let shutdown = CancellationToken::new();
        let stop = shutdown.clone();
        let task = tokio::spawn(async move {
            // Retained UI Arcs cannot keep admission open after this task exits.
            struct Admission(async_channel::Receiver<SftpCmd>, Arc<AtomicBool>);
            impl Drop for Admission {
                fn drop(&mut self) {
                    self.0.close();
                    self.1.store(false, Ordering::Release);
                }
            }
            let _admission = Admission(rx.clone(), alive.clone());
            crate::sftp_task::native_sftp_task(channel, rx, alive, stop, transport.stop.clone())
                .await;
            transport.complete().await;
        });
        Self {
            backend,
            shutdown,
            task,
        }
    }

    pub(crate) async fn complete(mut self) -> Result<(), tokio::task::JoinError> {
        self.shutdown.cancel();
        (&mut self.task).await
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

#[cfg(test)]
#[path = "native_sftp_tests.rs"]
mod tests;
