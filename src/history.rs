#[derive(Default, Debug)]
pub struct Histroy {
    pub mv: isize,
    pub pc: isize,
    pub key: isize,
    pub chk: bool,
}

#[derive(Default, Debug)]
pub struct Moved {
    pub mv: isize,
    pub zobrist_key: isize,
    // 吃子
    pub capture_piece: isize,
    // 是否将军
    pub checked: bool,
}

impl Moved {
    pub fn from_irrev(checked: bool) -> Self { Self { mv: 0, capture_piece: 0, zobrist_key: 0, checked } }

    pub fn from_null(zobrist_key: isize) -> Self {
        Self { mv: 0, capture_piece: 0, zobrist_key, checked: false }
    }

    pub fn new(mv: isize, zobrist_key: isize, capture_piece: isize, checked: bool) -> Self {
        Self { mv, zobrist_key, capture_piece, checked }
    }
}
