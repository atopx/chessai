use std::fmt;

use crate::color::Color;
use crate::error::MoveParseError;

/// Packed square index `0..=89`. `rank * 9 + file`.
///
/// Rank `0` is red's back rank, rank `9` is black's back rank; file `0` is column `a`.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Square(u8);

impl Square {
    pub const COUNT: usize = 90;

    /// Caller promises `raw < 90`.
    ///
    /// Prefer [`Square::from_index`] or [`Square::from_rank_file`] at API boundaries.
    #[inline]
    pub const fn new_unchecked(raw: u8) -> Square {
        debug_assert!(raw < 90);
        Square(raw)
    }

    #[inline]
    pub const fn from_index(raw: u8) -> Option<Square> { if raw < 90 { Some(Square(raw)) } else { None } }

    #[inline]
    pub const fn from_rank_file(rank: u8, file: u8) -> Option<Square> {
        if rank < 10 && file < 9 { Some(Square(rank * 9 + file)) } else { None }
    }

    #[inline]
    pub const fn raw(self) -> u8 { self.0 }

    #[inline]
    pub const fn rank(self) -> u8 { self.0 / 9 }

    #[inline]
    pub const fn file(self) -> u8 { self.0 % 9 }

    /// Mirror across the vertical center (file reflection).
    #[inline]
    pub const fn mirror_file(self) -> Square { Square(self.rank() * 9 + (8 - self.file())) }

    /// Flip across the river (rank reflection). Used for black-side PST lookups.
    #[inline]
    pub const fn flip_rank(self) -> Square { Square((9 - self.rank()) * 9 + self.file()) }

    /// True if the square lies in its color's palace.
    #[inline]
    pub const fn is_in_palace(self, color: Color) -> bool {
        let f = self.file();
        if f < 3 || f > 5 {
            return false;
        }
        match color {
            Color::Red => self.rank() <= 2,
            Color::Black => self.rank() >= 7,
        }
    }

    /// Home half of the board, i.e. the side of the river owned by `color`.
    #[inline]
    pub const fn is_home_half(self, color: Color) -> bool {
        match color {
            Color::Red => self.rank() <= 4,
            Color::Black => self.rank() >= 5,
        }
    }

    /// Parses ICCS cells `a0..=i9`.
    pub fn from_iccs(s: &str) -> Result<Square, MoveParseError> {
        let b = s.as_bytes();
        if b.len() != 2 {
            return Err(MoveParseError::BadIccs(s.to_string()));
        }
        let file = match b[0] {
            c @ b'a'..=b'i' => c - b'a',
            c @ b'A'..=b'I' => c - b'A',
            _ => return Err(MoveParseError::BadIccs(s.to_string())),
        };
        let rank = match b[1] {
            c @ b'0'..=b'9' => c - b'0',
            _ => return Err(MoveParseError::BadIccs(s.to_string())),
        };
        Square::from_rank_file(rank, file).ok_or_else(|| MoveParseError::BadIccs(s.to_string()))
    }

    /// Emits the ICCS cell form, e.g. `b0`.
    #[inline]
    pub fn to_iccs(self) -> String {
        let file_ch = (b'a' + self.file()) as char;
        let rank_ch = (b'0' + self.rank()) as char;
        let mut s = String::with_capacity(2);
        s.push(file_ch);
        s.push(rank_ch);
        s
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.to_iccs()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_file_roundtrip() {
        for r in 0u8..10 {
            for f in 0u8..9 {
                let sq = Square::from_rank_file(r, f).unwrap();
                assert_eq!(sq.rank(), r);
                assert_eq!(sq.file(), f);
                assert_eq!(sq.raw(), r * 9 + f);
            }
        }
    }

    #[test]
    fn iccs_roundtrip() {
        for raw in 0..90u8 {
            let sq = Square::from_index(raw).unwrap();
            let s = sq.to_iccs();
            assert_eq!(Square::from_iccs(&s).unwrap(), sq);
        }
    }

    #[test]
    fn palace_membership() {
        assert!(Square::from_iccs("d0").unwrap().is_in_palace(Color::Red));
        assert!(Square::from_iccs("e1").unwrap().is_in_palace(Color::Red));
        assert!(!Square::from_iccs("a0").unwrap().is_in_palace(Color::Red));
        assert!(Square::from_iccs("e9").unwrap().is_in_palace(Color::Black));
        assert!(!Square::from_iccs("e0").unwrap().is_in_palace(Color::Black));
    }

    #[test]
    fn flip_involution() {
        for raw in 0..90u8 {
            let sq = Square::from_index(raw).unwrap();
            assert_eq!(sq.flip_rank().flip_rank(), sq);
            assert_eq!(sq.mirror_file().mirror_file(), sq);
        }
    }
}
