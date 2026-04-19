use crate::bitboard::BitBoard;
use crate::mv::Move;
use crate::position::Position;
use crate::square::Square;
use crate::util::SplitMix64;

const MAX_BOOK_SIZE: usize = 16_384;
const BOOK_BYTES: &[u8] = include_bytes!("../assets/BOOK.DAT");

pub struct Book {
    locks: Vec<u32>,
    moves: Vec<u16>, // already translated to V2 format; 0 when the V1 entry is invalid.
    values: Vec<i16>,
}

impl Book {
    pub fn embedded() -> Self {
        let chunk = 8usize;
        let total = (BOOK_BYTES.len() / chunk).min(MAX_BOOK_SIZE);
        let mut locks = Vec::with_capacity(total);
        let mut moves = Vec::with_capacity(total);
        let mut values = Vec::with_capacity(total);
        for i in 0..total {
            let off = i * chunk;
            let lock =
                u32::from_le_bytes([BOOK_BYTES[off], BOOK_BYTES[off + 1], BOOK_BYTES[off + 2], BOOK_BYTES[off + 3]]);
            let v1_mv = u16::from_le_bytes([BOOK_BYTES[off + 4], BOOK_BYTES[off + 5]]);
            let val = i16::from_le_bytes([BOOK_BYTES[off + 6], BOOK_BYTES[off + 7]]);
            locks.push(lock >> 1);
            moves.push(v1_move_to_v2(v1_mv));
            values.push(val);
        }
        Book { locks, moves, values }
    }

    pub fn len(&self) -> usize { self.locks.len() }

    pub fn is_empty(&self) -> bool { self.locks.is_empty() }

    /// Probe with deterministic weighted sampling. `None` when no entry matches.
    pub fn probe(&self, pos: &Position, rng: &mut SplitMix64) -> Option<Move> {
        if self.locks.is_empty() {
            return None;
        }
        let lock_direct = pos.zobrist_lock() >> 1;
        let (lock, mirror) = if find_lock(&self.locks, lock_direct).is_some() {
            (lock_direct, false)
        } else {
            let mirrored = mirror_position_lock(pos);
            if find_lock(&self.locks, mirrored).is_some() {
                (mirrored, true)
            } else {
                return None;
            }
        };

        // Walk to the first entry matching `lock` (entries are sorted by lock).
        let first = {
            let mut lo = 0isize;
            let mut hi = self.locks.len() as isize - 1;
            let mut found = None;
            while lo <= hi {
                let mid = ((lo + hi) / 2) as usize;
                match self.locks[mid].cmp(&lock) {
                    std::cmp::Ordering::Less => lo = mid as isize + 1,
                    std::cmp::Ordering::Greater => hi = mid as isize - 1,
                    std::cmp::Ordering::Equal => {
                        found = Some(mid);
                        hi = mid as isize - 1;
                    }
                }
            }
            found?
        };

        // Collect legal candidates and their weights.
        let mut candidates: Vec<(Move, i32)> = Vec::new();
        let mut total: i32 = 0;
        let mut i = first;
        while i < self.locks.len() && self.locks[i] == lock {
            let raw = self.moves[i];
            if raw != 0 {
                let mut mv = Move::from_raw(raw);
                if mirror {
                    mv = mv.mirror_file();
                }
                // Probing should filter by pseudo-legality against the *current* position.
                // Without a full legal-move regenerator we rely on the caller to verify via
                // `Engine::make_move`. Here we include every matching entry.
                let w = self.values[i] as i32;
                if w > 0 {
                    candidates.push((mv, w));
                    total += w;
                }
            }
            i += 1;
        }
        if total == 0 || candidates.is_empty() {
            return None;
        }
        let mut roll = (rng.next_u32() as i32).unsigned_abs() as i32 % total;
        for (mv, w) in &candidates {
            roll -= *w;
            if roll < 0 {
                return Some(*mv);
            }
        }
        Some(candidates.last().unwrap().0)
    }
}

fn find_lock(locks: &[u32], key: u32) -> Option<usize> { locks.binary_search(&key).ok() }

/// Compute the Zobrist lock of the horizontally mirrored position, matching V1 semantics.
fn mirror_position_lock(pos: &Position) -> u32 {
    let mut mirrored = Position::empty();
    for sq_raw in 0..Square::COUNT as u8 {
        let sq = Square::new_unchecked(sq_raw);
        if let Some(p) = pos.piece_at(sq) {
            mirrored.put(sq.mirror_file(), p);
        }
    }
    if pos.side_to_move() != crate::color::Color::Red {
        mirrored.flip_side_to_move();
    }
    mirrored.zobrist_lock() >> 1
}

/// Translate a V1 16-bit move (two 8-bit mailbox squares) to a V2 Move. Returns
/// `0` (Move::NULL) when any endpoint is off-board; the caller treats null as "skip entry".
fn v1_move_to_v2(raw: u16) -> u16 {
    let v1_src = (raw & 0xff) as u8;
    let v1_dst = ((raw >> 8) & 0xff) as u8;
    match (v1_to_v2_square(v1_src), v1_to_v2_square(v1_dst)) {
        (Some(src), Some(dst)) => {
            let mv = Move::new(Square::new_unchecked(src), Square::new_unchecked(dst));
            mv.raw()
        }
        _ => 0,
    }
}

/// V1 mailbox index → V2 compact square. `None` for off-board indices.
fn v1_to_v2_square(v1: u8) -> Option<u8> {
    let rank = v1 >> 4;
    let file = v1 & 0x0f;
    if (3..=12).contains(&rank) && (3..=11).contains(&file) { Some((12 - rank) * 9 + (file - 3)) } else { None }
}

// Silence unused-BB lint if the mirror helper is optimised out under certain features.
const _: BitBoard = BitBoard::EMPTY;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::STARTING_FEN;

    #[test]
    fn book_loads_with_many_entries() {
        let b = Book::embedded();
        assert!(b.len() > 1000);
    }

    #[test]
    fn startpos_has_entries() {
        let b = Book::embedded();
        let pos = Position::from_fen(STARTING_FEN).unwrap();
        let mut rng = SplitMix64::new(0xC0FFEE);
        let mv = b.probe(&pos, &mut rng);
        assert!(mv.is_some(), "start position must be in the book");
    }
}
