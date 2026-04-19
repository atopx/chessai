use std::fmt;
use std::iter::FusedIterator;
use std::ops::BitAnd;
use std::ops::BitAndAssign;
use std::ops::BitOr;
use std::ops::BitOrAssign;
use std::ops::BitXor;
use std::ops::BitXorAssign;
use std::ops::Not;
use std::ops::Shl;
use std::ops::Shr;
use std::ops::Sub;

use crate::square::Square;

/// `1u128 << 90 - 1`; 1 bits in positions 0..=89.
pub const BOARD_MASK: u128 = (1u128 << 90) - 1;

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct BitBoard(pub u128);

impl BitBoard {
    pub const EMPTY: BitBoard = BitBoard(0);
    pub const FULL: BitBoard = BitBoard(BOARD_MASK);

    #[inline]
    pub const fn from_raw(raw: u128) -> BitBoard { BitBoard(raw & BOARD_MASK) }

    #[inline]
    pub const fn raw(self) -> u128 { self.0 }

    #[inline]
    pub const fn from_square(sq: Square) -> BitBoard { BitBoard(1u128 << sq.raw() as u32) }

    #[inline]
    pub const fn has(self, sq: Square) -> bool { (self.0 >> sq.raw() as u32) & 1 == 1 }

    #[inline]
    pub const fn with(self, sq: Square) -> BitBoard { BitBoard(self.0 | (1u128 << sq.raw() as u32)) }

    #[inline]
    pub const fn without(self, sq: Square) -> BitBoard { BitBoard(self.0 & !(1u128 << sq.raw() as u32)) }

    #[inline]
    pub const fn is_empty(self) -> bool { self.0 == 0 }

    #[inline]
    pub const fn any(self) -> bool { self.0 != 0 }

    #[inline]
    pub const fn popcount(self) -> u32 { self.0.count_ones() }

    /// Index of the lowest set bit. Caller must ensure the bitboard is non-empty.
    #[inline]
    pub const fn lsb_square(self) -> Square {
        debug_assert!(self.0 != 0);
        Square::new_unchecked(self.0.trailing_zeros() as u8)
    }

    /// Pops and returns the lowest set square (mutates).
    #[inline]
    pub fn pop_lsb(&mut self) -> Square {
        let sq = self.lsb_square();
        self.0 &= self.0 - 1;
        sq
    }

    /// Iterate over every set square in ascending order.
    #[inline]
    pub const fn iter(self) -> BitBoardIter { BitBoardIter(self.0) }

    // -------- Direction shifts (from red's perspective) --------

    /// Rank + 1 (toward black's home).
    #[inline]
    pub const fn up(self) -> BitBoard { BitBoard((self.0 << 9) & BOARD_MASK) }

    /// Rank - 1 (toward red's home).
    #[inline]
    pub const fn down(self) -> BitBoard { BitBoard(self.0 >> 9) }

    /// File + 1.
    #[inline]
    pub const fn right(self) -> BitBoard { BitBoard(((self.0 & NOT_FILE_I.0) << 1) & BOARD_MASK) }

    /// File - 1.
    #[inline]
    pub const fn left(self) -> BitBoard { BitBoard((self.0 & NOT_FILE_A.0) >> 1) }

    /// Shift by an abstract direction. Non-const because it dispatches on the enum.
    #[inline]
    pub fn shift(self, dir: Direction) -> BitBoard {
        match dir {
            Direction::Up => self.up(),
            Direction::Down => self.down(),
            Direction::Left => self.left(),
            Direction::Right => self.right(),
        }
    }
}

// -------- Iterator --------

#[derive(Copy, Clone, Debug)]
pub struct BitBoardIter(u128);

impl Iterator for BitBoardIter {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let sq = Square::new_unchecked(self.0.trailing_zeros() as u8);
        self.0 &= self.0 - 1;
        Some(sq)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for BitBoardIter {}
impl FusedIterator for BitBoardIter {}

impl IntoIterator for BitBoard {
    type Item = Square;
    type IntoIter = BitBoardIter;
    #[inline]
    fn into_iter(self) -> BitBoardIter { self.iter() }
}

// -------- Bitwise ops --------

impl BitOr for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn bitor(self, rhs: BitBoard) -> BitBoard { BitBoard(self.0 | rhs.0) }
}

impl BitAnd for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn bitand(self, rhs: BitBoard) -> BitBoard { BitBoard(self.0 & rhs.0) }
}

impl BitXor for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn bitxor(self, rhs: BitBoard) -> BitBoard { BitBoard(self.0 ^ rhs.0) }
}

impl Sub for BitBoard {
    /// `a - b` == `a & !b`; convenient for "remove friendly pieces from target mask".
    type Output = BitBoard;
    #[inline]
    fn sub(self, rhs: BitBoard) -> BitBoard { BitBoard(self.0 & !rhs.0) }
}

impl Not for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn not(self) -> BitBoard { BitBoard(!self.0 & BOARD_MASK) }
}

impl Shl<u32> for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn shl(self, n: u32) -> BitBoard { BitBoard((self.0 << n) & BOARD_MASK) }
}

impl Shr<u32> for BitBoard {
    type Output = BitBoard;
    #[inline]
    fn shr(self, n: u32) -> BitBoard { BitBoard(self.0 >> n) }
}

impl BitOrAssign for BitBoard {
    #[inline]
    fn bitor_assign(&mut self, rhs: BitBoard) { self.0 |= rhs.0; }
}

impl BitAndAssign for BitBoard {
    #[inline]
    fn bitand_assign(&mut self, rhs: BitBoard) { self.0 &= rhs.0; }
}

impl BitXorAssign for BitBoard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: BitBoard) { self.0 ^= rhs.0; }
}

impl fmt::Display for BitBoard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "  a b c d e f g h i")?;
        for rank in (0..10).rev() {
            write!(f, "{rank} ")?;
            for file in 0..9 {
                let sq = Square::from_rank_file(rank, file).unwrap();
                f.write_str(if self.has(sq) { "x " } else { ". " })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

// -------- Direction enum --------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ORTHO: [Direction; 4] = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
}

// -------- File / rank masks --------

const fn file_mask(file: u8) -> BitBoard {
    let mut m = 0u128;
    let mut r = 0u8;
    while r < 10 {
        m |= 1u128 << (r * 9 + file) as u32;
        r += 1;
    }
    BitBoard(m)
}

const fn rank_mask(rank: u8) -> BitBoard {
    let mut m = 0u128;
    let mut f = 0u8;
    while f < 9 {
        m |= 1u128 << (rank * 9 + f) as u32;
        f += 1;
    }
    BitBoard(m)
}

pub const FILE_MASKS: [BitBoard; 9] = [
    file_mask(0),
    file_mask(1),
    file_mask(2),
    file_mask(3),
    file_mask(4),
    file_mask(5),
    file_mask(6),
    file_mask(7),
    file_mask(8),
];

pub const RANK_MASKS: [BitBoard; 10] = [
    rank_mask(0),
    rank_mask(1),
    rank_mask(2),
    rank_mask(3),
    rank_mask(4),
    rank_mask(5),
    rank_mask(6),
    rank_mask(7),
    rank_mask(8),
    rank_mask(9),
];

pub const FILE_A: BitBoard = FILE_MASKS[0];
pub const FILE_I: BitBoard = FILE_MASKS[8];
pub const NOT_FILE_A: BitBoard = BitBoard(BOARD_MASK & !FILE_A.0);
pub const NOT_FILE_I: BitBoard = BitBoard(BOARD_MASK & !FILE_I.0);

// -------- Board region masks --------

const fn build_red_palace() -> BitBoard {
    let mut m = 0u128;
    let mut r = 0u8;
    while r <= 2 {
        let mut f = 3u8;
        while f <= 5 {
            m |= 1u128 << (r * 9 + f) as u32;
            f += 1;
        }
        r += 1;
    }
    BitBoard(m)
}

const fn build_black_palace() -> BitBoard {
    let mut m = 0u128;
    let mut r = 7u8;
    while r <= 9 {
        let mut f = 3u8;
        while f <= 5 {
            m |= 1u128 << (r * 9 + f) as u32;
            f += 1;
        }
        r += 1;
    }
    BitBoard(m)
}

const fn build_half(red: bool) -> BitBoard {
    let mut m = 0u128;
    let (lo, hi) = if red { (0u8, 4u8) } else { (5u8, 9u8) };
    let mut r = lo;
    while r <= hi {
        let mut f = 0u8;
        while f < 9 {
            m |= 1u128 << (r * 9 + f) as u32;
            f += 1;
        }
        r += 1;
    }
    BitBoard(m)
}

pub const RED_PALACE: BitBoard = build_red_palace();
pub const BLACK_PALACE: BitBoard = build_black_palace();
pub const PALACES: [BitBoard; 2] = [RED_PALACE, BLACK_PALACE];
pub const RED_HALF: BitBoard = build_half(true);
pub const BLACK_HALF: BitBoard = build_half(false);
pub const HOME_HALVES: [BitBoard; 2] = [RED_HALF, BLACK_HALF];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_has_90_bits() {
        assert_eq!(BitBoard::FULL.popcount(), 90);
    }

    #[test]
    fn top_bits_always_zero() {
        // NOT(EMPTY) must have exactly 90 bits set.
        let all = !BitBoard::EMPTY;
        assert_eq!(all.popcount(), 90);
        // `<< 9` from the top rank must not overflow into unused bits.
        let topmost = BitBoard::FULL & RANK_MASKS[9];
        assert!((topmost.up().raw() >> 90) == 0);
    }

    #[test]
    fn file_masks_are_disjoint_and_cover() {
        let mut acc = BitBoard::EMPTY;
        for m in FILE_MASKS {
            assert_eq!(m.popcount(), 10);
            assert!((acc & m).is_empty());
            acc |= m;
        }
        assert_eq!(acc, BitBoard::FULL);
    }

    #[test]
    fn rank_masks_are_disjoint_and_cover() {
        let mut acc = BitBoard::EMPTY;
        for m in RANK_MASKS {
            assert_eq!(m.popcount(), 9);
            assert!((acc & m).is_empty());
            acc |= m;
        }
        assert_eq!(acc, BitBoard::FULL);
    }

    #[test]
    fn shifts_clear_wrap() {
        // Rightmost column shifted right should become empty in columns because of mask.
        let c = FILE_I;
        assert!(c.right().is_empty());
        assert!((FILE_A).left().is_empty());
    }

    #[test]
    fn iter_yields_every_set_bit() {
        let mut bb = BitBoard::EMPTY;
        let sqs = [3u8, 17, 40, 89];
        for s in sqs {
            bb |= BitBoard::from_square(Square::new_unchecked(s));
        }
        let collected: Vec<u8> = bb.iter().map(|s| s.raw()).collect();
        assert_eq!(collected, sqs);
    }

    #[test]
    fn palace_has_nine_squares() {
        assert_eq!(RED_PALACE.popcount(), 9);
        assert_eq!(BLACK_PALACE.popcount(), 9);
        assert!((RED_PALACE & BLACK_PALACE).is_empty());
    }

    #[test]
    fn halves_split_board() {
        assert_eq!(RED_HALF.popcount(), 45);
        assert_eq!(BLACK_HALF.popcount(), 45);
        assert_eq!(RED_HALF | BLACK_HALF, BitBoard::FULL);
    }
}
