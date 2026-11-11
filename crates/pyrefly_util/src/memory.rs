/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under the MIT license found in the
 * LICENSE file in the root directory of this source tree.
 */

use std::cmp;
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread::sleep;
use std::thread::spawn;
use std::time::Duration;

use dupe::Dupe;
use human_bytes::human_bytes;
use memory_stats::memory_stats;

use crate::display::commas_iter;
use crate::lock::Mutex;

#[derive(Debug, Clone, Copy, Dupe, Default)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bytes(pub u64);

impl Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", human_bytes(self.0 as f64))
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryUsage {
    pub physical: Option<Bytes>,
    pub allocated: Option<Bytes>,
    pub active: Option<Bytes>,
}

impl Display for MemoryUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fields = [
            ("physical", self.physical),
            ("allocated", self.allocated),
            ("active", self.active),
        ];
        if fields.iter().all(|(_, v)| v.is_none()) {
            write!(f, "unknown")
        } else {
            write!(
                f,
                "{}",
                commas_iter(|| fields
                    .iter()
                    .filter_map(|(name, v)| v.map(|v| format!("{name} {v}"))))
            )
        }
    }
}

impl MemoryUsage {
    pub fn now() -> Self {
        let jemalloc = get_jemalloc_stats();
        Self {
            physical: memory_stats().map(|x| Bytes(x.physical_mem as u64)),
            allocated: jemalloc.allocated,
            active: jemalloc.active,
        }
    }

    pub fn max_by_field(x: &Self, y: &Self) -> Self {
        Self {
            physical: cmp::max(x.physical, y.physical),
            allocated: cmp::max(x.allocated, y.allocated),
            active: cmp::max(x.active, y.active),
        }
    }
}

pub struct JemallocStats {
    pub active: Option<Bytes>,
    pub allocated: Option<Bytes>,
}

static JEMALLOC_STATS: OnceLock<fn() -> JemallocStats> = OnceLock::new();

/// Sets how `MemoryUsage` reads jemalloc statistics. Only the binary knows which
/// allocator it links, so it calls this once at startup. Without it, the jemalloc
/// fields of `MemoryUsage` are `None`.
pub fn set_jemalloc_stats(get: fn() -> JemallocStats) {
    JEMALLOC_STATS
        .set(get)
        .expect("`set_jemalloc_stats` must be called at most once");
}

fn get_jemalloc_stats() -> JemallocStats {
    match JEMALLOC_STATS.get() {
        Some(get) => get(),
        None => JemallocStats {
            active: None,
            allocated: None,
        },
    }
}

pub struct MemoryUsageTrace {
    /// Pair of (stop condition, values that were recorded).
    /// Once stop is set to `true` we won't record any more results.
    state: Arc<Mutex<(bool, MemoryUsage)>>,
}

impl MemoryUsageTrace {
    pub fn start(frequency: Duration) -> Self {
        let state = Arc::new(Mutex::new((false, MemoryUsage::now())));
        let state2 = state.dupe();
        spawn(move || {
            loop {
                sleep(frequency);
                let mut lock = state2.lock();
                if lock.0 {
                    break;
                }
                lock.1 = MemoryUsage::max_by_field(&lock.1, &MemoryUsage::now());
            }
        });
        Self { state }
    }

    pub fn stop(&mut self) {
        let mut lock = self.state.lock();
        if !lock.0 {
            lock.0 = true;
            // If we were running before, make sure we capture a final snapshot
            lock.1 = MemoryUsage::max_by_field(&lock.1, &MemoryUsage::now());
        }
    }

    /// Won't necessarily be a single MemoryUsage, but the peak across many.
    pub fn peak(&self) -> MemoryUsage {
        self.state.lock().1.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "`set_jemalloc_stats` must be called at most once")]
    fn test_set_jemalloc_stats_twice() {
        fn get() -> JemallocStats {
            JemallocStats {
                active: None,
                allocated: None,
            }
        }
        set_jemalloc_stats(get);
        set_jemalloc_stats(get);
    }
}
