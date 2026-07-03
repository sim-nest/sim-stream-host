use sim_lib_stream_core::StreamStats;

use crate::{ProcessRingPush, ProcessSharedRing};

#[test]
fn process_ring_is_bounded_and_fifo() {
    let mut ring = ProcessSharedRing::with_capacity(2).unwrap();

    assert_eq!(ring.try_push("a"), ProcessRingPush::Accepted);
    assert_eq!(ring.try_push("b"), ProcessRingPush::Accepted);
    assert_eq!(ring.try_push("c"), ProcessRingPush::DroppedNewest("c"));
    assert_eq!(ring.try_pop(), Some("a"));
    assert_eq!(ring.try_push("d"), ProcessRingPush::Accepted);
    assert_eq!(ring.try_pop(), Some("b"));
    assert_eq!(ring.try_pop(), Some("d"));
    assert_eq!(ring.try_pop(), None);

    let stats = ring.stats();
    assert_eq!(stats.pushed, 4);
    assert_eq!(stats.accepted, 3);
    assert_eq!(stats.yielded, 3);
    assert_eq!(stats.dropped_newest, 1);
}

#[test]
fn process_ring_keeps_steady_state_capacity() {
    let mut ring = ProcessSharedRing::with_capacity(4).unwrap();
    let before = ring.snapshot();

    for item in 0..32 {
        assert_eq!(ring.try_push(item), ProcessRingPush::Accepted);
        assert_eq!(ring.try_pop(), Some(item));
    }

    assert_eq!(ring.snapshot(), before);
    assert_eq!(ring.stats().accepted, 32);
    assert_eq!(ring.stats().yielded, 32);
}

#[test]
fn process_ring_closes_and_cancels_without_allocating() {
    let mut ring = ProcessSharedRing::with_capacity(1).unwrap();
    let before = ring.snapshot();

    assert_eq!(ring.try_push(1), ProcessRingPush::Accepted);
    ring.cancel();
    assert!(ring.is_closed());
    assert_eq!(ring.try_push(2), ProcessRingPush::Closed(2));
    assert_eq!(ring.snapshot().capacity(), before.capacity());
    assert_eq!(ring.snapshot().allocated_slots(), before.allocated_slots());
    assert_eq!(
        ring.stats(),
        StreamStats {
            pushed: 2,
            accepted: 1,
            yielded: 1,
            closed: true,
            cancelled: true,
            ..StreamStats::default()
        }
    );
}
