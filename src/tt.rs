use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use crate::eval::BAN_VALUE;
use crate::eval::MATE_VALUE;
use crate::eval::WIN_VALUE;
use crate::mv::Move;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Bound {
    Exact = 0,
    Alpha = 1,
    Beta = 2,
}

impl Bound {
    #[inline]
    const fn from_u8(v: u8) -> Bound {
        match v & 0b11 {
            0 => Bound::Exact,
            1 => Bound::Alpha,
            _ => Bound::Beta,
        }
    }
}

/// Decoded TT entry view returned from `probe`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct TtHit {
    pub(crate) mv: Move,
    pub(crate) value: i32,
    pub(crate) depth: i32,
    pub(crate) bound: Bound,
}

/// Packed 48-bit payload layout (bits 0..48 of `data`):
///   mv     : 16
///   value  : 16  (i16)
///   depth  : 8   (i8)
///   bound  : 2
///   age    : 6
#[inline]
fn pack(mv: u16, value: i16, depth: i8, bound: Bound, age: u8) -> u64 {
    (mv as u64)
        | ((value as u16 as u64) << 16)
        | ((depth as u8 as u64) << 32)
        | (((bound as u8) as u64 & 0b11) << 40)
        | ((age as u64 & 0x3f) << 42)
}

#[inline]
fn unpack_mv(data: u64) -> u16 { data as u16 }
#[inline]
fn unpack_value(data: u64) -> i16 { (data >> 16) as i16 }
#[inline]
fn unpack_depth(data: u64) -> i8 { (data >> 32) as i8 }
#[inline]
fn unpack_bound(data: u64) -> Bound { Bound::from_u8(((data >> 40) & 0b11) as u8) }
#[inline]
fn unpack_age(data: u64) -> u8 { ((data >> 42) & 0x3f) as u8 }

pub(crate) struct TtEntry {
    key_xor_data: AtomicU64,
    data: AtomicU64,
}

impl TtEntry {
    const fn new() -> Self { TtEntry { key_xor_data: AtomicU64::new(0), data: AtomicU64::new(0) } }

    /// Probe with the full 64-bit Zobrist key. Returns `Some` iff the cluster slot holds
    /// this key *and* the XOR verification succeeds (guarding against torn writes from
    /// another thread).
    #[inline]
    fn probe(&self, key: u64) -> Option<TtHit> {
        let data = self.data.load(Ordering::Relaxed);
        let key_xor = self.key_xor_data.load(Ordering::Relaxed);
        if data == 0 && key_xor == 0 {
            return None;
        }
        if key_xor ^ data != key {
            return None;
        }
        Some(TtHit {
            mv: Move::from_raw(unpack_mv(data)),
            value: unpack_value(data) as i32,
            depth: unpack_depth(data) as i32,
            bound: unpack_bound(data),
        })
    }

    #[inline]
    fn load_raw(&self) -> (u64, u64) { (self.data.load(Ordering::Relaxed), self.key_xor_data.load(Ordering::Relaxed)) }

    #[inline]
    fn store(&self, key: u64, data: u64) {
        // XOR-validation lock-free write: store `data` first, then `key ^ data`. A racing
        // reader that loads the new `data` before the updated `key_xor_data` fails the
        // identity `key_xor_data ^ data == key` and treats the slot as a miss, so no torn
        // read is ever accepted as valid.
        self.data.store(data, Ordering::Relaxed);
        self.key_xor_data.store(key ^ data, Ordering::Relaxed);
    }
}

impl Default for TtEntry {
    fn default() -> Self { TtEntry::new() }
}

#[repr(align(64))]
#[derive(Default)]
pub(crate) struct Cluster {
    entries: [TtEntry; 4],
}

pub(crate) struct TranspositionTable {
    clusters: Box<[Cluster]>,
    mask: usize,
    age: AtomicU64, // 6-bit value; AtomicU64 chosen for simple fetch_add ergonomics
}

impl TranspositionTable {
    /// Build a table sized to at most `size_bytes`, rounded down to a power-of-two number
    /// of 64-byte clusters.
    pub(crate) fn new(size_bytes: usize) -> Self {
        let min_clusters = 1024usize;
        let cluster_size = std::mem::size_of::<Cluster>();
        let wanted = (size_bytes / cluster_size).max(min_clusters);
        let mut clusters = wanted.next_power_of_two();
        // Round *down* so we never exceed the requested size budget.
        if clusters > wanted {
            clusters /= 2;
        }
        let clusters = clusters.max(min_clusters);
        let mut v = Vec::with_capacity(clusters);
        for _ in 0..clusters {
            v.push(Cluster::default());
        }
        TranspositionTable { clusters: v.into_boxed_slice(), mask: clusters - 1, age: AtomicU64::new(0) }
    }

    pub(crate) fn clear(&mut self) {
        for c in self.clusters.iter_mut() {
            for e in c.entries.iter_mut() {
                e.key_xor_data.store(0, Ordering::Relaxed);
                e.data.store(0, Ordering::Relaxed);
            }
        }
        self.age.store(0, Ordering::Relaxed);
    }

    pub(crate) fn bump_age(&self) { let _ = self.age.fetch_add(1, Ordering::Relaxed); }

    fn current_age(&self) -> u8 { (self.age.load(Ordering::Relaxed) & 0x3f) as u8 }

    pub(crate) fn size_bytes(&self) -> usize { self.clusters.len() * std::mem::size_of::<Cluster>() }

    #[inline]
    fn cluster(&self, key: u64) -> &Cluster { &self.clusters[(key as usize) & self.mask] }

    /// Hint the CPU to fetch the cluster for `key` into L1. Call right after a `make_move`
    /// so the cache line is hot when the child node probes the TT a few hundred cycles
    /// later. No-op on architectures without a stable prefetch intrinsic.
    #[inline]
    pub(crate) fn prefetch(&self, key: u64) {
        let idx = (key as usize) & self.mask;
        // SAFETY: `idx <= self.mask < self.clusters.len()`, and prefetch hints are defined
        // to never fault even on out-of-bounds addresses.
        let ptr = unsafe { self.clusters.as_ptr().add(idx) };

        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::x86_64::_mm_prefetch::<{ core::arch::x86_64::_MM_HINT_T0 }>(ptr as *const i8);
        }
        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!(
                "prfm pldl1keep, [{ptr}]",
                ptr = in(reg) ptr,
                options(nostack, preserves_flags, readonly),
            );
        }
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let _ = ptr;
        }
    }

    /// Look up an entry. Returns the first matching cluster slot.
    pub(crate) fn probe(&self, key: u64) -> Option<TtHit> {
        let cluster = self.cluster(key);
        for entry in &cluster.entries {
            if let Some(hit) = entry.probe(key) {
                return Some(hit);
            }
        }
        None
    }

    pub(crate) fn store(&self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound, ply: u32) {
        let cluster = self.cluster(key);
        let age = self.current_age();

        // Replacement policy: prefer same-key refresh, then empty slots, then the entry
        // with the lowest `(depth - age_penalty)` score. Reads use `Relaxed`; stale data
        // here only affects replacement quality, not correctness.
        let mut victim = 0usize;
        let mut worst = i32::MAX;
        for (i, entry) in cluster.entries.iter().enumerate() {
            let (data, key_xor) = entry.load_raw();
            if data == 0 && key_xor == 0 {
                victim = i;
                break;
            }
            if (key_xor ^ data) == key {
                victim = i;
                break;
            }
            let e_depth = unpack_depth(data) as i32;
            let e_age = unpack_age(data) as i32;
            let age_penalty = (age as i32 - e_age) & 0x3f;
            let quality = e_depth - age_penalty * 4;
            if quality < worst {
                worst = quality;
                victim = i;
            }
        }

        let stored_score = mate_score_to_tt(score, ply);
        let packed = pack(mv.raw(), stored_score as i16, depth.clamp(-1, 127) as i8, bound, age);
        cluster.entries[victim].store(key, packed);
    }
}

// ====================== Mate-distance score adjustments ======================

#[inline]
pub(crate) fn mate_score_to_tt(score: i32, ply: u32) -> i32 {
    let p = ply as i32;
    if score > WIN_VALUE {
        score + p
    } else if score < -WIN_VALUE {
        score - p
    } else {
        score
    }
}

#[inline]
pub(crate) fn mate_score_from_tt(score: i32, ply: u32) -> i32 {
    let p = ply as i32;
    if score > WIN_VALUE {
        if score <= BAN_VALUE { -MATE_VALUE } else { score - p }
    } else if score < -WIN_VALUE {
        if score >= -BAN_VALUE { -MATE_VALUE } else { score + p }
    } else {
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_probe_roundtrip() {
        let tt = TranspositionTable::new(1 << 20);
        let key = 0xDEAD_BEEF_CAFE_BABE;
        tt.store(key, Move::NULL, 42, 5, Bound::Exact, 0);
        let hit = tt.probe(key).expect("should hit");
        assert_eq!(hit.value, 42);
        assert_eq!(hit.depth, 5);
        assert_eq!(hit.bound, Bound::Exact);
    }

    #[test]
    fn probe_miss_on_wrong_key() {
        let tt = TranspositionTable::new(1 << 18);
        tt.store(0x1111, Move::NULL, 1, 1, Bound::Exact, 0);
        assert!(tt.probe(0x2222).is_none());
    }

    #[test]
    fn same_key_refresh() {
        let tt = TranspositionTable::new(1 << 18);
        let key = 0xBEEF_FACE_DEAD_BEAF;
        tt.store(key, Move::NULL, 10, 3, Bound::Exact, 0);
        tt.store(key, Move::NULL, 20, 4, Bound::Exact, 0);
        assert_eq!(tt.probe(key).unwrap().value, 20);
    }

    #[test]
    fn mate_score_roundtrip() {
        let score = MATE_VALUE - 10;
        let stored = mate_score_to_tt(score, 5);
        assert_eq!(mate_score_from_tt(stored, 5), score);
    }

    #[test]
    fn concurrent_stores_no_ub() {
        use std::sync::Arc;
        let tt = Arc::new(TranspositionTable::new(1 << 20));
        let handles: Vec<_> = (0..4)
            .map(|tid| {
                let tt = Arc::clone(&tt);
                std::thread::spawn(move || {
                    for i in 0..10_000u64 {
                        let key = (tid as u64) * 1_000_000 + i;
                        tt.store(key, Move::from_raw(i as u16), i as i32, 1, Bound::Exact, 0);
                        let _ = tt.probe(key);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }
}
