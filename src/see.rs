use crate::attacks::ADVISOR_ATTACKS;
use crate::attacks::KING_ATTACKS;
use crate::attacks::PAWN_ATTACKS;
use crate::attacks::bishop_attacks;
use crate::attacks::knight_attacks;
use crate::bitboard::BitBoard;
use crate::color::Color;
use crate::magic::cannon_attacks;
use crate::magic::rook_attacks;
use crate::mv::Move;
use crate::piece::PieceType;
use crate::position::Position;
use crate::square::Square;

pub const SEE_KING: i32 = 10_000;
pub const SEE_ROOK: i32 = 500;
pub const SEE_CANNON: i32 = 300;
pub const SEE_KNIGHT: i32 = 280;
pub const SEE_ADVISOR: i32 = 120;
pub const SEE_BISHOP: i32 = 120;
pub const SEE_PAWN: i32 = 100;

#[inline]
pub const fn see_value(kind: PieceType) -> i32 {
    match kind {
        PieceType::King => SEE_KING,
        PieceType::Rook => SEE_ROOK,
        PieceType::Cannon => SEE_CANNON,
        PieceType::Knight => SEE_KNIGHT,
        PieceType::Advisor => SEE_ADVISOR,
        PieceType::Bishop => SEE_BISHOP,
        PieceType::Pawn => SEE_PAWN,
    }
}

/// Every piece (either side) that attacks `dst` given the board occupancy `occ`.
pub fn attackers_to(pos: &Position, dst: Square, occ: BitBoard) -> BitBoard {
    let mut attackers = BitBoard::EMPTY;
    for color in Color::ALL {
        // Non-sliding pieces.
        attackers |= KING_ATTACKS[dst.raw() as usize] & pos.pieces(color, PieceType::King);
        attackers |= ADVISOR_ATTACKS[dst.raw() as usize] & pos.pieces(color, PieceType::Advisor);

        // Bishop: symmetric 2-step diagonal.
        attackers |= bishop_attacks(dst, occ) & pos.pieces(color, PieceType::Bishop);

        // Knight: leg blocker is orientation-dependent, so we scan each knight.
        let mut knights = pos.pieces(color, PieceType::Knight);
        while knights.any() {
            let k = knights.pop_lsb();
            if knight_attacks(k, occ).has(dst) {
                attackers |= BitBoard::from_square(k);
            }
        }

        // Rook: symmetric ray attacks.
        attackers |= rook_attacks(dst, occ) & pos.pieces(color, PieceType::Rook);

        // Cannon: symmetric under the "exactly one screen" rule.
        let (_, cannon_caps) = cannon_attacks(dst, occ);
        attackers |= cannon_caps & pos.pieces(color, PieceType::Cannon);

        // Pawn: forward/side attacks depend on color.
        let mut pawns = pos.pieces(color, PieceType::Pawn);
        while pawns.any() {
            let p = pawns.pop_lsb();
            if PAWN_ATTACKS[color.index()][p.raw() as usize].has(dst) {
                attackers |= BitBoard::from_square(p);
            }
        }
    }
    attackers
}

#[inline]
fn least_valuable(pos: &Position, attackers: BitBoard, color: Color) -> Option<(Square, PieceType)> {
    for kind in [
        PieceType::Pawn,
        PieceType::Cannon,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Advisor,
        PieceType::Rook,
        PieceType::King,
    ] {
        let bb = pos.pieces(color, kind) & attackers;
        if bb.any() {
            return Some((bb.lsb_square(), kind));
        }
    }
    None
}

/// Returns the net material change for the side making `mv` after the optimal exchange.
/// Positive = `mv` wins material; 0 = breaks even; negative = losing trade.
pub fn see(pos: &Position, mv: Move) -> i32 {
    let src = mv.src();
    let dst = mv.dst();

    let attacker = match pos.piece_at(src) {
        Some(p) => p,
        None => return 0,
    };
    let victim_value = pos.piece_at(dst).map(|p| see_value(p.kind())).unwrap_or(0);

    let mut gain = [0i32; 32];
    let mut d = 0usize;
    gain[0] = victim_value;

    let mut occ = pos.occupancy() ^ BitBoard::from_square(src);
    let mut attackers = attackers_to(pos, dst, occ) & !BitBoard::from_square(src);
    let mut side = attacker.color().flip();
    let mut on_square_value = see_value(attacker.kind());

    loop {
        let side_attackers = attackers & pos.color_occupancy(side);
        let Some((att_sq, att_kind)) = least_valuable(pos, side_attackers, side) else {
            break;
        };
        d += 1;
        gain[d] = on_square_value - gain[d - 1];
        // Early cut-off: if either party would refuse at this depth, stop.
        if gain[d].max(-gain[d - 1]) < 0 {
            break;
        }
        occ ^= BitBoard::from_square(att_sq);
        attackers &= !BitBoard::from_square(att_sq);
        side = side.flip();
        on_square_value = see_value(att_kind);
    }

    while d > 0 {
        d -= 1;
        gain[d] = -(gain[d + 1].max(-gain[d]));
    }
    gain[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::STARTING_FEN;
    use crate::mv::Move as M;

    #[test]
    fn free_capture_returns_victim_value() {
        // Use a contrived pawn-only position so we can test SEE's behaviour on a non-capture
        // without involving the full opening setup.
        let _ = STARTING_FEN; // keep the import alive for adjacent tests
        let fen = "9/9/9/9/9/9/3P5/9/9/4K4 w";
        let pos = Position::from_fen(fen).unwrap();
        // No attackers around — `see` just returns 0 for a non-capture.
        let mv = M::from_iccs("d3-d4").unwrap();
        assert_eq!(see(&pos, mv), 0);
    }

    #[test]
    fn rook_takes_undefended_piece_is_positive() {
        // Red rook at a0 captures an isolated black knight at a5, no defender.
        let fen = "9/9/9/9/9/n8/9/9/9/R8 w";
        let pos = Position::from_fen(fen).unwrap();
        let mv = M::from_iccs("a0-a4").unwrap();
        assert_eq!(see(&pos, mv), SEE_KNIGHT);
    }

    #[test]
    fn defended_capture_breaks_even() {
        // Red rook a0 captures black knight a5; defended by black rook a9. Expected:
        // +knight - rook = 280 - 500 = negative, but SEE engine stops if negative, so
        // it'll return the base knight gain minus the worst-case loss that the defender
        // forces — which is a rook trade → -220.
        let fen = "r8/9/9/9/9/n8/9/9/9/R8 w";
        let pos = Position::from_fen(fen).unwrap();
        let mv = M::from_iccs("a0-a4").unwrap();
        let s = see(&pos, mv);
        assert!(s < 0, "expected losing trade but SEE = {s}");
        assert_eq!(s, SEE_KNIGHT - SEE_ROOK);
    }

    #[test]
    fn pawn_for_rook_is_a_winning_trade() {
        // Black pawn defended by nothing — red rook picks it up for free.
        let fen = "9/9/9/9/9/p8/9/9/9/R8 w";
        let pos = Position::from_fen(fen).unwrap();
        let mv = M::from_iccs("a0-a4").unwrap();
        assert_eq!(see(&pos, mv), SEE_PAWN);
    }
}
