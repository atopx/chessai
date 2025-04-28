#[derive(PartialEq, Debug)]
pub enum Status {
    Hash = 0,
    KillerFirst = 1,
    KillerSecond = 2,
    GenMoves = 3,
    REST = 4,
}

pub struct MoveState {
    pub mvs: Vec<isize>,
    pub vls: Vec<isize>,
    pub index: usize,
    pub hash: isize,
    pub killer_first: isize,
    pub killer_second: isize,
    pub phase: Status,
    pub signle: bool,
}

impl MoveState {
    pub fn new(hash: isize) -> Self {
        Self {
            mvs: vec![],
            vls: vec![],
            index: 0,
            hash,
            killer_first: 0,
            killer_second: 0,
            phase: Status::Hash,
            signle: false,
        }
    }
}
