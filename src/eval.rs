use crate::color::Color;
use crate::piece::Piece;
use crate::position::Position;
use crate::square::Square;

pub const ADVANCED_VALUE: i32 = 3;
pub const MATE_VALUE: i32 = 10_000;
pub const BAN_VALUE: i32 = MATE_VALUE - 100;
pub const WIN_VALUE: i32 = MATE_VALUE - 200;
pub const NULL_OKAY_MARGIN: i32 = 200;
pub const NULL_SAFE_MARGIN: i32 = 400;
pub const DRAW_VALUE: i32 = 20;

/// Piece-square tables, one per piece type, indexed by **red-perspective** square
/// (`0..=89`, rank 0 = red back). For black evaluation, pass `sq.flip_rank()`.
pub const PST: [[i16; 90]; 7] = {
    const KING_OR_PAWN: [i16; 90] = [
        // rank 0 (red back)
        0, 0, 0, 11, 15, 11, 0, 0, 0, // rank 1 (red palace row)
        0, 0, 0, 2, 2, 2, 0, 0, 0, // rank 2 (red palace row)
        0, 0, 0, 1, 1, 1, 0, 0, 0, // rank 3 (red pawn start)
        7, 0, 7, 0, 15, 0, 7, 0, 7, // rank 4
        7, 0, 13, 0, 16, 0, 13, 0, 7, // rank 5 (just across river)
        14, 18, 20, 27, 29, 27, 20, 18, 14, // rank 6
        19, 23, 27, 29, 30, 29, 27, 23, 19, // rank 7
        19, 24, 32, 37, 37, 37, 32, 24, 19, // rank 8
        19, 24, 34, 42, 44, 42, 34, 24, 19, // rank 9 (black back)
        9, 9, 9, 11, 13, 11, 9, 9, 9,
    ];

    const ADVISOR_OR_BISHOP: [i16; 90] = [
        // rank 0
        0, 0, 20, 20, 0, 20, 20, 0, 0, // rank 1
        0, 0, 0, 0, 23, 0, 0, 0, 0, // rank 2
        18, 0, 0, 20, 23, 20, 0, 0, 18, // rank 3
        0, 0, 0, 0, 0, 0, 0, 0, 0, // rank 4
        0, 0, 20, 0, 0, 0, 20, 0, 0, // rank 5..=9 unused for advisor / bishop (they stay home).
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0,
    ];

    const KNIGHT: [i16; 90] = [
        88, 85, 90, 88, 90, 88, 90, 85, 88, // rank 0
        85, 90, 92, 93, 78, 93, 92, 90, 85, // rank 1
        93, 92, 94, 95, 92, 95, 94, 92, 93, // rank 2
        92, 94, 98, 95, 98, 95, 98, 94, 92, // rank 3
        90, 98, 101, 102, 103, 102, 101, 98, 90, // rank 4
        90, 100, 99, 103, 104, 103, 99, 100, 90, // rank 5
        93, 108, 100, 107, 100, 107, 100, 108, 93, // rank 6
        92, 98, 99, 103, 99, 103, 99, 98, 92, // rank 7
        90, 96, 103, 97, 94, 97, 103, 96, 90, // rank 8
        90, 90, 90, 96, 90, 96, 90, 90, 90, // rank 9
    ];

    const ROOK: [i16; 90] = [
        194, 206, 204, 212, 200, 212, 204, 206, 194, // rank 0
        200, 208, 206, 212, 200, 212, 206, 208, 200, // rank 1
        198, 208, 204, 212, 212, 212, 204, 208, 198, // rank 2
        204, 209, 204, 212, 214, 212, 204, 209, 204, // rank 3
        208, 212, 212, 214, 215, 214, 212, 212, 208, // rank 4
        208, 211, 211, 214, 215, 214, 211, 211, 208, // rank 5
        206, 213, 213, 216, 216, 216, 213, 213, 206, // rank 6
        206, 208, 207, 214, 216, 214, 207, 208, 206, // rank 7
        206, 212, 209, 216, 233, 216, 209, 212, 206, // rank 8
        206, 208, 207, 213, 214, 213, 207, 208, 206, // rank 9
    ];

    const CANNON: [i16; 90] = [
        96, 96, 97, 99, 99, 99, 97, 96, 96, // rank 0
        96, 97, 98, 98, 98, 98, 98, 97, 96, // rank 1
        97, 96, 100, 99, 101, 99, 100, 96, 97, // rank 2
        96, 96, 96, 96, 96, 96, 96, 96, 96, // rank 3
        95, 96, 99, 96, 100, 96, 99, 96, 95, // rank 4
        96, 96, 96, 96, 100, 96, 96, 96, 96, // rank 5
        96, 99, 99, 98, 100, 98, 99, 99, 96, // rank 6
        97, 97, 96, 91, 92, 91, 96, 97, 97, // rank 7
        98, 98, 96, 92, 89, 92, 96, 98, 98, // rank 8
        100, 100, 96, 91, 90, 91, 96, 100, 100, // rank 9
    ];

    [KING_OR_PAWN, ADVISOR_OR_BISHOP, ADVISOR_OR_BISHOP, KNIGHT, ROOK, CANNON, KING_OR_PAWN]
};

/// Look up the PST contribution of a single piece on a single square. Black uses the
/// rank-flipped index, giving a natural symmetry.
#[inline]
pub const fn psq_value(piece: Piece, sq: Square) -> i16 {
    let idx = match piece.color() {
        Color::Red => sq.raw() as usize,
        Color::Black => sq.flip_rank().raw() as usize,
    };
    PST[piece.kind().index()][idx]
}

/// Material/positional score from the side-to-move's perspective, in centipawn units.
#[inline]
pub fn evaluate(pos: &Position) -> i32 {
    let red_score = pos.psq_score() + ADVANCED_VALUE;
    match pos.side_to_move() {
        Color::Red => red_score,
        Color::Black => -red_score,
    }
}

/// Distance-to-mate-aware draw sentinel, switched by ply parity so repetition favors the
/// side with the edge instead of letting one side force a cycle arbitrarily.
#[inline]
pub fn draw_value(ply_from_root: u32) -> i32 { if ply_from_root & 1 == 0 { -DRAW_VALUE } else { DRAW_VALUE } }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::STARTING_FEN;
    use crate::piece::PieceType;

    #[test]
    fn startpos_is_balanced() {
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        // A symmetric starting position should evaluate to exactly the side-to-move bonus.
        let s = evaluate(&pos);
        assert!(s.abs() <= ADVANCED_VALUE + 1, "startpos score = {s}");
    }

    #[test]
    fn rook_initial_psq_matches() {
        let sq = Square::from_iccs("a0").unwrap();
        let piece = Piece::new(Color::Red, PieceType::Rook);
        assert_eq!(psq_value(piece, sq), 194);
    }
}
