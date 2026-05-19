//! Subscription state: tracks which symbols are actively subscribed and their
//! reference counts. Uses a `LinkedHashMap` to preserve insertion order so
//! the oldest zero-ref symbol is always evictable first.

use linked_hash_map::LinkedHashMap;

pub struct Subscriptions {
    map: LinkedHashMap<String, u32>,
    cap: usize,
}

pub struct AdmitOutcome {
    /// True if this is a brand-new symbol (needs PriceStream + seeding).
    pub newly_added: bool,
    /// Symbols evicted to make room (need removal from PriceStream + Redis cleanup).
    pub evicted: Vec<String>,
}

impl Subscriptions {
    pub fn new(cap: usize) -> Self {
        Self {
            map: LinkedHashMap::new(),
            cap,
        }
    }

    /// Admit a symbol.
    ///
    /// - If already present: increment ref count and move to the tail (LRU
    ///   bump). Returns `newly_added = false`.
    /// - If new: insert with `ref = 1`. If already at cap, evict the oldest
    ///   entry whose ref count is zero. If no zero-ref entry exists, go over
    ///   cap (active subscriptions are never evicted).
    pub fn admit(&mut self, symbol: &str) -> AdmitOutcome {
        if let Some(&count) = self.map.get(symbol) {
            // Bump ref count and touch (move to tail for LRU purposes).
            self.map.remove(symbol);
            self.map.insert(symbol.to_string(), count + 1);
            return AdmitOutcome {
                newly_added: false,
                evicted: vec![],
            };
        }

        let mut evicted = Vec::new();
        if self.map.len() >= self.cap {
            // Find the oldest (front) entry with ref == 0 and evict it.
            let oldest_zero = self
                .map
                .iter()
                .find(|(_, c)| **c == 0)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest_zero {
                self.map.remove(&k);
                evicted.push(k);
            }
            // No zero-ref entry → intentionally exceed cap; active subs
            // never lose service.
        }

        self.map.insert(symbol.to_string(), 1);
        AdmitOutcome {
            newly_added: true,
            evicted,
        }
    }

    /// Insert a symbol with ref count 0 without triggering eviction or
    /// seeding. Used during rehydration from `tick:active`.
    pub fn admit_at_zero_ref(&mut self, symbol: &str) {
        if !self.map.contains_key(symbol) {
            self.map.insert(symbol.to_string(), 0);
        }
    }

    /// Decrement the ref count for a symbol. The symbol is kept in the map
    /// when it hits zero (left warm for re-subscribe).
    pub fn release(&mut self, symbol: &str) {
        if let Some(count) = self.map.get_mut(symbol) {
            *count = count.saturating_sub(1);
        }
    }

    pub fn contains(&self, symbol: &str) -> bool {
        self.map.contains_key(symbol)
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admit_new_symbol_returns_newly_added() {
        let mut subs = Subscriptions::new(10);
        let out = subs.admit("AAPL");
        assert!(out.newly_added);
        assert!(out.evicted.is_empty());
        assert!(subs.contains("AAPL"));
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn admit_existing_symbol_bumps_ref_not_newly_added() {
        let mut subs = Subscriptions::new(10);
        subs.admit("AAPL"); // ref = 1
        let out = subs.admit("AAPL"); // ref = 2
        assert!(!out.newly_added);
        assert!(out.evicted.is_empty());
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn evicts_oldest_zero_ref_when_cap_hit() {
        let mut subs = Subscriptions::new(2);
        subs.admit("AAPL"); // ref=1
        subs.release("AAPL"); // ref=0  — now evictable
        subs.admit("MSFT"); // ref=1  — cap not hit yet (len=2)
        // At cap. Admit GOOG: AAPL is oldest zero-ref → evict it.
        let out = subs.admit("GOOG");
        assert!(out.newly_added);
        assert_eq!(out.evicted, vec!["AAPL"]);
        assert!(!subs.contains("AAPL"));
        assert!(subs.contains("MSFT"));
        assert!(subs.contains("GOOG"));
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn no_eviction_when_all_active_goes_over_cap() {
        let mut subs = Subscriptions::new(2);
        subs.admit("AAPL"); // ref=1
        subs.admit("MSFT"); // ref=1  — at cap
        // Both are active (ref > 0): cannot evict → go over cap.
        let out = subs.admit("GOOG");
        assert!(out.newly_added);
        assert!(out.evicted.is_empty(), "should not evict active subs");
        assert_eq!(subs.len(), 3);
    }

    #[test]
    fn release_does_not_evict() {
        let mut subs = Subscriptions::new(10);
        subs.admit("AAPL");
        subs.release("AAPL");
        assert!(subs.contains("AAPL"), "symbol stays warm after release");
        assert_eq!(subs.len(), 1);
    }

    #[test]
    fn admit_at_zero_ref_inserts_without_newly_added_flag() {
        let mut subs = Subscriptions::new(10);
        // admit_at_zero_ref is used during rehydration — it does not return an
        // AdmitOutcome, but the symbol should be present with ref 0.
        subs.admit_at_zero_ref("SPY");
        assert!(subs.contains("SPY"));
        // Re-admitting it now via admit() should NOT be newly_added.
        let out = subs.admit("SPY");
        assert!(!out.newly_added, "already in map via admit_at_zero_ref");
    }
}
