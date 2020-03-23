//! Crate wrapping [`futures::channel::mpsc::unbounded`] to count messages send
//! and received on unbounded channel sender and receiver via Prometheus
//! counters.
//!
//! Bounding channels is necessary for backpressure and proper scheduling. With
//! unbounded channels there is no way of telling the producer side to slow down
//! in order for the consumer side to catch up. Rephrased, there is no way for
//! the scheduler to know when to favour the consumer task over the producer
//! task on a crowded channel and the other way round for an empty channel.
//!
//! One should avoid unbounded channels at all times. Still there are projects
//! that heavily use them. This crate enables one to gain visibility into the
//! queue length of those unbounded channels.
//!
//! Note: While this should be reasonably performant, given that it boils down
//! to a single atomic operation per send and receive, it is not meant to run in
//! production.
//!
//! Note: Keep in mind that this is using globally initialized counters. While
//! not in any way a programming best practice, using global counters enables
//! one to instrument ones unbounded channels in the least intrusive way. There
//! is no need to initialize counters and no need to register them with a
//! registry in place.
//!
//! ```rust
//! use futures::StreamExt;
//! use instrumented_mpsc::{register_metrics, unbounded};
//! use prometheus::{Counter, Encoder, Registry, TextEncoder};
//! let registry = Registry::new();
//!
//! register_metrics(&registry);
//!
//! let (tx, mut rx) = unbounded();
//!
//! tx.unbounded_send(()).unwrap();
//!
//! futures::executor::block_on(async {
//!     rx.next().await.unwrap();
//! });
//!
//! drop(rx);
//!
//! let mut buffer = vec![];
//! let encoder = TextEncoder::new();
//! let metric_families = registry.gather();
//! encoder.encode(&metric_families, &mut buffer).unwrap();
//!
//! assert_eq!(String::from_utf8(buffer).unwrap(), "# HELP instrumented_mpsc_channels_created_total Channels created total.\
//! \n# TYPE instrumented_mpsc_channels_created_total counter\
//! \ninstrumented_mpsc_channels_created_total 1\
//! \n# HELP instrumented_mpsc_channels_dropped_total Channels dropped total.\
//! \n# TYPE instrumented_mpsc_channels_dropped_total counter\
//! \ninstrumented_mpsc_channels_dropped_total 1\
//! \n# HELP instrumented_mpsc_msgs_received_total Messages received total.\
//! \n# TYPE instrumented_mpsc_msgs_received_total counter\
//! \ninstrumented_mpsc_msgs_received_total 1\
//! \n# HELP instrumented_mpsc_msgs_send_total Messages send total.\
//! \n# TYPE instrumented_mpsc_msgs_send_total counter\
//! \ninstrumented_mpsc_msgs_send_total 1\n");
//! ```

#[macro_use]
extern crate lazy_static;

use futures::{
    channel::mpsc::{self, SendError, TrySendError},
    stream::Stream,
};
use prometheus::{Counter, Registry};

use std::pin::Pin;
use std::task::{Context, Poll};

lazy_static! {
    static ref CHANNELS_CREATED: Counter = Counter::new(
        "instrumented_mpsc_channels_created_total",
        "Channels created total.",
    )
    .unwrap();
    static ref CHANNELS_DROPPED: Counter = Counter::new(
        "instrumented_mpsc_channels_dropped_total",
        "Channels dropped total."
    )
    .unwrap();
    static ref MSGS_SEND: Counter =
        Counter::new("instrumented_mpsc_msgs_send_total", "Messages send total.",).unwrap();
    static ref MSGS_RECEIVED: Counter = Counter::new(
        "instrumented_mpsc_msgs_received_total",
        "Messages received total."
    )
    .unwrap();
}

/// Register metrics like `instrumented_mpsc_msgs_received_total` with the given
/// registry.
pub fn register_metrics(registry: &Registry) {
    registry
        .register(Box::new(CHANNELS_CREATED.clone()))
        .unwrap();

    registry
        .register(Box::new(CHANNELS_DROPPED.clone()))
        .unwrap();

    registry.register(Box::new(MSGS_SEND.clone())).unwrap();

    registry.register(Box::new(MSGS_RECEIVED.clone())).unwrap();
}

/// Wraps [`futures::channel::mpsc::unbounded`] returning an
/// [`futures::channel::mpsc::UnboundedSender`]
/// [`futures::channel::mpsc::UnboundedReceiver`] set with small wrappers
/// counting messages send and received.
//
// TODO: Allow list of labels to be passed here.
pub fn unbounded<T>() -> (UnboundedSender<T>, UnboundedReceiver<T>) {
    CHANNELS_CREATED.inc();
    let (tx, rx) = mpsc::unbounded();
    (UnboundedSender(tx), UnboundedReceiver(rx))
}

/// Wraps [`futures::channel::mpsc::UnboundedSender`] counting messages send.
pub struct UnboundedSender<T>(mpsc::UnboundedSender<T>);

impl<T> UnboundedSender<T> {
    /// Check if the channel is ready to receive a message.
    pub fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), SendError>> {
        self.0.poll_ready(ctx)
    }

    /// Returns whether this channel is closed without needing a context.
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }

    /// Closes this channel from the sender side, preventing any new messages.
    pub fn close_channel(&self) {
        self.0.close_channel()
    }

    /// Disconnects this sender from the channel, closing it if there are no more senders left.
    pub fn disconnect(&mut self) {
        self.0.disconnect()
    }

    pub fn start_send(&mut self, msg: T) -> Result<(), SendError> {
        MSGS_SEND.inc();
        self.0.start_send(msg)
    }

    pub fn unbounded_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        MSGS_SEND.inc();
        self.0.unbounded_send(msg)
    }

    // TODO: needs access to inner sender. Maybe as_ref?
    // pub fn same_receiver(&self, other: &Self) -> bool {

    // }}
}

/// Wraps [`futures::channel::mpsc::UnboundedReceiver`] counting messages
/// received.
pub struct UnboundedReceiver<T>(mpsc::UnboundedReceiver<T>);

impl<T> Unpin for UnboundedReceiver<T> {}

impl<T> Stream for UnboundedReceiver<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        match <mpsc::UnboundedReceiver<T> as Stream>::poll_next(Pin::new(&mut self.0), cx) {
            Poll::Ready(Some(item)) => {
                MSGS_RECEIVED.inc();
                Poll::Ready(Some(item))
            }
            x @ _ => x,
        }
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        CHANNELS_DROPPED.inc();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn it_works() {
        let registry = Registry::new();
        register_metrics(&registry);

        let (tx, mut rx) = unbounded();

        tx.unbounded_send(()).unwrap();

        futures::executor::block_on(async {
            rx.next().await.unwrap();
        });

        drop(rx);

        assert_eq!(4, registry.gather().len());
        for metric in registry.gather() {
            assert_eq!(1_f64, metric.get_metric()[0].get_counter().get_value());
        }
    }
}
