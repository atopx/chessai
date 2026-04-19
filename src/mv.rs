use std::fmt;
use std::str::FromStr;

use crate::error::MoveParseError;
use crate::square::Square;

#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Move(u16);

impl Move {
    pub const NULL: Move = Move(0);

    #[inline]
    pub const fn new(src: Square, dst: Square) -> Move { Move((src.raw() as u16) | ((dst.raw() as u16) << 7)) }

    #[inline]
    pub const fn from_raw(raw: u16) -> Move { Move(raw) }

    #[inline]
    pub const fn raw(self) -> u16 { self.0 }

    #[inline]
    pub const fn is_null(self) -> bool { self.0 == 0 }

    #[inline]
    pub const fn src(self) -> Square { Square::new_unchecked((self.0 & 0x7f) as u8) }

    #[inline]
    pub const fn dst(self) -> Square { Square::new_unchecked(((self.0 >> 7) & 0x7f) as u8) }

    /// Mirror the move horizontally (file reflection). Used by the opening book.
    #[inline]
    pub const fn mirror_file(self) -> Move { Move::new(self.src().mirror_file(), self.dst().mirror_file()) }

    /// Parses both `b2-e2` and `b2e2` (case-insensitive) ICCS forms.
    pub fn from_iccs(s: &str) -> Result<Move, MoveParseError> {
        let s = s.trim();
        let (a, b) = if let Some((a, b)) = s.split_once('-') {
            (a, b)
        } else if s.len() == 4 {
            (&s[..2], &s[2..])
        } else {
            return Err(MoveParseError::BadMove(s.to_string()));
        };
        let src = Square::from_iccs(a)?;
        let dst = Square::from_iccs(b)?;
        Ok(Move::new(src, dst))
    }

    #[inline]
    pub fn to_iccs(self) -> String {
        if self.is_null() {
            return "0000".to_string();
        }
        let mut s = String::with_capacity(5);
        s.push_str(&self.src().to_iccs());
        s.push('-');
        s.push_str(&self.dst().to_iccs());
        s
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.to_iccs()) }
}

impl FromStr for Move {
    type Err = MoveParseError;
    fn from_str(s: &str) -> Result<Move, MoveParseError> { Move::from_iccs(s) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iccs_roundtrip() {
        let m = Move::from_iccs("b2-e2").unwrap();
        assert_eq!(m.to_iccs(), "b2-e2");
        assert_eq!(Move::from_iccs("h2e2").unwrap().to_iccs(), "h2-e2");
        assert!(Move::from_iccs("invalid").is_err());
    }

    #[test]
    fn mirror_reflects_files() {
        let m = Move::from_iccs("b2-e2").unwrap();
        let mm = m.mirror_file();
        assert_eq!(mm.src().file(), 8 - m.src().file());
        assert_eq!(mm.dst().file(), 8 - m.dst().file());
    }
}
