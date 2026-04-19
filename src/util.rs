pub(crate) struct Rc4 {
    state: [u8; 256],
    x: u8,
    y: u8,
}

impl Rc4 {
    pub(crate) fn new(key: &[u8]) -> Self {
        let mut state = [0u8; 256];
        for (i, slot) in state.iter_mut().enumerate() {
            *slot = i as u8;
        }
        if !key.is_empty() {
            let mut j: u8 = 0;
            for i in 0..256 {
                j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
                state.swap(i, j as usize);
            }
        }
        Rc4 { state, x: 0, y: 0 }
    }

    #[inline]
    pub(crate) fn next_byte(&mut self) -> u8 {
        self.x = self.x.wrapping_add(1);
        self.y = self.y.wrapping_add(self.state[self.x as usize]);
        self.state.swap(self.x as usize, self.y as usize);
        let t = self.state[self.x as usize].wrapping_add(self.state[self.y as usize]);
        self.state[t as usize]
    }

    /// Four RC4 output bytes packed little-endian into a `u32`.
    #[inline]
    pub(crate) fn next_u32(&mut self) -> u32 {
        let n0 = self.next_byte() as u32;
        let n1 = self.next_byte() as u32;
        let n2 = self.next_byte() as u32;
        let n3 = self.next_byte() as u32;
        n0 | (n1 << 8) | (n2 << 16) | (n3 << 24)
    }
}

/// SplitMix64 seed-expander. Produces the 64-bit Zobrist keys used by the transposition
/// table; RC4 is kept strictly for generating book-compatible 32-bit locks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub(crate) const fn new(seed: u64) -> Self { SplitMix64 { state: seed } }

    #[inline]
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    #[inline]
    pub(crate) fn next_u32(&mut self) -> u32 { self.next_u64() as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_deterministic() {
        let mut a = Rc4::new(&[0]);
        let mut b = Rc4::new(&[0]);
        for _ in 0..64 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn splitmix_distinct() {
        let mut r = SplitMix64::new(42);
        let mut out = std::collections::HashSet::new();
        for _ in 0..1024 {
            assert!(out.insert(r.next_u64()));
        }
    }
}
