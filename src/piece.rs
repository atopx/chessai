use std::fmt;

use crate::color::Color;

/// Seven piece types following Chinese chess nomenclature.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum PieceType {
    King = 0,
    Advisor = 1,
    Bishop = 2,
    Knight = 3,
    Rook = 4,
    Cannon = 5,
    Pawn = 6,
}

impl PieceType {
    pub const COUNT: usize = 7;
    pub const ALL: [PieceType; 7] = [
        PieceType::King,
        PieceType::Advisor,
        PieceType::Bishop,
        PieceType::Knight,
        PieceType::Rook,
        PieceType::Cannon,
        PieceType::Pawn,
    ];

    #[inline]
    pub const fn index(self) -> usize { self as usize }

    #[inline]
    pub const fn from_index(i: usize) -> Option<PieceType> {
        Some(match i {
            0 => PieceType::King,
            1 => PieceType::Advisor,
            2 => PieceType::Bishop,
            3 => PieceType::Knight,
            4 => PieceType::Rook,
            5 => PieceType::Cannon,
            6 => PieceType::Pawn,
            _ => return None,
        })
    }

    /// Uppercase FEN letter (always capital; callers lowercase for black).
    #[inline]
    pub const fn fen_char(self) -> char {
        match self {
            PieceType::King => 'K',
            PieceType::Advisor => 'A',
            PieceType::Bishop => 'B',
            PieceType::Knight => 'N',
            PieceType::Rook => 'R',
            PieceType::Cannon => 'C',
            PieceType::Pawn => 'P',
        }
    }

    /// Accepts the FEN letter regardless of case and the alternative letters used by some
    /// dialects (`E` for bishop, `H` for knight).
    pub const fn from_fen_char(c: char) -> Option<PieceType> {
        let up = c.to_ascii_uppercase();
        Some(match up {
            'K' => PieceType::King,
            'A' => PieceType::Advisor,
            'B' | 'E' => PieceType::Bishop,
            'N' | 'H' => PieceType::Knight,
            'R' => PieceType::Rook,
            'C' => PieceType::Cannon,
            'P' => PieceType::Pawn,
            _ => return None,
        })
    }
}

/// Colored piece packed into a single byte: `(color_index << 3) | piece_type_index`.
///
/// Value `0xFF` denotes an empty square when stored in a mailbox; use `Option<Piece>`
/// at API boundaries rather than the raw byte.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct Piece(u8);

impl Piece {
    pub const COUNT: usize = 14;

    #[inline]
    pub const fn new(color: Color, kind: PieceType) -> Piece { Piece(((color as u8) << 3) | kind as u8) }

    #[inline]
    pub const fn raw(self) -> u8 { self.0 }

    /// Dense index in `0..14` suitable for per-piece tables.
    #[inline]
    pub const fn index(self) -> usize {
        let color = (self.0 >> 3) as usize;
        let kind = (self.0 & 7) as usize;
        color * 7 + kind
    }

    #[inline]
    pub const fn color(self) -> Color {
        match self.0 >> 3 {
            0 => Color::Red,
            _ => Color::Black,
        }
    }

    #[inline]
    pub const fn kind(self) -> PieceType {
        // SAFETY of `unwrap`: the lower 3 bits of every constructed Piece are in 0..=6.
        match PieceType::from_index((self.0 & 7) as usize) {
            Some(k) => k,
            None => PieceType::Pawn,
        }
    }

    #[inline]
    pub const fn from_index(i: usize) -> Piece {
        let color = Color::from_index(i / 7);
        let kind = match PieceType::from_index(i % 7) {
            Some(k) => k,
            None => PieceType::Pawn,
        };
        Piece::new(color, kind)
    }

    pub const fn fen_char(self) -> char {
        let letter = self.kind().fen_char();
        match self.color() {
            Color::Red => letter,
            Color::Black => letter.to_ascii_lowercase(),
        }
    }

    pub const fn from_fen_char(c: char) -> Option<Piece> {
        let color = if c.is_ascii_uppercase() { Color::Red } else { Color::Black };
        match PieceType::from_fen_char(c) {
            Some(k) => Some(Piece::new(color, k)),
            None => None,
        }
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match (self.color(), self.kind()) {
            (Color::Red, PieceType::King) => "帥",
            (Color::Red, PieceType::Advisor) => "仕",
            (Color::Red, PieceType::Bishop) => "相",
            (Color::Red, PieceType::Knight) => "馬",
            (Color::Red, PieceType::Rook) => "車",
            (Color::Red, PieceType::Cannon) => "炮",
            (Color::Red, PieceType::Pawn) => "兵",
            (Color::Black, PieceType::King) => "將",
            (Color::Black, PieceType::Advisor) => "士",
            (Color::Black, PieceType::Bishop) => "象",
            (Color::Black, PieceType::Knight) => "馬",
            (Color::Black, PieceType::Rook) => "車",
            (Color::Black, PieceType::Cannon) => "砲",
            (Color::Black, PieceType::Pawn) => "卒",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_index_roundtrip() {
        for i in 0..Piece::COUNT {
            let p = Piece::from_index(i);
            assert_eq!(p.index(), i, "{i}: {p:?}");
        }
    }

    #[test]
    fn fen_char_roundtrip() {
        for c in ['K', 'A', 'B', 'N', 'R', 'C', 'P', 'k', 'a', 'b', 'n', 'r', 'c', 'p'] {
            let p = Piece::from_fen_char(c).unwrap();
            assert_eq!(p.fen_char(), c);
        }
    }
}
