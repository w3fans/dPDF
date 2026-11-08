//! A byte-budgeted LRU. Frames larger than the budget are returned but never cached.
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

pub struct ByteCache<K, V> {
    entries: HashMap<K, (Arc<V>, usize, u64)>,
    budget: usize,
    used: usize,
    clock: u64,
}

impl<K: Eq + Hash + Clone, V> ByteCache<K, V> {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget,
            used: 0,
            clock: 0,
        }
    }
    pub fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.clock += 1;
        self.entries.get_mut(key).map(|entry| {
            entry.2 = self.clock;
            entry.0.clone()
        })
    }
    pub fn insert(&mut self, key: K, value: Arc<V>, bytes: usize) {
        if let Some((_, size, _)) = self.entries.remove(&key) {
            self.used -= size;
        }
        if bytes > self.budget {
            return;
        }
        while self.used + bytes > self.budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.2)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            if let Some((_, size, _)) = self.entries.remove(&oldest) {
                self.used -= size;
            }
        }
        self.clock += 1;
        self.entries.insert(key, (value, bytes, self.clock));
        self.used += bytes;
    }
    pub fn retain(&mut self, keep: impl Fn(&K) -> bool) {
        self.entries.retain(|k, _| keep(k));
        self.used = self.entries.values().map(|v| v.1).sum();
    }
    pub fn bytes_used(&self) -> usize {
        self.used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evicts_by_last_access_and_never_exceeds_budget() {
        let mut cache = ByteCache::new(10);
        cache.insert(1, Arc::new("first"), 4);
        cache.insert(2, Arc::new("second"), 4);
        cache.get(&1);
        cache.insert(3, Arc::new("third"), 4);
        assert!(cache.get(&2).is_none());
        assert!(cache.get(&1).is_some());
        cache.insert(4, Arc::new("oversize"), 11);
        assert!(cache.get(&4).is_none());
        assert_eq!(cache.used, 8);
        cache.retain(|k| *k == 3);
        assert_eq!(cache.used, 4);
    }
}
