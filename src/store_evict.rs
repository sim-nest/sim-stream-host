//! Size-bounded content store with modeled retention eviction.

use std::collections::{BTreeMap, VecDeque};

use sim_kernel::{Error, Expr, Result, Symbol};

/// Stable key for a content-store entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreKey(Symbol);

impl StoreKey {
    /// Builds a store key from a symbol.
    pub fn new(symbol: Symbol) -> Self {
        Self(symbol)
    }

    /// Builds a stream-host content key.
    pub fn named(name: impl Into<String>) -> Self {
        Self(Symbol::qualified("stream/content", name.into()))
    }

    /// Returns the backing symbol.
    pub fn as_symbol(&self) -> &Symbol {
        &self.0
    }
}

/// One sample-by-reference frame stored under bounded host policy.
#[derive(Clone, Debug, PartialEq)]
pub struct ContentFrame {
    key: StoreKey,
    session: Symbol,
    receipt_seq: u64,
    inserted_tick: u64,
    size_bytes: usize,
    value: Expr,
}

impl ContentFrame {
    /// Builds a stored content frame.
    pub fn new(
        key: StoreKey,
        session: Symbol,
        receipt_seq: u64,
        inserted_tick: u64,
        size_bytes: usize,
        value: Expr,
    ) -> Self {
        Self {
            key,
            session,
            receipt_seq,
            inserted_tick,
            size_bytes,
            value,
        }
    }

    /// Returns this frame's key.
    pub fn key(&self) -> &StoreKey {
        &self.key
    }

    /// Returns this frame's owning session.
    pub fn session(&self) -> &Symbol {
        &self.session
    }

    /// Returns the governing consent receipt sequence.
    pub fn receipt_seq(&self) -> u64 {
        self.receipt_seq
    }

    /// Returns the modeled insertion tick.
    pub fn inserted_tick(&self) -> u64 {
        self.inserted_tick
    }

    /// Returns the frame size counted against the store bound.
    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    /// Returns the stored expression value.
    pub fn value(&self) -> &Expr {
        &self.value
    }
}

/// Retention window assigned by one visible consent receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionWindow {
    /// Session that owns the consent receipt.
    pub session: Symbol,
    /// Receipt sequence number.
    pub receipt_seq: u64,
    /// Retention window in modeled milliseconds.
    pub retain_ms: u64,
}

impl RetentionWindow {
    /// Builds a retention window.
    pub fn new(session: Symbol, receipt_seq: u64, retain_ms: u64) -> Self {
        Self {
            session,
            receipt_seq,
            retain_ms,
        }
    }
}

/// One removed content-store entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreEvicted {
    /// Removed key.
    pub key: StoreKey,
    /// Eviction reason.
    pub reason: Symbol,
}

/// Size-bounded store for sample-by-reference frames.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedContentStore {
    max_bytes: usize,
    used_bytes: usize,
    entries: BTreeMap<StoreKey, ContentFrame>,
    order: VecDeque<StoreKey>,
}

impl BoundedContentStore {
    /// Builds an empty store with a byte limit.
    pub fn new(max_bytes: usize) -> Result<Self> {
        if max_bytes == 0 {
            return Err(Error::Eval(
                "content store bound must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            max_bytes,
            used_bytes: 0,
            entries: BTreeMap::new(),
            order: VecDeque::new(),
        })
    }

    /// Inserts a frame, evicting oldest frames until the size bound holds.
    pub fn insert(&mut self, frame: ContentFrame) -> Result<Vec<StoreEvicted>> {
        if frame.size_bytes > self.max_bytes {
            return Err(Error::Eval(format!(
                "content frame {} bytes exceeds store bound {}",
                frame.size_bytes, self.max_bytes
            )));
        }
        let mut evicted = Vec::new();
        if self.remove_existing(&frame.key).is_some() {
            evicted.push(StoreEvicted {
                key: frame.key.clone(),
                reason: replaced_reason(),
            });
        }
        while self.used_bytes.saturating_add(frame.size_bytes) > self.max_bytes {
            if let Some(item) = self.evict_oldest(size_bound_reason()) {
                evicted.push(item);
            } else {
                break;
            }
        }
        self.used_bytes = self.used_bytes.saturating_add(frame.size_bytes);
        self.order.push_back(frame.key.clone());
        self.entries.insert(frame.key.clone(), frame);
        Ok(evicted)
    }

    /// Sweeps frames older than their modeled retention window.
    pub fn sweep_retention(
        &mut self,
        now_tick: u64,
        adapt_hz: u16,
        windows: &[RetentionWindow],
    ) -> Vec<StoreEvicted> {
        let expired: Vec<StoreKey> = self
            .entries
            .values()
            .filter(|frame| {
                windows
                    .iter()
                    .find(|window| {
                        window.session == frame.session && window.receipt_seq == frame.receipt_seq
                    })
                    .is_none_or(|window| {
                        elapsed_ms(now_tick, frame.inserted_tick, adapt_hz) > window.retain_ms
                    })
            })
            .map(|frame| frame.key.clone())
            .collect();
        expired
            .into_iter()
            .filter_map(|key| {
                self.remove_existing(&key).map(|_| StoreEvicted {
                    key,
                    reason: retention_reason(),
                })
            })
            .collect()
    }

    /// Returns true when the store contains `key`.
    pub fn contains(&self, key: &StoreKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the current byte use.
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    /// Returns the maximum byte bound.
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Returns the number of stored frames.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether no frames are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn evict_oldest(&mut self, reason: Symbol) -> Option<StoreEvicted> {
        while let Some(key) = self.order.pop_front() {
            if self.remove_existing(&key).is_some() {
                return Some(StoreEvicted { key, reason });
            }
        }
        None
    }

    fn remove_existing(&mut self, key: &StoreKey) -> Option<ContentFrame> {
        let frame = self.entries.remove(key)?;
        self.used_bytes = self.used_bytes.saturating_sub(frame.size_bytes);
        self.order.retain(|existing| existing != key);
        Some(frame)
    }
}

/// Returns the stable reason for size-bound eviction.
pub fn size_bound_reason() -> Symbol {
    Symbol::qualified("stream/content-evict", "size-bound")
}

/// Returns the stable reason for retention eviction.
pub fn retention_reason() -> Symbol {
    Symbol::qualified("stream/content-evict", "retention")
}

fn replaced_reason() -> Symbol {
    Symbol::qualified("stream/content-evict", "replaced")
}

fn elapsed_ms(now_tick: u64, then_tick: u64, adapt_hz: u16) -> u64 {
    let elapsed_ticks = now_tick.saturating_sub(then_tick);
    elapsed_ticks.saturating_mul(1000) / u64::from(adapt_hz.max(1))
}
