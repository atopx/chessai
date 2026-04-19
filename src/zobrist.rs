use std::sync::LazyLock;

use crate::piece::Piece;
use crate::square::Square;
use crate::util::Rc4;
use crate::util::SplitMix64;

pub struct ZobristTables {
    /// TT key tables. `[piece_index][square]`.
    pub key_piece: [[u64; Square::COUNT]; Piece::COUNT],
    /// Book-compat lock tables (matches Java RC4 stream, but re-indexed to `0..90`).
    pub lock_piece: [[u32; Square::COUNT]; Piece::COUNT],
    pub key_stm: u64,
    pub lock_stm: u32,
}

pub static ZOBRIST: LazyLock<ZobristTables> = LazyLock::new(build);

fn build() -> ZobristTables {
    // 1. Replay the exact RC4 stream used by the Java engine to produce per-mailbox keys.
    let mut rc4 = Rc4::new(&[0]);
    let _java_key_player_u32 = rc4.next_u32();
    let _ = rc4.next_u32();
    let java_lock_player_u32 = rc4.next_u32();

    let mut java_lock_256 = [[0u32; 256]; 14];
    for row in java_lock_256.iter_mut() {
        for slot in row.iter_mut() {
            let _key = rc4.next_u32();
            let _ = rc4.next_u32();
            *slot = rc4.next_u32();
        }
    }

    // 2. Compress to 0..90 using the legacy mailbox mapping:
    //    Java mailbox square = (rank + 3) * 16 + (file + 3),
    //    where rank = 3..=12 maps to the top-down FEN rows.
    //    V2 rank 0 is red's back (bottom), rank 9 is black's back (top), so
    //    `java_rank = 3 + (9 - v2_rank) = 12 - v2_rank`.
    let mut lock_piece = [[0u32; 90]; 14];
    for color in 0..2 {
        for kind in 0..7 {
            // Java piece index order: 0..=6 red, 7..=13 black.
            let java_idx = color * 7 + kind;
            // V2 piece index uses the same order (see `Piece::index`).
            let v2_idx = java_idx;
            for v2_sq in 0..90u8 {
                let v2_rank = v2_sq / 9;
                let v2_file = v2_sq % 9;
                let java_rank = 12 - v2_rank;
                let java_file = v2_file + 3;
                let java_mailbox = ((java_rank) * 16 + java_file) as usize;
                lock_piece[v2_idx][v2_sq as usize] = java_lock_256[java_idx][java_mailbox];
            }
        }
    }

    // 3. Generate fresh SplitMix64 keys for the TT. Seed arbitrary but fixed so tests are
    //    reproducible across runs.
    let mut rng = SplitMix64::new(0xA1B2_C3D4_E5F6_0718);
    let key_stm = rng.next_u64();
    let mut key_piece = [[0u64; 90]; 14];
    for row in key_piece.iter_mut() {
        for slot in row.iter_mut() {
            *slot = rng.next_u64();
        }
    }

    ZobristTables { key_piece, lock_piece, key_stm, lock_stm: java_lock_player_u32 }
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
