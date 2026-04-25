//! Engine facade. Owns the long-lived state (position, TT, optional book) and exposes an
//! ergonomic builder-based API with optional Lazy SMP parallelism.
//!
//! ```no_run
//! use std::time::Duration;
//! use chessai::{Engine, Limits};
//!
//! let mut engine = Engine::builder()
//!     .hash_size(128)
//!     .threads(4)
//!     .build();
//! let info = engine.search(Limits::new().depth(12).time(Duration::from_millis(500)));
//! println!("{:?} score={} depth={} nps={}", info.best_move, info.score, info.depth, info.nps);
//! ```

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;

use crate::book::Book;
use crate::error::ChessAIError;
use crate::fen::STARTING_FEN;
use crate::limits::Limits;
use crate::movegen::MoveList;
use crate::movegen::generate_pseudo;
use crate::mv::Move;
use crate::position::Position;
use crate::position::UndoInfo;
use crate::search::Search;
use crate::search::SearchInfo;
use crate::tt::TranspositionTable;
use crate::util::SplitMix64;

/// Fixed seed for the internal book-randomisation RNG. A deterministic seed is fine: the
/// book picks between moves stochastically, but users don't need to tune the sequence.
const BOOK_RNG_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

pub struct EngineBuilder {
    hash_size_bytes: usize,
    use_book: bool,
    threads: u8,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        EngineBuilder {
            hash_size_bytes: 32 * 1024 * 1024, // 32 MB default
            use_book: true,
            threads: 1,
        }
    }
}

impl EngineBuilder {
    pub fn new() -> Self { Self::default() }

    #[must_use]
    pub fn hash_size(mut self, mb: usize) -> Self {
        self.hash_size_bytes = mb * 1024 * 1024;
        self
    }

    #[must_use]
    pub fn use_book(mut self, yes: bool) -> Self {
        self.use_book = yes;
        self
    }

    /// Number of search threads (Lazy SMP). `0` falls back to 1.
    #[must_use]
    pub fn threads(mut self, n: u8) -> Self {
        self.threads = n.max(1);
        self
    }

    pub fn build(self) -> Engine {
        let position = Position::from_fen(STARTING_FEN).expect("startpos FEN parses");
        let book = if self.use_book { Some(Book::embedded()) } else { None };
        Engine {
            position,
            tt: Arc::new(TranspositionTable::new(self.hash_size_bytes)),
            book,
            stop: Arc::new(AtomicBool::new(false)),
            rng: SplitMix64::new(BOOK_RNG_SEED),
            move_counter: 0,
            game_keys: Vec::with_capacity(256),
            move_history: Vec::with_capacity(256),
            threads: self.threads,
        }
    }
}

pub struct Engine {
    position: Position,
    tt: Arc<TranspositionTable>,
    book: Option<Book>,
    stop: Arc<AtomicBool>,
    rng: SplitMix64,
    move_counter: u32,
    game_keys: Vec<u64>,
    move_history: Vec<(Move, UndoInfo)>,
    threads: u8,
}

impl Engine {
    pub fn builder() -> EngineBuilder { EngineBuilder::new() }

    // ---------------- Position access ----------------

    pub fn position(&self) -> &Position { &self.position }

    pub fn set_fen(&mut self, fen: &str) -> Result<(), ChessAIError> {
        self.position = Position::from_fen(fen)?;
        // Shared TT — need interior-mutable clear. `Arc::get_mut` works when we're the sole
        // owner, which is true here since workers are joined before returning.
        if let Some(tt) = Arc::get_mut(&mut self.tt) {
            tt.clear();
        } else {
            // Rare: someone still holds a reference. Allocate a fresh table; old one is
            // dropped when its last clone goes away.
            self.tt = Arc::new(TranspositionTable::new(self.tt.size_bytes()));
        }
        self.move_counter = 0;
        self.game_keys.clear();
        self.move_history.clear();
        Ok(())
    }

    pub fn reset_to_startpos(&mut self) { self.set_fen(STARTING_FEN).expect("startpos FEN must parse"); }

    pub fn fen(&self) -> String { self.position.to_fen() }

    pub fn side_to_move(&self) -> crate::color::Color { self.position.side_to_move() }

    pub fn threads(&self) -> u8 { self.threads }

    pub fn legal_moves(&mut self) -> Vec<Move> {
        let mut pseudo = MoveList::new();
        generate_pseudo(&self.position, &mut pseudo);
        let mut legal = Vec::with_capacity(pseudo.len());
        let us = self.position.side_to_move();
        for mv in pseudo.as_slice() {
            let undo = self.position.make_move(*mv);
            if !self.position.is_in_check(us) {
                legal.push(*mv);
            }
            self.position.undo_move(*mv, undo);
        }
        legal
    }

    pub fn make_move(&mut self, mv: Move) -> bool {
        let mut pseudo = MoveList::new();
        generate_pseudo(&self.position, &mut pseudo);
        if !pseudo.as_slice().contains(&mv) {
            return false;
        }
        let us = self.position.side_to_move();
        let pre_key = self.position.zobrist_key();
        let undo = self.position.make_move(mv);
        if self.position.is_in_check(us) {
            self.position.undo_move(mv, undo);
            return false;
        }
        self.game_keys.push(pre_key);
        self.move_counter += 1;
        self.move_history.push((mv, undo));
        true
    }

    /// Undo the most recently played move. Returns the move that was reverted, or `None` if
    /// no moves have been played since the last `set_fen` / `reset_to_startpos` (or since
    /// engine construction). Restores the board, repetition history, and move counter — but
    /// does **not** clear the transposition table, since prior entries remain valid for the
    /// restored positions.
    pub fn undo_move(&mut self) -> Option<Move> {
        let (mv, undo) = self.move_history.pop()?;
        self.position.undo_move(mv, undo);
        self.game_keys.pop();
        self.move_counter = self.move_counter.saturating_sub(1);
        Some(mv)
    }

    pub fn game_key_history(&self) -> &[u64] { &self.game_keys }

    pub fn history_len(&self) -> usize { self.move_history.len() }

    pub fn move_history(&self) -> impl Iterator<Item = Move> + '_ { self.move_history.iter().map(|(m, _)| *m) }

    // ---------------- Book ----------------

    pub fn book_move(&mut self) -> Option<Move> {
        let book = self.book.as_ref()?;
        let mv = book.probe(&self.position, &mut self.rng)?;
        let mut ml = MoveList::new();
        generate_pseudo(&self.position, &mut ml);
        let mut legal = false;
        for m in ml.as_slice() {
            if *m == mv {
                let undo = self.position.make_move(*m);
                if !self.position.is_in_check(self.position.side_to_move().flip()) {
                    legal = true;
                }
                self.position.undo_move(*m, undo);
                break;
            }
        }
        if legal { Some(mv) } else { None }
    }

    // ---------------- Search ----------------

    pub fn stop_handle(&self) -> Arc<AtomicBool> { self.stop.clone() }

    pub fn search(&mut self, limits: Limits) -> SearchInfo { self.search_with(limits, |_| {}) }

    pub fn search_with<F>(&mut self, limits: Limits, mut callback: F) -> SearchInfo
    where
        F: FnMut(&SearchInfo),
    {
        // Book first (disabled if `use_book(false)` was set).
        if self.book.is_some()
            && let Some(mv) = self.book_move()
        {
            let info = SearchInfo {
                depth: 0,
                score: 0,
                best_move: Some(mv),
                pv: vec![mv],
                nodes: 0,
                time: std::time::Duration::ZERO,
                nps: 0,
            };
            callback(&info);
            return info;
        }

        self.stop.store(false, Ordering::Relaxed);

        if self.threads <= 1 {
            return self.search_single(limits, &mut callback);
        }
        self.search_parallel(limits, &mut callback)
    }

    fn search_single<F: FnMut(&SearchInfo)>(&mut self, limits: Limits, callback: &mut F) -> SearchInfo {
        let mut search = Search::new(&mut self.position, Arc::clone(&self.tt), Arc::clone(&self.stop));
        search.seed_game_history(&self.game_keys);
        search.run(limits, |info| callback(info))
    }

    fn search_parallel<F: FnMut(&SearchInfo)>(&mut self, limits: Limits, callback: &mut F) -> SearchInfo {
        let n = self.threads as usize;

        thread::scope(|scope| {
            // Workers: thread ids 1..n.
            let mut worker_handles = Vec::with_capacity(n - 1);
            for tid in 1..n {
                let tt = Arc::clone(&self.tt);
                let stop = Arc::clone(&self.stop);
                let game_keys = self.game_keys.clone();
                let mut pos = self.position.clone();
                let h = scope.spawn(move || {
                    let mut search = Search::new(&mut pos, tt, stop);
                    search.seed_game_history(&game_keys);
                    search.thread_id = tid as u8;
                    search.run(limits, |_info| {})
                });
                worker_handles.push(h);
            }

            // Main thread (id 0). Drives the user-visible callback and owns the returned
            // info by default; workers' info is merged below.
            let main_info = {
                let mut search = Search::new(&mut self.position, Arc::clone(&self.tt), Arc::clone(&self.stop));
                search.seed_game_history(&self.game_keys);
                search.thread_id = 0;
                search.run(limits, |info| callback(info))
            };

            // Main thread finished → stop workers.
            self.stop.store(true, Ordering::Relaxed);

            let worker_infos: Vec<SearchInfo> =
                worker_handles.into_iter().map(|h| h.join().expect("worker panic")).collect();

            // Pick the deepest completed iteration as the authoritative result.
            let mut best = main_info;
            for info in worker_infos {
                if info.best_move.is_none() {
                    continue;
                }
                if info.depth > best.depth || (info.depth == best.depth && info.score > best.score) {
                    best = info;
                }
            }
            best
        })
    }
}

impl Default for Engine {
    fn default() -> Self { EngineBuilder::default().build() }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn single_thread_plays_a_move() {
        let mut e = EngineBuilder::default().threads(1).build();
        let info = e.search(Limits::new().depth(4).time(Duration::from_secs(2)));
        assert!(info.best_move.is_some());
    }

    #[test]
    fn parallel_plays_a_move() {
        let mut e = EngineBuilder::default().threads(4).build();
        let info = e.search(Limits::new().depth(6).time(Duration::from_secs(3)));
        assert!(info.best_move.is_some());
    }

    #[test]
    fn legal_move_count_is_44_at_startpos() {
        let mut e = Engine::default();
        assert_eq!(e.legal_moves().len(), 44);
    }

    #[test]
    fn undo_after_make_restores_zobrist() {
        let mut e = Engine::default();
        let key0 = e.position().zobrist_key();
        let mv = e.legal_moves()[0];
        assert!(e.make_move(mv));
        assert_ne!(e.position().zobrist_key(), key0);
        assert_eq!(e.undo_move(), Some(mv));
        assert_eq!(e.position().zobrist_key(), key0);
    }

    #[test]
    fn undo_restores_legal_moves_set() {
        use std::collections::HashSet;
        let mut e = Engine::default();
        let before: HashSet<_> = e.legal_moves().into_iter().collect();
        let mv = e.legal_moves()[0];
        e.make_move(mv);
        e.undo_move();
        let after: HashSet<_> = e.legal_moves().into_iter().collect();
        assert_eq!(before, after);
    }

    #[test]
    fn undo_restores_fen() {
        let mut e = Engine::default();
        let fen0 = e.fen();
        let mv = e.legal_moves()[0];
        e.make_move(mv);
        e.undo_move();
        assert_eq!(e.fen(), fen0);
    }

    #[test]
    fn undo_on_empty_history_returns_none() {
        let mut e = Engine::default();
        assert_eq!(e.undo_move(), None);
    }

    #[test]
    fn undo_decrements_counters() {
        let mut e = Engine::default();
        let mv = e.legal_moves()[0];
        e.make_move(mv);
        assert_eq!(e.history_len(), 1);
        assert_eq!(e.game_key_history().len(), 1);
        e.undo_move();
        assert_eq!(e.history_len(), 0);
        assert_eq!(e.game_key_history().len(), 0);
    }

    #[test]
    fn set_fen_clears_history() {
        let mut e = Engine::default();
        let mv = e.legal_moves()[0];
        e.make_move(mv);
        e.set_fen(STARTING_FEN).unwrap();
        assert_eq!(e.history_len(), 0);
        assert_eq!(e.undo_move(), None);
    }

    #[test]
    fn make_undo_make_yields_same_search_result() {
        // Two searches at the same depth, with a make/undo pair in between, must agree —
        // this catches TT corruption and game_keys desync.
        let mut e = EngineBuilder::default().threads(1).use_book(false).build();
        let limits = Limits::new().depth(4).time(Duration::from_secs(2));
        let info_a = e.search(limits);
        let mv = e.legal_moves()[0];
        e.make_move(mv);
        e.undo_move();
        let info_b = e.search(limits);
        assert_eq!(info_a.best_move, info_b.best_move);
    }

    #[test]
    fn multi_ply_unwind_to_startpos() {
        let mut e = Engine::default();
        let fen0 = e.fen();
        let mut played = Vec::new();
        for _ in 0..5 {
            let mv = e.legal_moves()[0];
            assert!(e.make_move(mv));
            played.push(mv);
        }
        assert_eq!(e.history_len(), 5);
        for expected in played.iter().rev() {
            assert_eq!(e.undo_move(), Some(*expected));
        }
        assert_eq!(e.history_len(), 0);
        assert_eq!(e.fen(), fen0);
    }

    #[test]
    fn move_history_iter_returns_played_moves_in_order() {
        let mut e = Engine::default();
        let mut played = Vec::new();
        for _ in 0..3 {
            let mv = e.legal_moves()[0];
            e.make_move(mv);
            played.push(mv);
        }
        let recorded: Vec<_> = e.move_history().collect();
        assert_eq!(recorded, played);
    }
}
