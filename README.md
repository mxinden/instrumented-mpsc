# Intrumented Mpsc

<!-- cargo-sync-readme start -->

Crate wrapping [`futures::channel::mpsc::unbounded`] to count messages send
and received on unbounded channel sender and receiver via Prometheus
counters.

Bounding channels is necessary for backpressure and proper scheduling. With
unbounded channels there is no way of telling the producer side to slow down
in order for the consumer side to catch up. Rephrased, there is no way for
the scheduler to know when to favour the consumer task over the producer
task on a crowded channel and the other way round for an empty channel.

One should avoid unbounded channels at all times. Still there are projects
that heavily use them. This crate enables one to gain visibility into the
queue length of those unbounded channels.

Note: While this should be reasonably performant, given that it boils down
to a single atomic operation per send and receive, it is not meant to run in
production.

Note: Keep in mind that this is using globally initialized counters. While
not in any way a programming best practice, using global counters enables
one to instrument ones unbounded channels in the least intrusive way. There
is no need to initialize counters and no need to register them with a
registry in place.

```rust
use futures::StreamExt;
use instrumented_mpsc::{register_metrics, unbounded};
use prometheus::{Counter, Encoder, Registry, TextEncoder};
let registry = Registry::new();

register_metrics(&registry);

let (tx, mut rx) = unbounded();

tx.unbounded_send(()).unwrap();

futures::executor::block_on(async {
    rx.next().await.unwrap();
});

drop(rx);

let mut buffer = vec![];
let encoder = TextEncoder::new();
let metric_families = registry.gather();
encoder.encode(&metric_families, &mut buffer).unwrap();

assert_eq!(String::from_utf8(buffer).unwrap(), "# HELP instrumented_mpsc_channels_created_total Channels created total.\
\n# TYPE instrumented_mpsc_channels_created_total counter\
\ninstrumented_mpsc_channels_created_total 1\
\n# HELP instrumented_mpsc_channels_dropped_total Channels dropped total.\
\n# TYPE instrumented_mpsc_channels_dropped_total counter\
\ninstrumented_mpsc_channels_dropped_total 1\
\n# HELP instrumented_mpsc_msgs_received_total Messages received total.\
\n# TYPE instrumented_mpsc_msgs_received_total counter\
\ninstrumented_mpsc_msgs_received_total 1\
\n# HELP instrumented_mpsc_msgs_send_total Messages send total.\
\n# TYPE instrumented_mpsc_msgs_send_total counter\
\ninstrumented_mpsc_msgs_send_total 1\n");
```

<!-- cargo-sync-readme end -->
