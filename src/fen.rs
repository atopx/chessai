use crate::color::Color;
use crate::error::ChessAIError;
use crate::piece::Piece;
use crate::position::Position;
use crate::square::Square;

pub const STARTING_FEN: &str = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";

impl Position {
    pub fn from_fen(fen: &str) -> Result<Position, ChessAIError> {
        let fen = fen.trim();
        if fen.is_empty() {
            return Err(ChessAIError::EmptyFen);
        }

        let mut pos = Position::empty();
        let (board_part, rest) = match fen.split_once(' ') {
            Some((b, r)) => (b, r),
            None => (fen, ""),
        };

        let mut board_rank: i8 = 9;
        let mut file: u8 = 0;
        let bytes = board_part.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            let c = b as char;
            match c {
                '/' => {
                    if file > 9 {
                        return Err(ChessAIError::FenRankOverflow { rank: board_rank as u8 });
                    }
                    board_rank -= 1;
                    file = 0;
                }
                '1'..='9' => {
                    file += b - b'0';
                    if file > 9 {
                        return Err(ChessAIError::FenRankOverflow { rank: board_rank as u8 });
                    }
                }
                'A'..='Z' | 'a'..='z' => {
                    if file >= 9 {
                        return Err(ChessAIError::FenRankOverflow { rank: board_rank as u8 });
                    }
                    if board_rank < 0 {
                        return Err(ChessAIError::InvalidFenChar { c, index: i });
                    }
                    let piece = Piece::from_fen_char(c).ok_or(ChessAIError::InvalidFenChar { c, index: i })?;
                    let sq = Square::from_rank_file(board_rank as u8, file)
                        .ok_or(ChessAIError::InvalidFenChar { c, index: i })?;
                    pos.put(sq, piece);
                    file += 1;
                }
                _ => return Err(ChessAIError::InvalidFenChar { c, index: i }),
            }
        }

        if board_rank > 0 {
            return Err(ChessAIError::FenRankUnderflow);
        }

        let mut it = rest.split_ascii_whitespace();
        if let Some(stm) = it.next() {
            let side = match stm {
                "w" | "r" | "W" | "R" => Color::Red,
                "b" | "B" => Color::Black,
                _ => {
                    let marker = stm.chars().next().unwrap_or('?');
                    return Err(ChessAIError::BadSideToMove { marker });
                }
            };
            pos.set_side_to_move(side);
        }

        Ok(pos)
    }

    /// Serialize the board + side-to-move portion.
    pub fn to_fen(&self) -> String {
        let mut out = String::with_capacity(80);
        for board_rank in (0..=9).rev() {
            let mut empty = 0u8;
            for file in 0..9 {
                let sq = Square::from_rank_file(board_rank, file).unwrap();
                match self.piece_at(sq) {
                    None => empty += 1,
                    Some(p) => {
                        if empty > 0 {
                            out.push((b'0' + empty) as char);
                            empty = 0;
                        }
                        out.push(p.fen_char());
                    }
                }
            }
            if empty > 0 {
                out.push((b'0' + empty) as char);
            }
            if board_rank > 0 {
                out.push('/');
            }
        }
        out.push(' ');
        out.push(match self.side_to_move() {
            Color::Red => 'w',
            Color::Black => 'b',
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_fen_roundtrip() {
        let p = Position::from_fen(STARTING_FEN).unwrap();
        let out = p.to_fen();
        // FEN prefix up to "w" should match.
        let expected = STARTING_FEN.split(' ').take(2).collect::<Vec<_>>().join(" ");
        assert_eq!(out, expected);
    }

    #[test]
    fn black_to_move_flag() {
        let p = Position::from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR b").unwrap();
        assert_eq!(p.side_to_move(), Color::Black);
    }

    #[test]
    fn invalid_char_rejected() {
        assert!(matches!(Position::from_fen("zzz"), Err(ChessAIError::InvalidFenChar { .. })));
    }
}
