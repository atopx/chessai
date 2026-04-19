use std::sync::LazyLock;

use crate::piece::Piece;
use crate::square::Square;
use crate::util::Rc4;
use crate::util::SplitMix64;

pub struct ZobristTables {
    /// Transposition-table keys, indexed as `[piece_index][square]`.
    pub key_piece: [[u64; Square::COUNT]; Piece::COUNT],
    /// Opening-book lock table. Values are drawn from the book's RC4 stream and re-indexed
    /// from the on-disk 16-wide mailbox to our compact `0..90` square layout.
    pub lock_piece: [[u32; Square::COUNT]; Piece::COUNT],
    pub key_stm: u64,
    pub lock_stm: u32,
}

pub static ZOBRIST: LazyLock<ZobristTables> = LazyLock::new(build);

/// Book file mailbox row stride (16 cells per row, 3-cell border on each side).
const BOOK_MAILBOX_STRIDE: u8 = 16;
/// Topmost playable rank in the book mailbox (the book stores the board rank-flipped).
const BOOK_RANK_TOP: u8 = 12;
/// Leftmost playable file in the book mailbox layout.
const BOOK_FILE_OFFSET: u8 = 3;

fn build() -> ZobristTables {
    // Step 1: drain the RC4 stream that the embedded book was built against, so lock values
    // for identical positions agree bit-for-bit with the book entries.
    let mut rc4 = Rc4::new(&[0]);
    let _ = rc4.next_u32(); // unused side-to-move key (book only needs the lock)
    let _ = rc4.next_u32();
    let lock_stm = rc4.next_u32();

    let mut lock_mailbox = [[0u32; 256]; Piece::COUNT];
    for row in lock_mailbox.iter_mut() {
        for slot in row.iter_mut() {
            let _ = rc4.next_u32(); // unused key word
            let _ = rc4.next_u32();
            *slot = rc4.next_u32();
        }
    }

    // Step 2: project each piece/square into the book's mailbox indexing (rank flipped,
    // 3-cell border on each side) so that a mirrored scan of our 0..=89 layout reproduces
    // the original lock values.
    let mut lock_piece = [[0u32; Square::COUNT]; Piece::COUNT];
    for color in 0..2 {
        for kind in 0..7 {
            let piece_idx = color * 7 + kind;
            for sq in 0..Square::COUNT as u8 {
                let rank = sq / 9;
                let file = sq % 9;
                let book_rank = BOOK_RANK_TOP - rank;
                let book_file = file + BOOK_FILE_OFFSET;
                let book_mailbox = (book_rank * BOOK_MAILBOX_STRIDE + book_file) as usize;
                lock_piece[piece_idx][sq as usize] = lock_mailbox[piece_idx][book_mailbox];
            }
        }
    }

    // Step 3: fresh 64-bit keys for the transposition table. Seed is arbitrary but fixed so
    // tests are reproducible across runs.
    let mut rng = SplitMix64::new(0xA1B2_C3D4_E5F6_0718);
    let key_stm = rng.next_u64();
    let mut key_piece = [[0u64; Square::COUNT]; Piece::COUNT];
    for row in key_piece.iter_mut() {
        for slot in row.iter_mut() {
            *slot = rng.next_u64();
        }
    }

    ZobristTables { key_piece, lock_piece, key_stm, lock_stm }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_populated() {
        let t = &*ZOBRIST;
        let mut zero_keys = 0usize;
        for row in &t.key_piece {
            for v in row {
                if *v == 0 {
                    zero_keys += 1;
                }
            }
        }
        assert!(zero_keys < 4, "zero-key entries should be rare: {zero_keys}");
        assert_ne!(t.key_stm, 0);
        assert_ne!(t.lock_stm, 0);
    }
}
