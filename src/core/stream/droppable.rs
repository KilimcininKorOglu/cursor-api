use alloc::sync::Arc;
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_core::stream::Stream;
use tokio::sync::Notify;

/// Stream wrapper that can be controlled to drop via external signal
pub struct DroppableStream<S> {
    stream: Option<S>,
    notify: Arc<Notify>,
    dropped: bool,
}

/// Control handle for triggering Stream drop
#[derive(Clone)]
#[repr(transparent)]
pub struct DropHandle {
    notify: Arc<Notify>,
}

impl<S> DroppableStream<S>
where S: Stream + Unpin
{
    /// Create new controllable Stream and its control handle
    pub fn new(stream: S) -> (Self, DropHandle) {
        let notify = Arc::new(Notify::new());

        (
            Self { stream: Some(stream), notify: notify.clone(), dropped: false },
            DropHandle { notify },
        )
    }
}

impl<S> Stream for DroppableStream<S>
where S: Stream + Unpin
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        // If already processed drop, return directly
        if this.dropped {
            return Poll::Ready(None);
        }

        // Check if there's a drop notification
        let notified = this.notify.notified();
        futures_util::pin_mut!(notified);

        if notified.poll(cx).is_ready() {
            this.stream = None;
            this.dropped = true;
            return Poll::Ready(None);
        }

        // Poll internal stream
        if let Some(ref mut stream) = this.stream {
            Pin::new(stream).poll_next(cx)
        } else {
            Poll::Ready(None)
        }
    }
}

impl DropHandle {
    /// Trigger associated Stream drop
    pub fn drop_stream(self) { self.notify.notify_one(); }
}
