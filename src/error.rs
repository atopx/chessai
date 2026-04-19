use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FenError {
    #[error("FEN is empty")]
    Empty,
    #[error("FEN contains invalid character {c:?} at byte {index}")]
    InvalidChar { c: char, index: usize },
    #[error("FEN describes more than 9 files in rank {rank}")]
    RankOverflow { rank: u8 },
    #[error("FEN describes fewer than 10 ranks")]
    RankUnderflow,
    #[error("unknown side-to-move marker {marker:?}; expected 'w', 'r', or 'b'")]
    BadSide { marker: char },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum MoveParseError {
    #[error("cannot parse square from {0:?}; expected e.g. b2")]
    BadIccs(String),
    #[error("cannot parse move from {0:?}; expected e.g. b2-e2 or h2e2")]
    BadMove(String),
}
