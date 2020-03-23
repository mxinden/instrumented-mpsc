# Intrumented Mpsc

<!-- cargo-sync-readme start -->

Crate wrapping [`futures::channel::mpsc::unbounded`] to count messages send
and received on unbounded channel sender and receiver.

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

<!-- cargo-sync-readme end -->
