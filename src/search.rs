use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

use crate::eval::BAN_VALUE;
use crate::eval::MATE_VALUE;
use crate::eval::NULL_OKAY_MARGIN;
use crate::eval::WIN_VALUE;
use crate::eval::draw_value;
use crate::eval::evaluate;
use crate::limits::Limits;
use crate::limits::MAX_SEARCH_DEPTH;
use crate::movegen::MoveList;
use crate::movegen::generate_captures;
use crate::movegen::generate_pseudo;
use crate::mv::Move;
use crate::picker::MovePicker;
use crate::piece::Piece;
use crate::piece::PieceType;
use crate::position::Position;
use crate::see::see;
use crate::tt::Bound;
use crate::tt::TranspositionTable;
use crate::tt::mate_score_from_tt;

/// Search thread identifier used to slightly diversify Lazy SMP workers.
pub(crate) type ThreadId = u8;

pub(crate) const MAX_PLY: usize = 64;

const INF: i32 = 32_000;
const ASPIRATION_DELTA: i32 = 16;

/// Lazy SMP depth-skip pattern. Helper threads (id ≥ 1) deliberately skip selected
/// iterative-deepening depths so that workers spread across the depth axis instead of
/// all racing on the same depth.
const SKIP_SIZE: [u8; 20] = [1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4];
const SKIP_PHASE: [u8; 20] = [0, 1, 0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 6, 7];

#[inline]
fn should_skip_depth(thread_id: ThreadId, depth: u8) -> bool {
    if thread_id == 0 {
        return false;
    }
    let i = ((thread_id - 1) as usize) % SKIP_SIZE.len();
    let phase = SKIP_PHASE[i];
    let size = SKIP_SIZE[i];
    ((depth + phase) / size) % 2 == 1
}

/// Snapshot of the best line found so far. Returned by `Search::run` and emitted to
/// callers via the `SearchInfo` callback on every completed iteration.
#[derive(Clone, Debug, Default)]
pub struct SearchInfo {
    pub depth: u8,
    pub score: i32,
    pub best_move: Option<Move>,
    pub pv: Vec<Move>,
    pub nodes: u64,
    pub time: Duration,
    pub nps: u64,
}

pub(crate) struct History {
    pub(crate) killers: [[Move; 2]; MAX_PLY],
    pub(crate) butterfly: Box<[[i32; 90]; Piece::COUNT]>,
    /// Countermove table: `countermove[prev_piece_idx][prev_to_sq]` = the best quiet reply
    /// we've seen after a move made by `prev_piece` that ended on `prev_to`.
    pub(crate) countermove: Box<[[Move; 90]; Piece::COUNT]>,
}

impl Default for History {
    fn default() -> Self { Self::new() }
}

impl History {
    pub(crate) fn new() -> Self {
        History {
            killers: [[Move::NULL; 2]; MAX_PLY],
            butterfly: Box::new([[0; 90]; Piece::COUNT]),
            countermove: Box::new([[Move::NULL; 90]; Piece::COUNT]),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.killers = [[Move::NULL; 2]; MAX_PLY];
        for row in self.butterfly.iter_mut() {
            *row = [0; 90];
        }
        for row in self.countermove.iter_mut() {
            *row = [Move::NULL; 90];
        }
    }

    #[inline]
    pub(crate) fn update_killer(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.killers[ply][0] != mv {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mv;
        }
    }

    #[inline]
    pub(crate) fn update_history(&mut self, piece: Piece, dst_sq: u8, bonus: i32) {
        let row = &mut self.butterfly[piece.index()];
        let entry = &mut row[dst_sq as usize];
        // Exponential-decay update: `h += bonus - h * |bonus| / MAX`, keeps values bounded.
        let clamped = bonus.clamp(-MAX_HISTORY, MAX_HISTORY);
        let delta = clamped - *entry * clamped.abs() / MAX_HISTORY;
        *entry += delta;
    }

    #[inline]
    pub(crate) fn update_countermove(&mut self, prev_piece: Piece, prev_to: u8, reply: Move) {
        self.countermove[prev_piece.index()][prev_to as usize] = reply;
    }

    #[inline]
    pub(crate) fn countermove_for(&self, prev_piece: Piece, prev_to: u8) -> Move {
        self.countermove[prev_piece.index()][prev_to as usize]
    }
}

const MAX_HISTORY: i32 = 1 << 14;

/// One search invocation. Owns per-thread scratch (history, PV, key stack) and borrows the
/// shared TT via `Arc`, allowing Lazy SMP worker threads to share a single table.
pub(crate) struct Search<'a> {
    pos: &'a mut Position,
    tt: Arc<TranspositionTable>,
    history: History,
    stop: Arc<AtomicBool>,

    pub(crate) thread_id: ThreadId,
    start: Instant,
    soft_limit: Option<Duration>,
    hard_limit: Option<Duration>,
    node_limit: Option<u64>,
    max_depth: u8,

    pub(crate) nodes: u64,
    /// Principal variation triangle; `pv[ply][0..pv_len[ply]]` is the line rooted at ply.
    pv: Vec<[Move; MAX_PLY]>,
    pv_len: [usize; MAX_PLY],
    /// Repetition history: Zobrist keys along the current search path.
    key_stack: Vec<u64>,
    /// Per-ply metadata aligned with `key_stack`: was the side-to-move in check after the
    /// move that produced this key? Was it a capture? Used to classify the
    /// xiangqi-specific perpetual-check / perpetual-chase cycle types.
    meta_stack: Vec<PlyMeta>,
    /// Static evaluation at each ply, enabling the "improving" heuristic used by LMP/LMR.
    static_evals: [i32; MAX_PLY + 1],
    /// Per-ply excluded move. When non-null at ply `p`, the `alpha_beta` call for that ply
    /// will skip the move in its main loop. Enables Singular Extension verification.
    excluded_at_ply: [Move; MAX_PLY + 1],
    /// (piece, to-square) of the most recent move at each ply, for countermove lookup.
    prev_move_info: [Option<(Piece, u8)>; MAX_PLY + 1],
}

#[derive(Copy, Clone, Debug, Default)]
struct PlyMeta {
    /// After applying this move, is the new side-to-move in check?
    gave_check: bool,
    /// True when this move was a capture (breaks repetition chains for irreversible moves).
    was_capture: bool,
}

impl<'a> Search<'a> {
    pub(crate) fn new(pos: &'a mut Position, tt: Arc<TranspositionTable>, stop: Arc<AtomicBool>) -> Self {
        Search {
            pos,
            tt,
            history: History::new(),
            stop,
            thread_id: 0,
            start: Instant::now(),
            soft_limit: None,
            hard_limit: None,
            node_limit: None,
            max_depth: MAX_SEARCH_DEPTH,
            nodes: 0,
            pv: vec![[Move::NULL; MAX_PLY]; MAX_PLY + 1],
            pv_len: [0; MAX_PLY],
            key_stack: Vec::with_capacity(256),
            meta_stack: Vec::with_capacity(256),
            static_evals: [0; MAX_PLY + 1],
            excluded_at_ply: [Move::NULL; MAX_PLY + 1],
            prev_move_info: [None; MAX_PLY + 1],
        }
    }

    /// Pre-populate the repetition history with zobrist keys seen prior to the current
    /// search. Callers pass `Engine::game_key_history()` here so 3-fold draws that span
    /// across search invocations are detected correctly.
    pub(crate) fn seed_game_history(&mut self, keys: &[u64]) {
        self.key_stack.clear();
        self.key_stack.extend_from_slice(keys);
        // We don't have per-move metadata for historical plies — treat them conservatively
        // as quiet, non-checking moves. This is a heuristic: the worst that happens is we
        // occasionally fail to classify an older perpetual cycle, which is acceptable.
        self.meta_stack.clear();
        self.meta_stack.resize(keys.len(), PlyMeta::default());
    }

    /// Run the iterative-deepening search. `callback` is invoked once per completed
    /// iteration (useful for streaming depth/score/pv to the caller).
    pub(crate) fn run(mut self, limits: Limits, mut callback: impl FnMut(&SearchInfo)) -> SearchInfo {
        self.start = Instant::now();
        self.nodes = 0;
        self.max_depth = limits.max_depth.clamp(1, MAX_SEARCH_DEPTH);
        self.hard_limit = limits.max_time;
        self.soft_limit = limits.max_time.map(|t| t / 2 + t / 8); // ~62.5% of budget
        self.node_limit = limits.max_nodes;
        self.tt.bump_age();
        self.history.clear();

        let mut prev_score = 0i32;
        let mut best_info = SearchInfo::default();

        for depth in 1..=self.max_depth {
            // Lazy SMP: helper threads skip select depths so that workers explore the
            // search tree at staggered horizons instead of redundantly racing the main
            // thread. Helpers always run depth 1 (cheap and seeds the TT) and any depth
            // that survives the skip pattern.
            if depth > 1 && should_skip_depth(self.thread_id, depth) {
                continue;
            }

            self.pv_len = [0; MAX_PLY];

            let mut alpha = -INF;
            let mut beta = INF;
            // Per-thread aspiration delta dispersion — helpers cast a slightly wider net
            // so that they don't all re-search on the same fail-high/low boundary.
            let aspiration_seed = ASPIRATION_DELTA + (self.thread_id as i32) * 4;
            let mut delta = aspiration_seed;

            if depth >= 5 {
                alpha = (prev_score - aspiration_seed).max(-INF);
                beta = (prev_score + aspiration_seed).min(INF);
            }

            let score = loop {
                let s = self.alpha_beta(alpha, beta, depth as i32, 0, false);
                if self.stop_requested() && depth > 1 {
                    break s;
                }
                if s <= alpha {
                    beta = (alpha + beta) / 2;
                    alpha = (alpha - delta).max(-INF);
                    delta += delta / 2;
                } else if s >= beta {
                    beta = (beta + delta).min(INF);
                    delta += delta / 2;
                } else {
                    break s;
                }
            };

            if self.stop_requested() && depth > 1 {
                break;
            }

            prev_score = score;
            let pv_line = self.pv[0][..self.pv_len[0]].to_vec();
            let best_move = pv_line.first().copied();
            let elapsed = self.start.elapsed();
            let nps = if elapsed.as_micros() > 0 { (self.nodes as f64 / elapsed.as_secs_f64()) as u64 } else { 0 };
            best_info = SearchInfo { depth, score, best_move, pv: pv_line, nodes: self.nodes, time: elapsed, nps };
            callback(&best_info);

            if score.abs() > WIN_VALUE {
                break;
            }

            // Soft-limit: if we've used more than the soft budget we won't start another iter.
            if let Some(soft) = self.soft_limit
                && elapsed >= soft
            {
                break;
            }
        }

        best_info
    }

    // ------------------------------------------------------------
    // Time / node checks
    // ------------------------------------------------------------

    #[inline]
    fn stop_requested(&self) -> bool { self.stop.load(Ordering::Relaxed) }

    #[inline]
    fn check_stop(&self) {
        // Lightweight; only checked every ~4096 nodes by the caller.
        if let Some(limit) = self.hard_limit
            && self.start.elapsed() >= limit
        {
            self.stop.store(true, Ordering::Relaxed);
        }
        if let Some(n) = self.node_limit
            && self.nodes >= n
        {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    // ------------------------------------------------------------
    // Principal variation
    // ------------------------------------------------------------

    fn copy_pv(&mut self, ply: usize, mv: Move) {
        // Check-extension can drive `ply` up to MAX_PLY in deep forced-check sequences.
        // pv_len has MAX_PLY slots; pv has MAX_PLY+1 rows. Discard PV silently past the limit.
        if ply >= MAX_PLY {
            return;
        }
        self.pv[ply][0] = mv;
        let child = self.pv_len.get(ply + 1).copied().unwrap_or(0);
        let copy_n = child.min(MAX_PLY - 1);
        if copy_n > 0 {
            let (parent_slice, child_slice) = self.pv.split_at_mut(ply + 1);
            let src = &child_slice[0][..copy_n];
            let dst = &mut parent_slice[ply][1..1 + copy_n];
            dst.copy_from_slice(src);
        }
        self.pv_len[ply] = 1 + copy_n;
    }

    // ------------------------------------------------------------
    // Repetition
    // ------------------------------------------------------------

    /// Xiangqi-aware repetition classification.
    ///
    /// Walks the history backward (stopping at captures, which are irreversible) and, each
    /// time the same Zobrist key reappears, decides whether **we** or **the opponent** has
    /// been giving check continuously along the cycle.
    ///
    /// Returned flags:
    /// * `bit 0` – any repetition seen at all.
    /// * `bit 1` – *we* delivered check on every one of our moves in the cycle (we're the
    ///   perpetual checker → under xiangqi rules the chaser loses).
    /// * `bit 2` – the opponent delivered check on all their moves (we're the victim →
    ///   they lose).
    ///
    /// Zero means no repetition cycle was found.
    fn classify_repetition(&self, current_key: u64) -> u32 {
        let mut self_turn = false; // first iteration inspects the opponent's most-recent move.
        let mut self_perp_check = true;
        let mut opp_perp_check = true;
        // We need 1 prior occurrence of the current key plus the current position to call
        // it 2-fold. That's already enough to short-circuit a losing forced cycle inside
        // search, since another repetition gives 3-fold = textbook draw territory anyway.
        let mut i = self.key_stack.len();
        while i > 0 {
            i -= 1;
            let meta = self.meta_stack.get(i).copied().unwrap_or_default();
            if meta.was_capture {
                break;
            }
            if self_turn {
                self_perp_check &= meta.gave_check;
                if self.key_stack[i] == current_key {
                    let mut flags = 1u32;
                    if self_perp_check {
                        flags |= 2;
                    }
                    if opp_perp_check {
                        flags |= 4;
                    }
                    return flags;
                }
            } else {
                opp_perp_check &= meta.gave_check;
            }
            self_turn = !self_turn;
        }
        0
    }

    /// Map repetition flags to a search score. `ply` scales the penalty so that
    /// mate-distance scores decay cleanly along the PV.
    fn rep_value(&self, flags: u32, ply: u32) -> i32 {
        let ban = ply as i32 - BAN_VALUE;
        let mut v = 0;
        if flags & 2 != 0 {
            v += ban; // we are the chaser → heavy penalty on ourselves
        }
        if flags & 4 != 0 {
            v -= ban; // opponent is the chaser → big positive for us
        }
        if v == 0 { draw_value(ply) } else { v }
    }

    // ------------------------------------------------------------
    // Alpha-Beta with PVS, null move, LMR, TT, and quiesce at leaves.
    // ------------------------------------------------------------

    fn alpha_beta(&mut self, mut alpha: i32, mut beta: i32, mut depth: i32, ply: u32, no_null: bool) -> i32 {
        // Ensure PV length reset at this ply.
        if (ply as usize) < MAX_PLY {
            self.pv_len[ply as usize] = 0;
        }

        // Leaf → quiesce.
        if depth <= 0 {
            return self.quiesce(alpha, beta, ply);
        }

        self.nodes += 1;
        if self.nodes & 0xfff == 0 {
            self.check_stop();
        }
        if self.stop_requested() {
            return 0;
        }

        let is_pv = beta - alpha > 1;
        let us = self.pos.side_to_move();
        let in_check = self.pos.is_in_check(us);

        // Mate distance pruning.
        alpha = alpha.max(-MATE_VALUE + ply as i32);
        beta = beta.min(MATE_VALUE - ply as i32 - 1);
        if alpha >= beta {
            return alpha;
        }

        let ply_idx = ply as usize;
        let key = self.pos.zobrist_key();

        if ply > 0 {
            let flags = self.classify_repetition(key);
            if flags != 0 {
                return self.rep_value(flags, ply);
            }
        }

        // ---------- Excluded-move context (for Singular Extensions) ----------
        // When we're in the middle of verifying whether `excluded` is singular, we must
        // skip TT probe/store and null-move pruning — otherwise we'd short-circuit the
        // verification or pollute the TT with singular-specific entries.
        let excluded = if ply_idx <= MAX_PLY { self.excluded_at_ply[ply_idx] } else { Move::NULL };

        // ---------- TT probe ----------
        let tt_entry = if excluded.is_null() { self.tt.probe(key) } else { None };
        let mut tt_move = Move::NULL;
        if let Some(hit) = tt_entry {
            tt_move = hit.mv;
            let value = mate_score_from_tt(hit.value, ply);
            if value != -MATE_VALUE && !is_pv && hit.depth >= depth {
                match hit.bound {
                    Bound::Exact => return value,
                    Bound::Alpha => {
                        if value <= alpha {
                            return value;
                        }
                    }
                    Bound::Beta => {
                        if value >= beta {
                            return value;
                        }
                    }
                }
            }
        }

        // ---------- Check extension ----------
        if in_check {
            depth += 1;
        }

        // Static evaluation — reused by reverse futility, razoring, and later futility.
        let static_eval = if in_check { -INF } else { evaluate(self.pos) };
        if ply_idx <= MAX_PLY {
            self.static_evals[ply_idx] = static_eval;
        }

        // Improving heuristic: did our static eval go up since we were last on move?
        // A "not improving" trajectory justifies more aggressive LMP / LMR reductions.
        let improving = !in_check && ply >= 2 && ply_idx <= MAX_PLY && static_eval > self.static_evals[ply_idx - 2];

        // ---------- Reverse futility pruning (static null) ----------
        if !is_pv && !in_check && depth <= 6 {
            let margin = if improving { 120 * depth } else { 160 * depth };
            if static_eval.saturating_sub(margin) >= beta && static_eval < WIN_VALUE {
                return static_eval - margin;
            }
        }

        // ---------- Razoring ----------
        if !is_pv && !in_check && depth <= 3 {
            let margin = 200 + 80 * depth;
            if static_eval + margin < alpha {
                let q = self.quiesce(alpha, beta, ply);
                if q < alpha {
                    return q;
                }
            }
        }

        // ---------- Internal Iterative Reduction / Deepening ----------
        //
        // When no TT move is available, we don't have a great move to sort first. Without
        // good ordering, alpha-beta grows wider. Two complementary tricks:
        //   * At non-PV nodes, just reduce depth by 1 (IIR — cheap, always-on for deep
        //     searches). Saves nodes by deferring real work to a later iteration.
        //   * At PV nodes, do an actual shallow search first (IID) to populate the TT.
        if tt_move.is_null() && depth >= 4 && !in_check {
            if is_pv && depth >= 6 {
                let reduced = depth - 2;
                let _ = self.alpha_beta(alpha, beta, reduced, ply, true);
                if let Some(hit) = self.tt.probe(key) {
                    tt_move = hit.mv;
                }
            } else if !is_pv {
                depth -= 1;
            }
        }

        // ---------- Null-move pruning ----------
        if !is_pv
            && !in_check
            && !no_null
            && excluded.is_null()
            && depth >= 3
            && self.pos.material(us) > NULL_OKAY_MARGIN
        {
            let r = 2 + depth / 4; // classical 2 + depth/4 reduction
            let null_info = self.pos.make_null();
            self.key_stack.push(key);
            self.meta_stack.push(PlyMeta::default());
            let score = -self.alpha_beta(-beta, -beta + 1, depth - r - 1, ply + 1, true);
            self.meta_stack.pop();
            self.key_stack.pop();
            self.pos.undo_null(null_info);
            if self.stop_requested() {
                return 0;
            }
            if score >= beta {
                return score.min(WIN_VALUE);
            }
        }

        // ---------- Staged move picker ----------
        // Look up the countermove suggestion from our most recent opponent reply.
        let countermove = if ply > 0 && ply_idx <= MAX_PLY {
            match self.prev_move_info[ply_idx - 1] {
                Some((prev_p, prev_to)) => self.history.countermove_for(prev_p, prev_to),
                None => Move::NULL,
            }
        } else {
            Move::NULL
        };
        let mut picker = MovePicker::with_history(tt_move, &self.history, ply as usize, countermove);

        // Futility base for quiet-move pruning inside the move loop.
        let futility_base = if !is_pv && !in_check && depth <= 3 {
            let margin = if improving { 100 * depth + 75 } else { 70 * depth + 50 };
            Some(static_eval + margin)
        } else {
            None
        };
        // Late Move Pruning limit — more generous on the improving trajectory, aggressive
        // when we're stagnating or falling behind.
        let lmp_limit: Option<u32> = if !is_pv && !in_check && depth <= 5 {
            let base = 5 + depth * depth;
            Some(if improving { base as u32 } else { (base / 2) as u32 })
        } else {
            None
        };

        let mut best_value = -INF;
        let mut best_move = Move::NULL;
        let mut bound = Bound::Alpha;
        let mut move_count: u32 = 0;
        let mut searched_any = false;

        while let Some(mv) = picker.next(self.pos, &self.history) {
            if mv == excluded {
                continue;
            }
            let is_capture_pre = self.pos.piece_at(mv.dst()).is_some();

            // ---------- Pre-make pruning (futility / LMP) ----------
            if move_count >= 1 && !is_capture_pre && mv != tt_move {
                if let Some(limit) = lmp_limit
                    && move_count + 1 > limit
                {
                    continue;
                }
                if let Some(margin) = futility_base
                    && margin < alpha
                {
                    continue;
                }
            }

            // ---------- Singular Extensions ----------
            // If the TT move is strong (proven ≥ depth-3 with a lower bound), check whether
            // any *other* move comes within `singular_beta` of it. If none does, extend the
            // TT move's search by one ply.
            let mut extension = 0;
            if excluded.is_null()
                && mv == tt_move
                && !mv.is_null()
                && depth >= 8
                && ply > 0
                && let Some(hit) = tt_entry
                && hit.bound != Bound::Alpha
                && hit.depth >= depth - 3
            {
                let tt_value = mate_score_from_tt(hit.value, ply);
                if tt_value.abs() < WIN_VALUE {
                    let singular_beta = (tt_value - 2 * depth).max(-MATE_VALUE + 1);
                    let singular_depth = (depth - 1) / 2;
                    if ply_idx <= MAX_PLY {
                        self.excluded_at_ply[ply_idx] = mv;
                    }
                    let value = self.alpha_beta(singular_beta - 1, singular_beta, singular_depth, ply, true);
                    if ply_idx <= MAX_PLY {
                        self.excluded_at_ply[ply_idx] = Move::NULL;
                    }
                    if !self.stop_requested() && value < singular_beta {
                        extension = 1;
                    }
                }
            }

            // Capture piece + dst info BEFORE making the move — used for prev_move_info.
            let mover_piece = self.pos.piece_at(mv.src());
            let undo = self.pos.make_move(mv);
            if self.pos.is_in_check(us) {
                self.pos.undo_move(mv, undo);
                continue;
            }
            // Prefetch the child-position TT cluster while we do legality / book-keeping
            // work. By the time the recursive call probes, the line should be in L1.
            self.tt.prefetch(self.pos.zobrist_key());
            move_count += 1;
            searched_any = true;
            let is_capture = undo.captured.is_some();
            let gives_check = self.pos.is_in_check(us.flip());

            // Record the move for the child ply's countermove lookup.
            if ply_idx < MAX_PLY {
                self.prev_move_info[ply_idx + 1] = mover_piece.map(|p| (p, mv.dst().raw()));
            }

            // ---------- LMR ----------
            let mut reduction = 0;
            if depth >= 3 && move_count > 3 && !is_capture && !gives_check && !in_check {
                reduction = lmr(depth, move_count, is_pv).min(depth - 1);
                // Extra reduction on a non-improving trajectory.
                if !improving {
                    reduction += 1;
                }
                reduction = reduction.min(depth - 1).max(0);
            }

            self.key_stack.push(key);
            self.meta_stack.push(PlyMeta { gave_check: gives_check, was_capture: is_capture });

            // ---------- PVS ----------
            let new_depth = depth - 1 + extension;
            let score = if move_count == 1 {
                -self.alpha_beta(-beta, -alpha, new_depth, ply + 1, false)
            } else {
                // Zero-window scout, possibly reduced.
                let mut s = -self.alpha_beta(-alpha - 1, -alpha, new_depth - reduction, ply + 1, false);
                if s > alpha && reduction > 0 {
                    s = -self.alpha_beta(-alpha - 1, -alpha, new_depth, ply + 1, false);
                }
                if s > alpha && s < beta {
                    s = -self.alpha_beta(-beta, -alpha, new_depth, ply + 1, false);
                }
                s
            };

            self.meta_stack.pop();
            self.key_stack.pop();
            self.pos.undo_move(mv, undo);

            if self.stop_requested() {
                return 0;
            }

            if score > best_value {
                best_value = score;
                best_move = mv;
                if score > alpha {
                    alpha = score;
                    bound = Bound::Exact;
                    self.copy_pv(ply as usize, mv);
                    if score >= beta {
                        bound = Bound::Beta;
                        if !is_capture {
                            self.history.update_killer(ply as usize, mv);
                            if let Some(p) = mover_piece {
                                self.history.update_history(p, mv.dst().raw(), depth * depth);
                            }
                            // Countermove: remember the opponent's move that provoked
                            // this cutoff — for next time they play the same move.
                            if ply > 0
                                && let Some((prev_p, prev_to)) = self.prev_move_info[ply_idx - 1]
                            {
                                self.history.update_countermove(prev_p, prev_to, mv);
                            }
                        }
                        break;
                    }
                }
            }
        }

        // ---------- Terminal ----------
        if !searched_any {
            // No legal move. Xiangqi: stalemate = loss for side-to-move.
            return -MATE_VALUE + ply as i32;
        }

        if !self.stop_requested() && excluded.is_null() {
            self.tt.store(key, best_move, best_value, depth, bound, ply);
        }
        best_value
    }

    // ------------------------------------------------------------
    // Quiescence search (captures only; check evasions handled fully)
    // ------------------------------------------------------------

    fn quiesce(&mut self, mut alpha: i32, beta: i32, ply: u32) -> i32 {
        self.nodes += 1;
        if self.nodes & 0xfff == 0 {
            self.check_stop();
        }
        if self.stop_requested() {
            return 0;
        }

        if (ply as usize) >= MAX_PLY {
            return evaluate(self.pos);
        }

        let us = self.pos.side_to_move();
        let in_check = self.pos.is_in_check(us);

        let stand_pat;
        let see_prune = !in_check;
        if in_check {
            stand_pat = -INF;
        } else {
            stand_pat = evaluate(self.pos);
            if stand_pat >= beta {
                return stand_pat;
            }
            // Global delta pruning: if even capturing the most valuable opposing piece
            // can't lift us to alpha, there's no point looking at any capture.
            if stand_pat + crate::see::SEE_ROOK + 2 * crate::see::SEE_PAWN < alpha {
                return alpha;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
        }

        // When in check we must enumerate every legal reply (any quiet might dodge / block);
        // otherwise the qsearch only considers captures, generated directly to skip the
        // post-filter pass.
        let mut moves = MoveList::new();
        if in_check {
            generate_pseudo(self.pos, &mut moves);
        } else {
            generate_captures(self.pos, &mut moves);
        }

        // In the capture path, compute SEE *once* so it can both drive ordering and act as
        // a losing-capture filter — losing trades are skipped entirely. Per-capture delta
        // pruning refuses trades that can't reach alpha even with the victim for free.
        let mut list_len = 0usize;
        let mut buffer = [(Move::NULL, 0i32); crate::movegen::MAX_MOVES];
        for mv in moves.as_slice() {
            let victim = self.pos.piece_at(mv.dst());
            if !in_check {
                let victim_val = victim.map(|p| crate::see::see_value(p.kind())).unwrap_or(0);
                if stand_pat + victim_val + 150 < alpha {
                    continue;
                }
            }
            let score = if in_check {
                self.mvv_lva(*mv)
            } else {
                let s = see(self.pos, *mv);
                if see_prune && s < 0 {
                    continue;
                }
                s
            };
            buffer[list_len] = (*mv, score);
            list_len += 1;
        }
        // Selection sort by score descending. The index-based form is the natural shape of
        // selection sort; iterator gymnastics here would only obscure intent.
        #[allow(clippy::needless_range_loop)]
        for i in 0..list_len {
            let mut best_idx = i;
            let mut best = buffer[i].1;
            for j in (i + 1)..list_len {
                if buffer[j].1 > best {
                    best = buffer[j].1;
                    best_idx = j;
                }
            }
            buffer.swap(i, best_idx);
        }

        let mut best = if in_check { -INF } else { stand_pat };
        for entry in &buffer[..list_len] {
            let mv = entry.0;
            let undo = self.pos.make_move(mv);
            if self.pos.is_in_check(us) {
                self.pos.undo_move(mv, undo);
                continue;
            }
            self.tt.prefetch(self.pos.zobrist_key());
            let score = -self.quiesce(-beta, -alpha, ply + 1);
            self.pos.undo_move(mv, undo);
            if self.stop_requested() {
                return 0;
            }
            if score > best {
                best = score;
                if score >= beta {
                    return score;
                }
                if score > alpha {
                    alpha = score;
                }
            }
        }

        if in_check && best == -INF {
            return -MATE_VALUE + ply as i32;
        }
        best
    }

    // ------------------------------------------------------------
    // Move ordering helpers (qsearch only — alpha_beta uses MovePicker)
    // ------------------------------------------------------------

    fn mvv_lva(&self, mv: Move) -> i32 {
        if let Some(victim) = self.pos.piece_at(mv.dst()) {
            let attacker = self.pos.piece_at(mv.src()).map(|p| p.kind()).unwrap_or(PieceType::Pawn);
            piece_weight(victim.kind()) * 100 - piece_weight(attacker)
        } else {
            0
        }
    }
}

#[inline]
fn piece_weight(kind: PieceType) -> i32 {
    match kind {
        PieceType::King => 10_000,
        PieceType::Rook => 500,
        PieceType::Cannon => 300,
        PieceType::Knight => 280,
        PieceType::Advisor => 120,
        PieceType::Bishop => 120,
        PieceType::Pawn => 100,
    }
}

#[inline]
fn lmr(depth: i32, move_count: u32, is_pv: bool) -> i32 {
    // Log-based reduction: r ≈ 0.6 + ln(depth) * ln(movecount) / 2.35 (minus 1 on PV nodes).
    let d = depth.max(1) as f32;
    let m = move_count.max(1) as f32;
    let mut r = 0.6 + (d.ln() * m.ln() / 2.35);
    if is_pv {
        r -= 1.0;
    }
    (r as i32).max(0)
}
