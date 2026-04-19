use crate::movegen::MAX_MOVES;
use crate::movegen::MoveList;
use crate::movegen::generate_captures;
use crate::movegen::generate_quiets;
use crate::mv::Move;
use crate::piece::Piece;
use crate::piece::PieceType;
use crate::position::Position;
use crate::search::History;
use crate::search::MAX_PLY;
use crate::see::see;

/// Score sentinel: a quiet that turned out to match TT/killer/countermove and was already
/// yielded earlier — must be skipped on the quiet pass.
const ALREADY_YIELDED: i32 = i32::MIN;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Stage {
    TtMove,
    GenCaptures,
    GoodCaptures,
    Killer1,
    Killer2,
    Countermove,
    GenQuiets,
    Quiets,
    BadCaptures,
    Done,
}

pub struct MovePicker {
    stage: Stage,
    tt_move: Move,
    killer1: Move,
    killer2: Move,
    countermove: Move,

    captures: [Move; MAX_MOVES],
    capture_scores: [i32; MAX_MOVES],
    capture_len: usize,
    capture_idx: usize,

    bad_captures: [Move; MAX_MOVES],
    bad_capture_scores: [i32; MAX_MOVES],
    bad_capture_len: usize,
    bad_capture_idx: usize,

    quiets: [Move; MAX_MOVES],
    quiet_scores: [i32; MAX_MOVES],
    quiet_len: usize,
    quiet_idx: usize,
}

impl MovePicker {
    pub fn new(tt_move: Move, killers: [Move; 2], countermove: Move) -> Self {
        let stage = if tt_move.is_null() { Stage::GenCaptures } else { Stage::TtMove };
        MovePicker {
            stage,
            tt_move,
            killer1: killers[0],
            killer2: killers[1],
            countermove,
            captures: [Move::NULL; MAX_MOVES],
            capture_scores: [0; MAX_MOVES],
            capture_len: 0,
            capture_idx: 0,
            bad_captures: [Move::NULL; MAX_MOVES],
            bad_capture_scores: [0; MAX_MOVES],
            bad_capture_len: 0,
            bad_capture_idx: 0,
            quiets: [Move::NULL; MAX_MOVES],
            quiet_scores: [0; MAX_MOVES],
            quiet_len: 0,
            quiet_idx: 0,
        }
    }

    /// Construct a picker for the killer/countermove fields drawn from `history` at the
    /// given ply. Convenience wrapper.
    pub fn with_history(tt_move: Move, history: &History, ply: usize, countermove: Move) -> Self {
        let killers = if ply < MAX_PLY { history.killers[ply] } else { [Move::NULL; 2] };
        Self::new(tt_move, killers, countermove)
    }

    /// Yield the next move, or `None` when exhausted. `history` is consulted only when
    /// scoring the quiet pool — captures use MVV-LVA (then SEE on selection).
    pub fn next(&mut self, pos: &Position, history: &History) -> Option<Move> {
        loop {
            match self.stage {
                Stage::TtMove => {
                    self.stage = Stage::GenCaptures;
                    if is_pseudo_legal(pos, self.tt_move) {
                        return Some(self.tt_move);
                    }
                }
                Stage::GenCaptures => {
                    let mut ml = MoveList::new();
                    generate_captures(pos, &mut ml);
                    self.capture_len = ml.len();
                    for i in 0..self.capture_len {
                        let mv = ml[i];
                        self.captures[i] = mv;
                        self.capture_scores[i] = mvv_lva(pos, mv);
                    }
                    self.stage = Stage::GoodCaptures;
                }
                Stage::GoodCaptures => {
                    while self.capture_idx < self.capture_len {
                        let i = self.capture_idx;
                        let mut best = i;
                        let mut best_score = self.capture_scores[i];
                        for j in (i + 1)..self.capture_len {
                            if self.capture_scores[j] > best_score {
                                best_score = self.capture_scores[j];
                                best = j;
                            }
                        }
                        self.captures.swap(i, best);
                        self.capture_scores.swap(i, best);
                        let mv = self.captures[i];
                        self.capture_idx += 1;
                        if mv == self.tt_move {
                            continue;
                        }
                        // Lazy SEE: only computed for captures actually about to be tried.
                        let see_score = see(pos, mv);
                        if see_score >= 0 {
                            return Some(mv);
                        }
                        let bi = self.bad_capture_len;
                        self.bad_captures[bi] = mv;
                        self.bad_capture_scores[bi] = see_score;
                        self.bad_capture_len += 1;
                    }
                    self.stage = Stage::Killer1;
                }
                Stage::Killer1 => {
                    self.stage = Stage::Killer2;
                    let mv = self.killer1;
                    if !mv.is_null()
                        && mv != self.tt_move
                        && pos.piece_at(mv.dst()).is_none()
                        && is_pseudo_legal(pos, mv)
                    {
                        return Some(mv);
                    }
                }
                Stage::Killer2 => {
                    self.stage = Stage::Countermove;
                    let mv = self.killer2;
                    if !mv.is_null()
                        && mv != self.tt_move
                        && mv != self.killer1
                        && pos.piece_at(mv.dst()).is_none()
                        && is_pseudo_legal(pos, mv)
                    {
                        return Some(mv);
                    }
                }
                Stage::Countermove => {
                    self.stage = Stage::GenQuiets;
                    let mv = self.countermove;
                    if !mv.is_null()
                        && mv != self.tt_move
                        && mv != self.killer1
                        && mv != self.killer2
                        && pos.piece_at(mv.dst()).is_none()
                        && is_pseudo_legal(pos, mv)
                    {
                        return Some(mv);
                    }
                }
                Stage::GenQuiets => {
                    let mut ml = MoveList::new();
                    generate_quiets(pos, &mut ml);
                    self.quiet_len = ml.len();
                    for i in 0..self.quiet_len {
                        let mv = ml[i];
                        self.quiets[i] = mv;
                        self.quiet_scores[i] =
                            if mv == self.tt_move || mv == self.killer1 || mv == self.killer2 || mv == self.countermove
                            {
                                ALREADY_YIELDED
                            } else if let Some(p) = pos.piece_at(mv.src()) {
                                history.butterfly[p.index()][mv.dst().raw() as usize]
                            } else {
                                0
                            };
                    }
                    self.stage = Stage::Quiets;
                }
                Stage::Quiets => {
                    while self.quiet_idx < self.quiet_len {
                        let i = self.quiet_idx;
                        let mut best = i;
                        let mut best_score = self.quiet_scores[i];
                        for j in (i + 1)..self.quiet_len {
                            if self.quiet_scores[j] > best_score {
                                best_score = self.quiet_scores[j];
                                best = j;
                            }
                        }
                        self.quiets.swap(i, best);
                        self.quiet_scores.swap(i, best);
                        let mv = self.quiets[i];
                        let score = self.quiet_scores[i];
                        self.quiet_idx += 1;
                        if score == ALREADY_YIELDED {
                            continue;
                        }
                        return Some(mv);
                    }
                    self.stage = Stage::BadCaptures;
                }
                Stage::BadCaptures => {
                    if self.bad_capture_idx < self.bad_capture_len {
                        let i = self.bad_capture_idx;
                        let mut best = i;
                        let mut best_score = self.bad_capture_scores[i];
                        for j in (i + 1)..self.bad_capture_len {
                            if self.bad_capture_scores[j] > best_score {
                                best_score = self.bad_capture_scores[j];
                                best = j;
                            }
                        }
                        self.bad_captures.swap(i, best);
                        self.bad_capture_scores.swap(i, best);
                        let mv = self.bad_captures[i];
                        self.bad_capture_idx += 1;
                        return Some(mv);
                    }
                    self.stage = Stage::Done;
                }
                Stage::Done => return None,
            }
        }
    }
}

#[inline]
fn mvv_lva(pos: &Position, mv: Move) -> i32 {
    let victim = pos.piece_at(mv.dst()).map(|p| p.kind()).unwrap_or(PieceType::Pawn);
    let attacker = pos.piece_at(mv.src()).map(|p| p.kind()).unwrap_or(PieceType::Pawn);
    piece_weight(victim) * 100 - piece_weight(attacker)
}

#[inline]
fn piece_weight(kind: PieceType) -> i32 {
    match kind {
        PieceType::King => 10_000,
        PieceType::Rook => 500,
        PieceType::Cannon => 300,
        PieceType::Knight => 280,
        PieceType::Advisor => 120,
        PieceType::Bishop => 120,
        PieceType::Pawn => 100,
    }
}

/// Cheap pseudo-legality check for a TT/killer/countermove suggestion. Verifies that the
/// piece on `src` belongs to the side to move and that `dst` lies in its current attack
/// set. Does NOT verify king safety — the caller still discards moves that leave the king
/// in check after `make_move`.
pub fn is_pseudo_legal(pos: &Position, mv: Move) -> bool {
    use crate::attacks::ADVISOR_ATTACKS;
    use crate::attacks::KING_ATTACKS;
    use crate::attacks::PAWN_ATTACKS;
    use crate::attacks::bishop_attacks;
    use crate::attacks::knight_attacks;
    use crate::bitboard::BitBoard;
    use crate::bitboard::HOME_HALVES;
    use crate::magic::cannon_attacks;
    use crate::magic::rook_attacks;

    if mv.is_null() {
        return false;
    }
    let src = mv.src();
    let dst = mv.dst();
    if src == dst {
        return false;
    }
    let stm = pos.side_to_move();
    let piece: Piece = match pos.piece_at(src) {
        Some(p) if p.color() == stm => p,
        _ => return false,
    };
    if let Some(d) = pos.piece_at(dst)
        && d.color() == stm
    {
        return false;
    }

    let occ = pos.occupancy();
    let dst_bb = BitBoard::from_square(dst);
    let attacks = match piece.kind() {
        PieceType::King => KING_ATTACKS[src.raw() as usize],
        PieceType::Advisor => ADVISOR_ATTACKS[src.raw() as usize],
        PieceType::Bishop => bishop_attacks(src, occ) & HOME_HALVES[stm.index()],
        PieceType::Knight => knight_attacks(src, occ),
        PieceType::Rook => rook_attacks(src, occ),
        PieceType::Cannon => {
            let (quiet, captures) = cannon_attacks(src, occ);
            if pos.piece_at(dst).is_some() { captures } else { quiet }
        }
        PieceType::Pawn => PAWN_ATTACKS[stm.index()][src.raw() as usize],
    };
    (attacks & dst_bb).any()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::STARTING_FEN;
    use crate::movegen::generate_pseudo;

    #[test]
    fn picker_yields_complete_set_when_no_hints() {
        // With no TT / killer / countermove hints, the picker must enumerate every
        // pseudo-legal move exactly once.
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        let mut all = MoveList::new();
        generate_pseudo(&pos, &mut all);
        let expected: Vec<Move> = {
            let mut v: Vec<_> = all.as_slice().to_vec();
            v.sort_by_key(|m| m.raw());
            v
        };

        let mut picker = MovePicker::new(Move::NULL, [Move::NULL; 2], Move::NULL);
        let history = History::new();
        let mut yielded = Vec::new();
        while let Some(mv) = picker.next(&pos, &history) {
            yielded.push(mv);
        }
        let mut got = yielded.clone();
        got.sort_by_key(|m| m.raw());
        assert_eq!(got, expected);
    }

    #[test]
    fn picker_emits_tt_move_first() {
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        let mut all = MoveList::new();
        generate_pseudo(&pos, &mut all);
        let tt = all[3];
        let history = History::new();
        let mut picker = MovePicker::new(tt, [Move::NULL; 2], Move::NULL);
        assert_eq!(picker.next(&pos, &history), Some(tt));
    }

    #[test]
    fn picker_skips_invalid_tt_move() {
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        // A move with no piece on src — should be skipped.
        let bogus = Move::from_iccs("e5-e4").unwrap();
        let history = History::new();
        let mut picker = MovePicker::new(bogus, [Move::NULL; 2], Move::NULL);
        let first = picker.next(&pos, &history).expect("picker yields a real move");
        assert_ne!(first, bogus);
    }

    #[test]
    fn picker_does_not_yield_duplicates() {
        // TT, killers, countermove that all overlap with quiet generation must each appear
        // exactly once.
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        let mut all = MoveList::new();
        generate_pseudo(&pos, &mut all);
        let tt = all[0];
        let k1 = all[1];
        let k2 = all[2];
        let cm = all[3];
        let history = History::new();
        let mut picker = MovePicker::new(tt, [k1, k2], cm);
        let mut yielded = Vec::new();
        while let Some(mv) = picker.next(&pos, &history) {
            yielded.push(mv);
        }
        let unique: std::collections::HashSet<_> = yielded.iter().copied().collect();
        assert_eq!(unique.len(), yielded.len(), "picker yielded duplicates: {yielded:?}");
        assert_eq!(unique.len(), all.len());
    }

    #[test]
    fn pseudo_legal_recognizes_real_moves() {
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        let mut all = MoveList::new();
        generate_pseudo(&pos, &mut all);
        for mv in all.as_slice() {
            assert!(is_pseudo_legal(&pos, *mv), "{mv} should be pseudo-legal");
        }
    }

    #[test]
    fn pseudo_legal_rejects_garbage() {
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        // Empty source.
        assert!(!is_pseudo_legal(&pos, Move::from_iccs("e5-e4").unwrap()));
        // Source belongs to opponent.
        assert!(!is_pseudo_legal(&pos, Move::from_iccs("a9-a8").unwrap()));
        // Self-capture (rook tries to land on own pawn).
        assert!(!is_pseudo_legal(&pos, Move::from_iccs("a0-a3").unwrap()));
    }
}
