//! Preallocated bounded ring buffer for process-site stream boundaries.

use sim_kernel::{Error, Result};
use sim_lib_stream_core::StreamStats;

/// Push result for the bounded process-site ring.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessRingPush<T> {
    /// The item was stored in the ring.
    Accepted,
    /// The ring was full; the rejected item is returned.
    DroppedNewest(T),
    /// The ring was closed; the rejected item is returned.
    Closed(T),
}

/// Capacity snapshot for steady-state process-site checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessRingSnapshot {
    capacity: usize,
    len: usize,
    allocated_slots: usize,
}

/// Preallocated bounded ring used at process-site stream boundaries.
///
/// Cloning is unsupported: a ring handle owns one process-boundary buffer, so
/// copies must be explicit higher-level handles with documented shared state.
///
/// ```compile_fail
/// use sim_lib_stream_host::ProcessSharedRing;
///
/// let ring = ProcessSharedRing::<u8>::with_capacity(1).unwrap();
/// let _copy = ring.clone();
/// ```
#[derive(Debug)]
pub struct ProcessSharedRing<T> {
    slots: Vec<Option<T>>,
    head: usize,
    len: usize,
    closed: bool,
    stats: StreamStats,
}

impl<T> ProcessSharedRing<T> {
    /// Creates a ring preallocated to hold `capacity` items.
    ///
    /// Returns an evaluation error when `capacity` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_host::{ProcessRingPush, ProcessSharedRing};
    ///
    /// let mut ring = ProcessSharedRing::with_capacity(2).unwrap();
    /// assert_eq!(ring.try_push(1), ProcessRingPush::Accepted);
    /// assert_eq!(ring.try_push(2), ProcessRingPush::Accepted);
    /// assert_eq!(ring.try_push(3), ProcessRingPush::DroppedNewest(3));
    /// assert_eq!(ring.try_pop(), Some(1));
    /// ```
    pub fn with_capacity(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::Eval(
                "process ring capacity must be greater than zero".to_owned(),
            ));
        }
        let slots = std::iter::repeat_with(|| None).take(capacity).collect();
        Ok(Self {
            slots,
            head: 0,
            len: 0,
            closed: false,
            stats: StreamStats::default(),
        })
    }

    /// Pushes an item, returning whether it was accepted, dropped (full), or
    /// rejected (closed).
    pub fn try_push(&mut self, item: T) -> ProcessRingPush<T> {
        self.stats.pushed = self.stats.pushed.saturating_add(1);
        if self.closed {
            self.stats.closed = true;
            return ProcessRingPush::Closed(item);
        }
        if self.len == self.capacity() {
            self.stats.dropped_newest = self.stats.dropped_newest.saturating_add(1);
            return ProcessRingPush::DroppedNewest(item);
        }
        let tail = (self.head + self.len) % self.capacity();
        self.slots[tail] = Some(item);
        self.len += 1;
        self.stats.accepted = self.stats.accepted.saturating_add(1);
        ProcessRingPush::Accepted
    }

    /// Pops the oldest item, or returns `None` when the ring is empty.
    pub fn try_pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.slots[self.head].take();
        self.head = (self.head + 1) % self.capacity();
        self.len -= 1;
        if item.is_some() {
            self.stats.yielded = self.stats.yielded.saturating_add(1);
        }
        item
    }

    /// Closes the ring; further pushes are rejected.
    pub fn close(&mut self) {
        self.closed = true;
        self.stats.closed = true;
    }

    /// Drains all buffered items and closes the ring, recording cancellation.
    pub fn cancel(&mut self) {
        while self.try_pop().is_some() {}
        self.closed = true;
        self.stats.closed = true;
        self.stats.cancelled = true;
    }

    /// Returns the number of buffered items.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the ring capacity in items.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Returns the number of slots backing storage has allocated.
    pub fn allocated_slots(&self) -> usize {
        self.slots.capacity()
    }

    /// Returns whether the ring has been closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Captures a capacity snapshot for steady-state checks.
    pub fn snapshot(&self) -> ProcessRingSnapshot {
        ProcessRingSnapshot {
            capacity: self.capacity(),
            len: self.len,
            allocated_slots: self.allocated_slots(),
        }
    }

    /// Returns a clone of the ring's running statistics.
    pub fn stats(&self) -> StreamStats {
        self.stats.clone()
    }
}

impl ProcessRingSnapshot {
    /// Returns the captured ring capacity.
    pub fn capacity(self) -> usize {
        self.capacity
    }

    /// Returns the captured buffered length.
    pub fn len(self) -> usize {
        self.len
    }

    /// Returns whether the ring was empty when captured.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Returns the captured number of allocated slots.
    pub fn allocated_slots(self) -> usize {
        self.allocated_slots
    }
}
