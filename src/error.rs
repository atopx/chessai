use thiserror::Error;

/// All errors surfaced by the public API.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ChessAIError {
    #[error("FEN is empty")]
    EmptyFen,
    #[error("FEN contains invalid character {c:?} at byte {index}")]
    InvalidFenChar { c: char, index: usize },
    #[error("FEN describes more than 9 files in rank {rank}")]
    FenRankOverflow { rank: u8 },
    #[error("FEN describes fewer than 10 ranks")]
    FenRankUnderflow,
    #[error("unknown side-to-move marker {marker:?}; expected 'w', 'r', or 'b'")]
    BadSideToMove { marker: char },
    #[error("cannot parse square from {0:?}; expected e.g. b2")]
    BadIccsSquare(String),
    #[error("cannot parse move from {0:?}; expected e.g. b2-e2 or h2e2")]
    BadIccsMove(String),
}
