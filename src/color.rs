use std::fmt;

/// Chinese chess colors. Red moves first.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Red = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::Red, Color::Black];

    #[inline]
    pub const fn index(self) -> usize { self as usize }

    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::Red => Color::Black,
            Color::Black => Color::Red,
        }
    }

    #[inline]
    pub const fn from_index(i: usize) -> Color {
        match i {
            0 => Color::Red,
            _ => Color::Black,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Color::Red => "red",
            Color::Black => "black",
        })
    }
}
