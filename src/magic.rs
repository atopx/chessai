use std::sync::LazyLock;

use crate::bitboard::BitBoard;
use crate::square::Square;

const RANK_OCC_SIZE: usize = 512; // 9 bits
const FILE_OCC_SIZE: usize = 1024; // 10 bits

type RankTable = [[BitBoard; RANK_OCC_SIZE]; 90];
type FileTable = [[BitBoard; FILE_OCC_SIZE]; 90];

// -------------- Slice extractors --------------

#[inline]
pub fn rank_occ(occ: BitBoard, rank: u8) -> u16 { ((occ.raw() >> (rank as u32 * 9)) & 0x1FF) as u16 }

#[inline]
pub fn file_occ(occ: BitBoard, file: u8) -> u16 {
    // Extract 10 bits from non-contiguous positions: file + 0, file + 9, ..., file + 81.
    // Loop is fully unrolled by LLVM into pure shift+mask+OR.
    let x = occ.raw() >> file as u32;
    let mut r = 0u16;
    r |= (x & 1) as u16;
    r |= (((x >> 9) & 1) as u16) << 1;
    r |= (((x >> 18) & 1) as u16) << 2;
    r |= (((x >> 27) & 1) as u16) << 3;
    r |= (((x >> 36) & 1) as u16) << 4;
    r |= (((x >> 45) & 1) as u16) << 5;
    r |= (((x >> 54) & 1) as u16) << 6;
    r |= (((x >> 63) & 1) as u16) << 7;
    r |= (((x >> 72) & 1) as u16) << 8;
    r |= (((x >> 81) & 1) as u16) << 9;
    r
}

// -------------- Rook tables --------------

pub static ROOK_RANK: LazyLock<Box<RankTable>> = LazyLock::new(|| {
    let mut table = Box::new([[BitBoard::EMPTY; RANK_OCC_SIZE]; 90]);
    for sq_raw in 0..90u8 {
        let rank = sq_raw / 9;
        let file = sq_raw % 9;
        for occ_pattern in 0..RANK_OCC_SIZE {
            let occ = occ_pattern as u16;
            let mut attack_files = 0u16;
            // West.
            let mut f = file as i32 - 1;
            while f >= 0 {
                attack_files |= 1 << f;
                if occ & (1 << f) != 0 {
                    break;
                }
                f -= 1;
            }
            // East.
            let mut f = file as i32 + 1;
            while f <= 8 {
                attack_files |= 1 << f;
                if occ & (1 << f) != 0 {
                    break;
                }
                f += 1;
            }
            table[sq_raw as usize][occ_pattern] = expand_rank(attack_files, rank);
        }
    }
    table
});

pub static ROOK_FILE: LazyLock<Box<FileTable>> = LazyLock::new(|| {
    let mut table = Box::new([[BitBoard::EMPTY; FILE_OCC_SIZE]; 90]);
    for sq_raw in 0..90u8 {
        let rank = sq_raw / 9;
        let file = sq_raw % 9;
        for occ_pattern in 0..FILE_OCC_SIZE {
            let occ = occ_pattern as u16;
            let mut attack_ranks = 0u16;
            // South (rank - 1).
            let mut r = rank as i32 - 1;
            while r >= 0 {
                attack_ranks |= 1 << r;
                if occ & (1 << r) != 0 {
                    break;
                }
                r -= 1;
            }
            // North (rank + 1).
            let mut r = rank as i32 + 1;
            while r <= 9 {
                attack_ranks |= 1 << r;
                if occ & (1 << r) != 0 {
                    break;
                }
                r += 1;
            }
            table[sq_raw as usize][occ_pattern] = expand_file(attack_ranks, file);
        }
    }
    table
});

// -------------- Cannon tables --------------

#[derive(Copy, Clone, Debug, Default)]
pub struct CannonLine {
    pub quiet: BitBoard,
    pub capture: BitBoard,
}

type CannonRankTable = [[CannonLine; RANK_OCC_SIZE]; 90];
type CannonFileTable = [[CannonLine; FILE_OCC_SIZE]; 90];

pub static CANNON_RANK: LazyLock<Box<CannonRankTable>> = LazyLock::new(|| {
    // Build on the heap directly — `[[T; 1024]; 90]` would blow the stack.
    let mut boxed: Box<CannonRankTable> =
        vec![[CannonLine::default(); RANK_OCC_SIZE]; 90].into_boxed_slice().try_into().expect("vec length is 90");
    for sq_raw in 0..90u8 {
        let rank = sq_raw / 9;
        let file = sq_raw % 9;
        for occ_pattern in 0..RANK_OCC_SIZE {
            let occ = occ_pattern as u16;
            let (quiet_files, cap_files) = cannon_line_bits(occ, file, 0, 8);
            boxed[sq_raw as usize][occ_pattern] =
                CannonLine { quiet: expand_rank(quiet_files, rank), capture: expand_rank(cap_files, rank) };
        }
    }
    boxed
});

pub static CANNON_FILE: LazyLock<Box<CannonFileTable>> = LazyLock::new(|| {
    // Same heap-direct allocation pattern; the inner array is 32 KB which is fine on the
    // stack as a vec! seed, but the full 2.88 MB outer array is not.
    let mut boxed: Box<CannonFileTable> =
        vec![[CannonLine::default(); FILE_OCC_SIZE]; 90].into_boxed_slice().try_into().expect("vec length is 90");
    for sq_raw in 0..90u8 {
        let rank = sq_raw / 9;
        let file = sq_raw % 9;
        for occ_pattern in 0..FILE_OCC_SIZE {
            let occ = occ_pattern as u16;
            let (quiet_ranks, cap_ranks) = cannon_line_bits(occ, rank, 0, 9);
            boxed[sq_raw as usize][occ_pattern] =
                CannonLine { quiet: expand_file(quiet_ranks, file), capture: expand_file(cap_ranks, file) };
        }
    }
    boxed
});

// -------------- Public attack helpers --------------

#[inline]
pub fn rook_attacks(sq: Square, occ: BitBoard) -> BitBoard {
    let r = rank_occ(occ, sq.rank());
    let f = file_occ(occ, sq.file());
    ROOK_RANK[sq.raw() as usize][r as usize] | ROOK_FILE[sq.raw() as usize][f as usize]
}

/// Returns `(quiet, captures)` — the two components of cannon attacks.
#[inline]
pub fn cannon_attacks(sq: Square, occ: BitBoard) -> (BitBoard, BitBoard) {
    let r = rank_occ(occ, sq.rank());
    let f = file_occ(occ, sq.file());
    let rr = &CANNON_RANK[sq.raw() as usize][r as usize];
    let ff = &CANNON_FILE[sq.raw() as usize][f as usize];
    (rr.quiet | ff.quiet, rr.capture | ff.capture)
}

// -------------- Internal helpers --------------

/// Compute cannon line bitmasks on a single line where the slider is at position `pos`
/// and the line runs from `lo` to `hi` (inclusive). Returns `(quiet_mask, capture_mask)`.
fn cannon_line_bits(occ: u16, pos: u8, lo: u8, hi: u8) -> (u16, u16) {
    let mut quiet = 0u16;
    let mut capture = 0u16;
    // Walk in both directions. For each side, enumerate squares until the first occupied
    // one (the "screen"), then skip empty squares past it and the first occupied square
    // past the screen is the capture target.
    for dir in [-1i32, 1i32] {
        let mut i = pos as i32 + dir;
        // Phase 1: empty squares pre-screen (all become quiet targets).
        while i >= lo as i32 && i <= hi as i32 {
            if occ & (1 << i) != 0 {
                break;
            }
            quiet |= 1 << i;
            i += dir;
        }
        // Skip past screen.
        i += dir;
        // Phase 2: first occupied past screen = capture target.
        while i >= lo as i32 && i <= hi as i32 {
            if occ & (1 << i) != 0 {
                capture |= 1 << i;
                break;
            }
            i += dir;
        }
    }
    (quiet, capture)
}

/// Expand a 9-bit rank-relative mask into a full-board BitBoard by shifting into the
/// correct rank's bit range.
#[inline]
fn expand_rank(bits: u16, rank: u8) -> BitBoard { BitBoard((bits as u128) << (rank as u32 * 9)) }

/// Expand a 10-bit file-relative mask into a full-board BitBoard by scattering bits to
/// the positions `file, file + 9, file + 18, ..., file + 81`.
#[inline]
fn expand_file(bits: u16, file: u8) -> BitBoard {
    let mut out = 0u128;
    let mut b = bits;
    let mut r = 0u32;
    while b != 0 {
        if b & 1 != 0 {
            out |= 1u128 << (file as u32 + 9 * r);
        }
        b >>= 1;
        r += 1;
    }
    BitBoard(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_file_occ_roundtrip() {
        // Setting a bit and extracting should give a single-bit slice back.
        for sq_raw in 0..90u8 {
            let bb = BitBoard::from_square(Square::new_unchecked(sq_raw));
            let rank = sq_raw / 9;
            let file = sq_raw % 9;
            assert_eq!(rank_occ(bb, rank), 1 << file);
            assert_eq!(file_occ(bb, file), 1 << rank);
        }
    }
}
