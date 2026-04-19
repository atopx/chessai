use crate::bitboard::BitBoard;
use crate::bitboard::HOME_HALVES;
use crate::bitboard::PALACES;
use crate::color::Color;
use crate::square::Square;

// ====================== Non-sliding tables ======================

pub const KING_ATTACKS: [BitBoard; 90] = build_king();
pub const ADVISOR_ATTACKS: [BitBoard; 90] = build_advisor();
pub const PAWN_ATTACKS: [[BitBoard; 90]; 2] = [build_pawn(Color::Red), build_pawn(Color::Black)];

// ====================== Bishop / Knight entries ======================

/// One blocker-dependent ray: the entry contributes `destinations` to the attack set when
/// `(blocker & occupancy).is_empty()`. Unused slots hold `BitBoard::EMPTY` for both fields.
#[derive(Copy, Clone, Debug, Default)]
pub struct RayEntry {
    pub blocker: BitBoard,
    pub destinations: BitBoard,
}

pub const BISHOP_RAYS: [[RayEntry; 4]; 90] = build_bishop_rays();
pub const KNIGHT_RAYS: [[RayEntry; 4]; 90] = build_knight_rays();

// ====================== Runtime attack computation ======================

#[inline]
pub fn bishop_attacks(sq: Square, occ: BitBoard) -> BitBoard {
    let mut att = BitBoard::EMPTY;
    for entry in BISHOP_RAYS[sq.raw() as usize].iter() {
        if entry.destinations.is_empty() {
            continue;
        }
        if (entry.blocker & occ).is_empty() {
            att |= entry.destinations;
        }
    }
    att
}

#[inline]
pub fn knight_attacks(sq: Square, occ: BitBoard) -> BitBoard {
    let mut att = BitBoard::EMPTY;
    for entry in KNIGHT_RAYS[sq.raw() as usize].iter() {
        if entry.destinations.is_empty() {
            continue;
        }
        if (entry.blocker & occ).is_empty() {
            att |= entry.destinations;
        }
    }
    att
}

// ====================== Table builders (const fn) ======================

const fn sq(rank: u8, file: u8) -> Option<u8> { if rank < 10 && file < 9 { Some(rank * 9 + file) } else { None } }

const fn set_bit(bb: BitBoard, raw: u8) -> BitBoard { BitBoard(bb.0 | (1u128 << raw as u32)) }

const fn build_king() -> [BitBoard; 90] {
    let mut out = [BitBoard::EMPTY; 90];
    let mut s = 0u8;
    while s < 90 {
        let rank = s / 9;
        let file = s % 9;
        let in_red_palace = rank <= 2 && file >= 3 && file <= 5;
        let in_black_palace = rank >= 7 && file >= 3 && file <= 5;
        if in_red_palace || in_black_palace {
            let mut bb = BitBoard::EMPTY;
            // Up / down / left / right, restricted to the palace this square belongs to.
            let palace = if in_red_palace { PALACES[0] } else { PALACES[1] };
            let candidates = [
                sq(rank.wrapping_add(1), file),
                if rank > 0 { sq(rank - 1, file) } else { None },
                if file > 0 { sq(rank, file - 1) } else { None },
                sq(rank, file.wrapping_add(1)),
            ];
            let mut i = 0;
            while i < 4 {
                if let Some(t) = candidates[i] {
                    let bit = 1u128 << t as u32;
                    if palace.0 & bit != 0 {
                        bb = BitBoard(bb.0 | bit);
                    }
                }
                i += 1;
            }
            out[s as usize] = bb;
        }
        s += 1;
    }
    out
}

const fn build_advisor() -> [BitBoard; 90] {
    let mut out = [BitBoard::EMPTY; 90];
    let mut s = 0u8;
    while s < 90 {
        let rank = s / 9;
        let file = s % 9;
        let in_red_palace = rank <= 2 && file >= 3 && file <= 5;
        let in_black_palace = rank >= 7 && file >= 3 && file <= 5;
        if in_red_palace || in_black_palace {
            let mut bb = BitBoard::EMPTY;
            let palace = if in_red_palace { PALACES[0] } else { PALACES[1] };
            let candidates = [
                if rank > 0 && file > 0 { sq(rank - 1, file - 1) } else { None },
                if rank > 0 { sq(rank - 1, file.wrapping_add(1)) } else { None },
                if file > 0 { sq(rank.wrapping_add(1), file - 1) } else { None },
                sq(rank.wrapping_add(1), file.wrapping_add(1)),
            ];
            let mut i = 0;
            while i < 4 {
                if let Some(t) = candidates[i] {
                    let bit = 1u128 << t as u32;
                    if palace.0 & bit != 0 {
                        bb = BitBoard(bb.0 | bit);
                    }
                }
                i += 1;
            }
            out[s as usize] = bb;
        }
        s += 1;
    }
    out
}

const fn build_pawn(color: Color) -> [BitBoard; 90] {
    let mut out = [BitBoard::EMPTY; 90];
    let mut s = 0u8;
    while s < 90 {
        let rank = s / 9;
        let file = s % 9;
        let mut bb = BitBoard::EMPTY;

        // Forward square.
        let forward = match color {
            Color::Red if rank < 9 => sq(rank + 1, file),
            Color::Black if rank > 0 => sq(rank - 1, file),
            _ => None,
        };
        if let Some(t) = forward {
            bb = set_bit(bb, t);
        }

        // Sideways only after the pawn has crossed the river.
        let crossed = match color {
            Color::Red => rank >= 5,
            Color::Black => rank <= 4,
        };
        if crossed {
            if file > 0
                && let Some(t) = sq(rank, file - 1)
            {
                bb = set_bit(bb, t);
            }
            if file < 8
                && let Some(t) = sq(rank, file + 1)
            {
                bb = set_bit(bb, t);
            }
        }

        out[s as usize] = bb;
        s += 1;
    }
    out
}

const fn build_bishop_rays() -> [[RayEntry; 4]; 90] {
    let empty_entry = RayEntry { blocker: BitBoard::EMPTY, destinations: BitBoard::EMPTY };
    let mut out = [[empty_entry; 4]; 90];
    let deltas: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

    let mut s = 0u8;
    while s < 90 {
        let rank = (s / 9) as i8;
        let file = (s % 9) as i8;
        let mut slot = 0usize;

        let mut i = 0;
        while i < 4 {
            let (dr, df) = deltas[i];
            let eye_r = rank + dr;
            let eye_f = file + df;
            let dst_r = rank + 2 * dr;
            let dst_f = file + 2 * df;
            i += 1;

            if eye_r < 0 || eye_r > 9 || eye_f < 0 || eye_f > 8 {
                continue;
            }
            if dst_r < 0 || dst_r > 9 || dst_f < 0 || dst_f > 8 {
                continue;
            }
            let eye = (eye_r as u8) * 9 + eye_f as u8;
            let dst = (dst_r as u8) * 9 + dst_f as u8;

            // Bishop cannot cross the river. Enforce home-half on both eye and destination.
            let red_home = HOME_HALVES[Color::Red as usize].0;
            let black_home = HOME_HALVES[Color::Black as usize].0;
            let eye_bit = 1u128 << eye as u32;
            let dst_bit = 1u128 << dst as u32;
            let src_bit = 1u128 << s as u32;

            let src_red = src_bit & red_home != 0;
            let src_black = src_bit & black_home != 0;
            let all_red = src_red && (eye_bit & red_home != 0) && (dst_bit & red_home != 0);
            let all_black = src_black && (eye_bit & black_home != 0) && (dst_bit & black_home != 0);
            if !all_red && !all_black {
                continue;
            }

            out[s as usize][slot] = RayEntry { blocker: BitBoard(eye_bit), destinations: BitBoard(dst_bit) };
            slot += 1;
        }
        s += 1;
    }
    out
}

const fn build_knight_rays() -> [[RayEntry; 4]; 90] {
    let empty_entry = RayEntry { blocker: BitBoard::EMPTY, destinations: BitBoard::EMPTY };
    let mut out = [[empty_entry; 4]; 90];

    // Four "legs" (1 orthogonal step) and the two targets per leg (each 2 steps
    // perpendicular to the leg direction).
    //
    //          N
    //     target target
    //       leg
    //  target ·· target  < W    E >
    //       leg
    //     target target
    //          S
    //
    // Leg directions: N, S, W, E → indexed 0..=3. For each we list dst deltas in
    // (dr, df) form.
    let legs: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, -1), (0, 1)];
    let targets_per_leg: [[(i8, i8); 2]; 4] = [
        [(2, -1), (2, 1)],   // leg N
        [(-2, -1), (-2, 1)], // leg S
        [(-1, -2), (1, -2)], // leg W
        [(-1, 2), (1, 2)],   // leg E
    ];

    let mut s = 0u8;
    while s < 90 {
        let rank = (s / 9) as i8;
        let file = (s % 9) as i8;
        let mut slot = 0usize;

        let mut leg_i = 0;
        while leg_i < 4 {
            let (lr, lf) = legs[leg_i];
            let leg_r = rank + lr;
            let leg_f = file + lf;
            if leg_r < 0 || leg_r > 9 || leg_f < 0 || leg_f > 8 {
                leg_i += 1;
                continue;
            }
            let leg_sq = (leg_r as u8) * 9 + leg_f as u8;

            let mut dsts = 0u128;
            let mut j = 0;
            while j < 2 {
                let (dr, df) = targets_per_leg[leg_i][j];
                let dr2 = rank + dr;
                let df2 = file + df;
                if dr2 >= 0 && dr2 <= 9 && df2 >= 0 && df2 <= 8 {
                    let dst = (dr2 as u8) * 9 + df2 as u8;
                    dsts |= 1u128 << dst as u32;
                }
                j += 1;
            }

            if dsts != 0 {
                out[s as usize][slot] =
                    RayEntry { blocker: BitBoard(1u128 << leg_sq as u32), destinations: BitBoard(dsts) };
                slot += 1;
            }
            leg_i += 1;
        }
        s += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn king_only_non_empty_in_palaces() {
        let mut total = 0u32;
        for sq_raw in 0..90u8 {
            let sq = Square::new_unchecked(sq_raw);
            let bb = KING_ATTACKS[sq_raw as usize];
            if bb.any() {
                assert!(sq.is_in_palace(Color::Red) || sq.is_in_palace(Color::Black));
                total += 1;
            }
        }
        assert_eq!(total, 18, "nine palace squares × 2 colors");
    }

    #[test]
    fn advisor_center_attacks_four_corners() {
        // Red palace center is (rank=1, file=4) — ICCS e1.
        let center = Square::from_iccs("e1").unwrap();
        let bb = ADVISOR_ATTACKS[center.raw() as usize];
        assert_eq!(bb.popcount(), 4);
    }

    #[test]
    fn pawn_forward_direction() {
        // Red pawn at rank 0 advances to rank 1.
        let start = Square::from_rank_file(0, 0).unwrap();
        let bb = PAWN_ATTACKS[Color::Red.index()][start.raw() as usize];
        assert!(bb.has(Square::from_rank_file(1, 0).unwrap()));
        assert!(!bb.has(Square::from_rank_file(0, 1).unwrap()));
    }

    #[test]
    fn pawn_gains_sideways_after_crossing_river() {
        // Red pawn on rank 5 (just crossed) has 3 destinations: forward + L/R.
        let s = Square::from_rank_file(5, 4).unwrap();
        let bb = PAWN_ATTACKS[Color::Red.index()][s.raw() as usize];
        assert_eq!(bb.popcount(), 3);
    }

    #[test]
    fn knight_from_center_has_8_targets() {
        let s = Square::from_rank_file(4, 4).unwrap();
        let att = knight_attacks(s, BitBoard::EMPTY);
        assert_eq!(att.popcount(), 8);
    }

    #[test]
    fn knight_with_leg_blocked_loses_two_targets() {
        let s = Square::from_rank_file(4, 4).unwrap();
        let occ = BitBoard::from_square(Square::from_rank_file(5, 4).unwrap());
        let att = knight_attacks(s, occ);
        // Blocked leg to the north eliminates two targets.
        assert_eq!(att.popcount(), 6);
    }

    #[test]
    fn bishop_from_back_rank_has_2_targets() {
        let s = Square::from_iccs("c0").unwrap(); // red bishop initial
        let att = bishop_attacks(s, BitBoard::EMPTY);
        assert_eq!(att.popcount(), 2);
    }

    #[test]
    fn bishop_never_crosses_river() {
        let red_half = HOME_HALVES[Color::Red.index()];
        for sq_raw in 0..90u8 {
            let sq = Square::new_unchecked(sq_raw);
            let src_is_red_half = red_half.has(sq);
            for entry in BISHOP_RAYS[sq_raw as usize].iter() {
                if entry.destinations.is_empty() {
                    continue;
                }
                let dst_sq = entry.destinations.lsb_square();
                assert_eq!(src_is_red_half, red_half.has(dst_sq));
            }
        }
    }
}
