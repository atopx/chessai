use std::ops::Index;

use crate::attacks::ADVISOR_ATTACKS;
use crate::attacks::KING_ATTACKS;
use crate::attacks::PAWN_ATTACKS;
use crate::attacks::bishop_attacks;
use crate::attacks::knight_attacks;
use crate::bitboard::BitBoard;
use crate::bitboard::HOME_HALVES;
use crate::magic::cannon_attacks;
use crate::magic::rook_attacks;
use crate::mv::Move;
use crate::piece::PieceType;
use crate::position::Position;
use crate::square::Square;

pub(crate) const MAX_MOVES: usize = 128;

/// Fixed-capacity move list. Consumers treat it as a slice via `Deref`/`Index`.
#[derive(Clone)]
pub(crate) struct MoveList {
    moves: [Move; MAX_MOVES],
    len: u8,
}

impl MoveList {
    #[inline]
    pub(crate) const fn new() -> Self { MoveList { moves: [Move::NULL; MAX_MOVES], len: 0 } }

    #[inline]
    pub(crate) fn len(&self) -> usize { self.len as usize }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[Move] { &self.moves[..self.len as usize] }

    #[inline]
    pub(crate) fn iter(&self) -> std::slice::Iter<'_, Move> { self.as_slice().iter() }

    #[inline]
    pub(crate) fn push(&mut self, mv: Move) {
        debug_assert!((self.len as usize) < MAX_MOVES);
        self.moves[self.len as usize] = mv;
        self.len += 1;
    }

    #[inline]
    pub(crate) fn clear(&mut self) { self.len = 0; }

    #[inline]
    fn push_from_bb(&mut self, src: Square, mut bb: BitBoard) {
        while bb.any() {
            let dst = bb.pop_lsb();
            self.push(Move::new(src, dst));
        }
    }
}

impl Default for MoveList {
    fn default() -> Self { MoveList::new() }
}

impl Index<usize> for MoveList {
    type Output = Move;
    fn index(&self, i: usize) -> &Move { &self.as_slice()[i] }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;
    fn into_iter(self) -> Self::IntoIter { self.iter() }
}

// ======================================================================
// Pseudo-legal generation
// ======================================================================

pub(crate) fn generate_pseudo(pos: &Position, out: &mut MoveList) {
    out.clear();
    let stm = pos.side_to_move();
    let own = pos.color_occupancy(stm);
    let occ = pos.occupancy();
    let opp = occ - own;

    // King — confined to its palace.
    for src in pos.pieces(stm, PieceType::King) {
        let bb = KING_ATTACKS[src.raw() as usize] - own;
        out.push_from_bb(src, bb);
    }

    // Advisor — confined to palace.
    for src in pos.pieces(stm, PieceType::Advisor) {
        let bb = ADVISOR_ATTACKS[src.raw() as usize] - own;
        out.push_from_bb(src, bb);
    }

    // Bishop — stays on home half, eye must be empty (encoded in attack table).
    let home = HOME_HALVES[stm.index()];
    for src in pos.pieces(stm, PieceType::Bishop) {
        let bb = bishop_attacks(src, occ) & (home - own);
        out.push_from_bb(src, bb);
    }

    // Knight — leg blockers handled inside `knight_attacks`.
    for src in pos.pieces(stm, PieceType::Knight) {
        let bb = knight_attacks(src, occ) - own;
        out.push_from_bb(src, bb);
    }

    // Rook — straight lines; cannot capture own.
    for src in pos.pieces(stm, PieceType::Rook) {
        let bb = rook_attacks(src, occ) - own;
        out.push_from_bb(src, bb);
    }

    // Cannon — split quiet and capture components.
    for src in pos.pieces(stm, PieceType::Cannon) {
        let (quiet, captures) = cannon_attacks(src, occ);
        let mut bb = quiet | (captures & opp);
        while bb.any() {
            let dst = bb.pop_lsb();
            out.push(Move::new(src, dst));
        }
    }

    // Pawn — forward + (post-river) sideways, cannot capture own.
    let pawn_att_table = &PAWN_ATTACKS[stm.index()];
    for src in pos.pieces(stm, PieceType::Pawn) {
        let bb = pawn_att_table[src.raw() as usize] - own;
        out.push_from_bb(src, bb);
    }
}

/// Pseudo-legal moves that land on an enemy piece (captures only). Used by the staged
/// move picker so that an early TT-cutoff doesn't pay the cost of generating quiets.
pub(crate) fn generate_captures(pos: &Position, out: &mut MoveList) {
    out.clear();
    let stm = pos.side_to_move();
    let own = pos.color_occupancy(stm);
    let occ = pos.occupancy();
    let opp = occ - own;

    for src in pos.pieces(stm, PieceType::King) {
        let bb = KING_ATTACKS[src.raw() as usize] & opp;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Advisor) {
        let bb = ADVISOR_ATTACKS[src.raw() as usize] & opp;
        out.push_from_bb(src, bb);
    }
    let home = HOME_HALVES[stm.index()];
    for src in pos.pieces(stm, PieceType::Bishop) {
        let bb = bishop_attacks(src, occ) & home & opp;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Knight) {
        let bb = knight_attacks(src, occ) & opp;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Rook) {
        let bb = rook_attacks(src, occ) & opp;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Cannon) {
        let (_quiet, captures) = cannon_attacks(src, occ);
        let bb = captures & opp;
        out.push_from_bb(src, bb);
    }
    let pawn_att_table = &PAWN_ATTACKS[stm.index()];
    for src in pos.pieces(stm, PieceType::Pawn) {
        let bb = pawn_att_table[src.raw() as usize] & opp;
        out.push_from_bb(src, bb);
    }
}

/// Pseudo-legal moves that land on an empty square (non-captures). Complement of
/// `generate_captures`; their union equals `generate_pseudo`.
pub(crate) fn generate_quiets(pos: &Position, out: &mut MoveList) {
    out.clear();
    let stm = pos.side_to_move();
    let own = pos.color_occupancy(stm);
    let occ = pos.occupancy();
    let empties = !occ;

    for src in pos.pieces(stm, PieceType::King) {
        let bb = KING_ATTACKS[src.raw() as usize] & empties;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Advisor) {
        let bb = ADVISOR_ATTACKS[src.raw() as usize] & empties;
        out.push_from_bb(src, bb);
    }
    let home = HOME_HALVES[stm.index()];
    for src in pos.pieces(stm, PieceType::Bishop) {
        let bb = bishop_attacks(src, occ) & home & empties;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Knight) {
        let bb = knight_attacks(src, occ) & empties;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Rook) {
        let bb = rook_attacks(src, occ) & empties;
        out.push_from_bb(src, bb);
    }
    for src in pos.pieces(stm, PieceType::Cannon) {
        // Cannon's quiet component is by construction non-overlapping with `occ`.
        let (quiet, _captures) = cannon_attacks(src, occ);
        out.push_from_bb(src, quiet);
    }
    let pawn_att_table = &PAWN_ATTACKS[stm.index()];
    for src in pos.pieces(stm, PieceType::Pawn) {
        let bb = pawn_att_table[src.raw() as usize] & empties;
        out.push_from_bb(src, bb);
    }
    let _ = own; // unused — every "& empties" implicitly excludes own pieces
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::STARTING_FEN;

    #[test]
    fn startpos_has_44_pseudo_legal_moves() {
        let p = Position::from_fen(STARTING_FEN).unwrap();
        let mut ml = MoveList::new();
        generate_pseudo(&p, &mut ml);
        assert_eq!(ml.len(), 44, "moves generated: {}", ml.len());
    }

    #[test]
    fn captures_plus_quiets_equals_pseudo() {
        // Across a handful of real positions, the staged generators must partition the
        // pseudo-legal move set exactly. Same set, no overlap, no missing moves.
        let fens = [
            STARTING_FEN,
            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w",
            "r1bakabr1/9/1cn1c1n2/p1p3p1p/4p4/2P6/P3P1P1P/2N1C1N2/9/R1BAKABR1 w",
            "5k3/4P4/3a5/9/9/9/9/9/9/4K4 w",
        ];
        for fen in fens {
            let pos = Position::from_fen(fen).unwrap();
            let mut all = MoveList::new();
            generate_pseudo(&pos, &mut all);
            let mut caps = MoveList::new();
            generate_captures(&pos, &mut caps);
            let mut quiets = MoveList::new();
            generate_quiets(&pos, &mut quiets);

            assert_eq!(caps.len() + quiets.len(), all.len(), "fen={fen}");

            // Verify no overlap and union matches.
            let mut combined: Vec<_> = caps.as_slice().iter().chain(quiets.as_slice().iter()).copied().collect();
            combined.sort_by_key(|m| m.raw());
            let mut expected: Vec<_> = all.as_slice().to_vec();
            expected.sort_by_key(|m| m.raw());
            assert_eq!(combined, expected, "fen={fen}");
        }
    }

    #[test]
    fn captures_target_only_opponent() {
        let fen = "r1bakabr1/9/1cn1c1n2/p1p3p1p/4p4/2P6/P3P1P1P/2N1C1N2/9/R1BAKABR1 w";
        let pos = Position::from_fen(fen).unwrap();
        let mut caps = MoveList::new();
        generate_captures(&pos, &mut caps);
        for mv in caps.as_slice() {
            let victim = pos.piece_at(mv.dst()).expect("capture must land on a piece");
            assert_ne!(victim.color(), pos.side_to_move(), "captured own piece");
        }
        assert!(caps.len() > 0, "this position has at least one capture");
    }

    #[test]
    fn quiets_target_only_empties() {
        let fen = "r1bakabr1/9/1cn1c1n2/p1p3p1p/4p4/2P6/P3P1P1P/2N1C1N2/9/R1BAKABR1 w";
        let pos = Position::from_fen(fen).unwrap();
        let mut quiets = MoveList::new();
        generate_quiets(&pos, &mut quiets);
        for mv in quiets.as_slice() {
            assert!(pos.piece_at(mv.dst()).is_none(), "quiet move {mv} landed on a piece");
        }
    }
}
