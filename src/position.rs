use crate::attacks::bishop_attacks;
use crate::attacks::knight_attacks;
use crate::bitboard::BitBoard;
use crate::bitboard::HOME_HALVES;
use crate::color::Color;
use crate::eval::psq_value;
use crate::magic::cannon_attacks;
use crate::magic::rook_attacks;
use crate::mv::Move;
use crate::piece::Piece;
use crate::piece::PieceType;
use crate::square::Square;
use crate::zobrist::ZOBRIST;

/// Snapshot needed to undo a single ply.
#[derive(Copy, Clone, Debug, Default)]
pub struct UndoInfo {
    pub captured: Option<Piece>,
    pub key_before: u64,
    pub lock_before: u32,
}

/// Snapshot for a null-move (pass).
#[derive(Copy, Clone, Debug, Default)]
pub struct NullUndo {
    pub key_before: u64,
    pub lock_before: u32,
}

#[derive(Clone, Debug)]
pub struct Position {
    /// Occupancy per color.
    color_bb: [BitBoard; 2],
    /// Occupancy per piece type, colorless; `piece_bb[kind] = red_pieces | black_pieces`.
    piece_bb: [BitBoard; PieceType::COUNT],
    /// All occupied squares.
    occ: BitBoard,

    /// Mailbox for O(1) square → piece lookup. `None` encoded as `u8::MAX`.
    mailbox: [u8; Square::COUNT],

    /// Side to move.
    stm: Color,

    /// TT key and book-compat lock.
    key: u64,
    lock: u32,

    /// Incremental material score per color.
    material: [i32; 2],
    /// Incremental piece-square score per color (red's perspective = `psq[0] - psq[1]`).
    psq: [i32; 2],
}

const EMPTY_MAILBOX_SLOT: u8 = u8::MAX;

impl Default for Position {
    fn default() -> Self { Position::empty() }
}

impl Position {
    pub const fn empty() -> Self {
        Position {
            color_bb: [BitBoard::EMPTY; 2],
            piece_bb: [BitBoard::EMPTY; PieceType::COUNT],
            occ: BitBoard::EMPTY,
            mailbox: [EMPTY_MAILBOX_SLOT; Square::COUNT],
            stm: Color::Red,
            key: 0,
            lock: 0,
            material: [0, 0],
            psq: [0, 0],
        }
    }

    // --------------------------------------------------------------------
    // Accessors
    // --------------------------------------------------------------------

    #[inline]
    pub fn side_to_move(&self) -> Color { self.stm }

    #[inline]
    pub fn occupancy(&self) -> BitBoard { self.occ }

    #[inline]
    pub fn color_occupancy(&self, color: Color) -> BitBoard { self.color_bb[color.index()] }

    #[inline]
    pub fn pieces(&self, color: Color, kind: PieceType) -> BitBoard {
        self.piece_bb[kind.index()] & self.color_bb[color.index()]
    }

    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        let raw = self.mailbox[sq.raw() as usize];
        if raw == EMPTY_MAILBOX_SLOT { None } else { Some(Piece::from_index(raw as usize)) }
    }

    #[inline]
    pub fn zobrist_key(&self) -> u64 { self.key }

    #[inline]
    pub fn zobrist_lock(&self) -> u32 { self.lock }

    #[inline]
    pub fn king_square(&self, color: Color) -> Option<Square> {
        let bb = self.pieces(color, PieceType::King);
        if bb.is_empty() { None } else { Some(bb.lsb_square()) }
    }

    // --------------------------------------------------------------------
    // Placement primitives (incremental updates for every derived field)
    // --------------------------------------------------------------------

    pub fn put(&mut self, sq: Square, piece: Piece) {
        debug_assert!(self.mailbox[sq.raw() as usize] == EMPTY_MAILBOX_SLOT);
        let bb = BitBoard::from_square(sq);
        self.color_bb[piece.color().index()] |= bb;
        self.piece_bb[piece.kind().index()] |= bb;
        self.occ |= bb;
        self.mailbox[sq.raw() as usize] = piece.index() as u8;

        let z = &*ZOBRIST;
        self.key ^= z.key_piece[piece.index()][sq.raw() as usize];
        self.lock ^= z.lock_piece[piece.index()][sq.raw() as usize];

        self.material[piece.color().index()] += piece_value(piece.kind());
        self.psq[piece.color().index()] += psq_value(piece, sq) as i32;
    }

    pub fn remove(&mut self, sq: Square) -> Piece {
        let raw = self.mailbox[sq.raw() as usize];
        debug_assert!(raw != EMPTY_MAILBOX_SLOT);
        let piece = Piece::from_index(raw as usize);
        let bb = BitBoard::from_square(sq);
        self.color_bb[piece.color().index()] ^= bb;
        self.piece_bb[piece.kind().index()] ^= bb;
        self.occ ^= bb;
        self.mailbox[sq.raw() as usize] = EMPTY_MAILBOX_SLOT;

        let z = &*ZOBRIST;
        self.key ^= z.key_piece[piece.index()][sq.raw() as usize];
        self.lock ^= z.lock_piece[piece.index()][sq.raw() as usize];

        self.material[piece.color().index()] -= piece_value(piece.kind());
        self.psq[piece.color().index()] -= psq_value(piece, sq) as i32;
        piece
    }

    pub fn flip_side_to_move(&mut self) {
        self.stm = self.stm.flip();
        let z = &*ZOBRIST;
        self.key ^= z.key_stm;
        self.lock ^= z.lock_stm;
    }

    pub(crate) fn set_side_to_move(&mut self, stm: Color) {
        if self.stm != stm {
            self.flip_side_to_move();
        }
    }

    // --------------------------------------------------------------------
    // make_move / undo_move
    // --------------------------------------------------------------------

    /// Apply `mv` to the board. Returns the undo information; caller must pass it back to
    /// `undo_move`. Caller guarantees the move is *pseudo-legal* — king-safety is checked
    /// separately via `is_in_check` after the move.
    pub fn make_move(&mut self, mv: Move) -> UndoInfo {
        let src = mv.src();
        let dst = mv.dst();
        let key_before = self.key;
        let lock_before = self.lock;

        let captured =
            if self.mailbox[dst.raw() as usize] == EMPTY_MAILBOX_SLOT { None } else { Some(self.remove(dst)) };
        let mover = self.remove(src);
        self.put(dst, mover);
        self.flip_side_to_move();

        UndoInfo { captured, key_before, lock_before }
    }

    /// Pass the turn without moving a piece (null-move pruning).
    pub fn make_null(&mut self) -> NullUndo {
        let info = NullUndo { key_before: self.key, lock_before: self.lock };
        self.flip_side_to_move();
        info
    }

    pub fn undo_null(&mut self, info: NullUndo) {
        self.flip_side_to_move();
        debug_assert_eq!(self.key, info.key_before);
        debug_assert_eq!(self.lock, info.lock_before);
    }

    pub fn undo_move(&mut self, mv: Move, info: UndoInfo) {
        let src = mv.src();
        let dst = mv.dst();

        self.flip_side_to_move();
        let mover = self.remove(dst);
        self.put(src, mover);
        if let Some(captured) = info.captured {
            self.put(dst, captured);
        }

        // Restore key / lock directly to guard against hash-function drift if we ever switch
        // to a non-xor-homomorphic scheme. Currently this is merely a sanity safeguard.
        debug_assert_eq!(self.key, info.key_before);
        debug_assert_eq!(self.lock, info.lock_before);
        self.key = info.key_before;
        self.lock = info.lock_before;
    }

    // --------------------------------------------------------------------
    // Attack / check detection
    // --------------------------------------------------------------------

    /// Squares attacked by `attacker`, used by in-check detection.
    ///
    /// Note that pawn attacks for "is this square attacked by a pawn" is **not** the same as
    /// pawn move targets — pre-river pawns only attack forward; there is no separate capture
    /// rule like western chess. So we can reuse `PAWN_ATTACKS`.
    pub fn attacks_from(&self, attacker: Color) -> BitBoard {
        let mut att = BitBoard::EMPTY;
        let occ = self.occ;

        // King (matters for flying-general rule).
        if let Some(king) = self.king_square(attacker) {
            att |= crate::attacks::KING_ATTACKS[king.raw() as usize];
        }
        for sq in self.pieces(attacker, PieceType::Advisor) {
            att |= crate::attacks::ADVISOR_ATTACKS[sq.raw() as usize];
        }
        for sq in self.pieces(attacker, PieceType::Bishop) {
            att |= bishop_attacks(sq, occ);
        }
        for sq in self.pieces(attacker, PieceType::Knight) {
            att |= knight_attacks(sq, occ);
        }
        for sq in self.pieces(attacker, PieceType::Rook) {
            att |= rook_attacks(sq, occ);
        }
        for sq in self.pieces(attacker, PieceType::Cannon) {
            let (_quiet, captures) = cannon_attacks(sq, occ);
            att |= captures;
        }
        for sq in self.pieces(attacker, PieceType::Pawn) {
            att |= crate::attacks::PAWN_ATTACKS[attacker.index()][sq.raw() as usize];
        }

        att
    }

    /// Returns `true` when `color`'s king is in check or facing the opposing king
    /// along an unobstructed file (the "flying-general" rule).
    pub fn is_in_check(&self, color: Color) -> bool {
        let king = match self.king_square(color) {
            Some(k) => k,
            None => return false,
        };
        let opp = color.flip();

        // Flying-general: kings on the same file with no pieces between.
        if let Some(opp_king) = self.king_square(opp)
            && king.file() == opp_king.file()
        {
            let (lo, hi) =
                if king.raw() < opp_king.raw() { (king.raw(), opp_king.raw()) } else { (opp_king.raw(), king.raw()) };
            let mut between_empty = true;
            let mut sq = lo + 9;
            while sq < hi {
                if self.mailbox[sq as usize] != EMPTY_MAILBOX_SLOT {
                    between_empty = false;
                    break;
                }
                sq += 9;
            }
            if between_empty {
                return true;
            }
        }

        let attacks = self.attacks_from(opp);
        attacks.has(king)
    }

    // --------------------------------------------------------------------
    // Utility for V2 incremental evaluation (reserved).
    // --------------------------------------------------------------------

    #[inline]
    pub fn material(&self, color: Color) -> i32 { self.material[color.index()] }

    /// PSQ score from red's perspective: `psq[red] - psq[black]`.
    #[inline]
    pub fn psq_score(&self) -> i32 { self.psq[0] - self.psq[1] }

    /// True if `sq` lies on `color`'s home side of the river.
    #[inline]
    pub fn is_home_half(sq: Square, color: Color) -> bool { HOME_HALVES[color.index()].has(sq) }
}

// --------------------------------------------------------------------
// Piece values (traditional Chinese-chess point count; adjust in eval.rs later).
// --------------------------------------------------------------------

#[inline]
const fn piece_value(kind: PieceType) -> i32 {
    match kind {
        PieceType::King => 10_000,
        PieceType::Advisor => 20,
        PieceType::Bishop => 20,
        PieceType::Knight => 40,
        PieceType::Rook => 90,
        PieceType::Cannon => 45,
        PieceType::Pawn => 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_and_remove_is_symmetric() {
        let mut p = Position::empty();
        let sq = Square::from_iccs("e0").unwrap();
        let piece = Piece::new(Color::Red, PieceType::King);
        p.put(sq, piece);
        assert_eq!(p.piece_at(sq), Some(piece));
        assert_eq!(p.king_square(Color::Red), Some(sq));
        p.remove(sq);
        assert_eq!(p.piece_at(sq), None);
        assert_eq!(p.zobrist_key(), 0);
        assert_eq!(p.zobrist_lock(), 0);
    }

    #[test]
    fn make_undo_restores_state() {
        let mut p = Position::empty();
        let red_rook = Piece::new(Color::Red, PieceType::Rook);
        let black_rook = Piece::new(Color::Black, PieceType::Rook);
        p.put(Square::from_iccs("a0").unwrap(), red_rook);
        p.put(Square::from_iccs("a9").unwrap(), black_rook);

        let key0 = p.zobrist_key();
        let lock0 = p.zobrist_lock();

        let mv = Move::from_iccs("a0-a9").unwrap();
        let info = p.make_move(mv);
        assert_eq!(info.captured, Some(black_rook));
        assert_eq!(p.piece_at(Square::from_iccs("a9").unwrap()), Some(red_rook));

        p.undo_move(mv, info);
        assert_eq!(p.zobrist_key(), key0);
        assert_eq!(p.zobrist_lock(), lock0);
        assert_eq!(p.piece_at(Square::from_iccs("a9").unwrap()), Some(black_rook));
    }
}
