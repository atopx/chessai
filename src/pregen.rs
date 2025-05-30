pub const LIMIT_DEPTH: usize = 64;
pub const NULL_DEPTH: isize = 2;
pub const RANDOMNESS: isize = 8;
pub const HASH_ALPHA: isize = 1;
pub const HASH_BETA: isize = 2;
pub const HASH_PV: isize = 3;

pub const MATE_VALUE: isize = 10000;
pub const BAN_VALUE: isize = MATE_VALUE - 100;
pub const WIN_VALUE: isize = MATE_VALUE - 200;

pub const NULL_SAFE_MARGIN: isize = 400;
pub const NULL_OKAY_MARGIN: isize = 200;

pub const DRAW_VALUE: isize = 20;
pub const ADVANCED_VALUE: isize = 3;

pub const RANK_TOP: isize = 3;
pub const RANK_BOTTOM: isize = 12;

pub const FILE_LEFT: isize = 3;
pub const FILE_RIGHT: isize = 11;

pub const PIECE_KING: isize = 0;
pub const PIECE_ADVISOR: isize = 1;
pub const PIECE_BISHOP: isize = 2;
pub const PIECE_KNIGHT: isize = 3;
pub const PIECE_ROOK: isize = 4;
pub const PIECE_CANNON: isize = 5;
pub const PIECE_PAWN: isize = 6;

pub const WINNER_WHITE: isize = 0;
pub const WINNER_BLACK: isize = 1;
pub const WINNER_TIE: isize = 2;
pub const WINNER_3: isize = 3;

pub enum Winner {
    White, // 红方胜
    Black, // 黑方胜
    Tie,   // 和
}

pub fn from_char(c: char) -> Option<isize> {
    match c {
        'K' => Some(PIECE_KING),
        'A' => Some(PIECE_ADVISOR),
        'B' => Some(PIECE_BISHOP),
        'E' => Some(PIECE_BISHOP),
        'H' => Some(PIECE_KNIGHT),
        'N' => Some(PIECE_KNIGHT),
        'R' => Some(PIECE_ROOK),
        'C' => Some(PIECE_CANNON),
        'P' => Some(PIECE_PAWN),
        _ => None,
    }
}

pub const FEN_PIECE: [char; 24] = [
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', 'K', 'A', 'B', 'N', 
    'R', 'C', 'P', ' ', 'k', 'a', 'b', 'n', 'r', 'c', 'p', ' ',
];

pub const BROAD: [i8; 256] = include!("data/BROAD.dat");

pub const FORT: [i8; 256] = include!("data/FORT.dat");

pub const fn in_broad(idx: isize) -> bool {
    BROAD[idx as usize] != 0
}

pub const fn in_fort(idx: isize) -> bool {
    FORT[idx as usize] != 0
}

pub const fn king_span(src: isize, dst: isize) -> bool {
    LEGAL_SPAN[(dst - src + 256) as usize] == 1
}

pub const fn advisor_span(src: isize, dst: isize) -> bool {
    LEGAL_SPAN[(dst - src + 256) as usize] == 2
}

pub const fn bishop_span(src: isize, dst: isize) -> bool {
    LEGAL_SPAN[(dst - src + 256) as usize] == 3
}

pub const fn bishop_pin(src: isize, dst: isize) -> usize {
    ((src + dst) >> 1) as usize
}

pub const fn knight_pin(src: isize, dst: isize) -> isize {
    src + KNIGHT_PIN[(dst - src + 256) as usize]
}

pub const fn home_half(sq: isize, sd: isize) -> bool {
    (sq & 0x80) != (sd << 7)
}

pub const fn away_half(sq: isize, sd: isize) -> bool {
    (sq & 0x80) == (sd << 7)
}

pub const fn same_half(src: isize, dst: isize) -> bool {
    ((src ^ dst) & 0x80) == 0
}

pub const fn same_rank(src: isize, dst: isize) -> bool {
    ((src ^ dst) & 0xf0) == 0
}

pub const fn same_file(src: isize, dst: isize) -> bool {
    ((src ^ dst) & 0x0f) == 0
}

pub const fn mvv_lva(pc: isize, lva: isize) -> isize {
    MVV_VALUE[(pc & 7) as usize] - lva
}

#[derive(Debug)]
pub enum PieceAction {
    ADD,
    DEL,
}

pub const KING_DELTA: [isize; 4] = [-16, -1, 1, 16];
pub const ADVISOR_DELTA: [isize; 4] = [-17, -15, 15, 17];
pub const KNIGHT_DELTA: [[isize; 2]; 4] = [[-33, -31], [-18, 14], [-14, 18], [31, 33]];
pub const KNIGHT_CHECK_DELTA: [[isize; 2]; 4] = [[-33, -18], [-31, -14], [14, 31], [18, 33]];
pub const MVV_VALUE: [isize; 8] = [50, 10, 10, 30, 40, 30, 20, 0];

pub const LEGAL_SPAN: [isize; 512] = include!("data/LEGAL_SPAN.dat");

pub const KNIGHT_PIN: [isize; 512] = include!("data/KNIGHT_PIN.dat");

pub const PIECE_VALUE: [[isize; 256]; 7] = include!("data/PIECE_VALUE.dat");

pub const PRE_GEN_ZOB_RIST_KEY_PLAYER: isize = 1099503838;
pub const PRE_GEN_ZOB_RIST_LOCK_PLAYER: isize = 1730021002;

pub static PRE_GEN_ZOB_RIST_KEY_TABLE: [[isize; 256]; 14] = include!("data/KEY_TABLE.dat");

pub static PRE_GEN_ZOB_RIST_LOCK_TABLE: [[isize; 256]; 14] = include!("data/LOCK_TABLE.dat");
