use std::sync::atomic::{AtomicU64, Ordering};

/// Thread-safe UID generator using atomic operations
#[derive(Debug)]
pub struct UidGenerator {
    counter: AtomicU64,
}

impl UidGenerator {
    /// Create a new UID generator starting from 1
    pub fn new() -> Self {
        UidGenerator {
            counter: AtomicU64::new(1),
        }
    }

    /// Create a new UID generator starting from a specific value
    pub fn from(start: u64) -> Self {
        UidGenerator {
            counter: AtomicU64::new(start),
        }
    }

    /// Get the next UID
    pub fn next(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Get the current UID without incrementing
    pub fn current(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }

    /// Reset the counter to a specific value
    pub fn reset(&self, value: u64) {
        self.counter.store(value, Ordering::Relaxed);
    }
}

impl Default for UidGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_uid_generation() {
        let gen = UidGenerator::new();
        assert_eq!(gen.next(), 1);
        assert_eq!(gen.next(), 2);
        assert_eq!(gen.next(), 3);
    }

    #[test]
    fn test_uid_from_custom_start() {
        let gen = UidGenerator::from(100);
        assert_eq!(gen.next(), 100);
        assert_eq!(gen.next(), 101);
    }

    #[test]
    fn test_uid_thread_safe() {
        let gen = Arc::new(UidGenerator::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let gen_clone = Arc::clone(&gen);
            let handle = thread::spawn(move || {
                let mut uids = Vec::new();
                for _ in 0..100 {
                    uids.push(gen_clone.next());
                }
                uids
            });
            handles.push(handle);
        }

        let mut all_uids = Vec::new();
        for handle in handles {
            let uids = handle.join().unwrap();
            all_uids.extend(uids);
        }

        // Check we have 1000 unique UIDs
        assert_eq!(all_uids.len(), 1000);

        // Sort and check they're sequential
        all_uids.sort();
        for (i, uid) in all_uids.iter().enumerate() {
            assert_eq!(*uid, (i + 1) as u64);
        }
    }

    #[test]
    fn test_current_and_reset() {
        let gen = UidGenerator::new();
        assert_eq!(gen.current(), 1);
        assert_eq!(gen.next(), 1);
        assert_eq!(gen.current(), 2);

        gen.reset(1000);
        assert_eq!(gen.current(), 1000);
        assert_eq!(gen.next(), 1000);
        assert_eq!(gen.current(), 1001);
    }
}
