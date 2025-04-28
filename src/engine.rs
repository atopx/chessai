use std::time::Duration;
use std::time::Instant;

use crate::borad::Borad;
use crate::data;
use crate::data::piece;
use crate::shell;
use crate::state::MoveState;
use crate::state::Status;
use crate::util;

#[derive(Clone, Copy, Default)]
pub struct Hash {
    pub depth: isize,
    pub flag: isize,
    pub vl: isize,
    pub mv: isize,
    pub zobrist_lock: isize,
}

pub enum Winner {
    Red,
    Black,
    Draw,
}

pub struct Engine {
    pub board: Borad,
    pub mask: isize,
    pub hash_table: Vec<Hash>,
    pub history: Vec<isize>,
    pub killer_table: Vec<[isize; 2]>,
    pub result: isize,
    pub all_nodes: isize,
}

impl Default for Engine {
    fn default() -> Self { Self::new() }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            board: Borad::new(),
            mask: 65535,
            hash_table: vec![],
            history: vec![],
            killer_table: vec![],
            result: 0,
            all_nodes: 0,
        }
    }

    pub fn from_fen(&mut self, fen: &str) { self.board.from_fen(fen); }

    pub fn to_fen(&mut self) -> String { self.board.to_fen() }

    pub fn winner(&mut self) -> Option<Winner> {
        if self.board.has_mate() {
            return match 1 - self.board.sd_player {
                0 => Some(Winner::Red),
                1 => Some(Winner::Black),
                _ => Some(Winner::Draw),
            };
        };
        let pc = piece::KING + util::side_tag(self.board.sd_player);
        let mut mate = 0;

        for i in 0..self.board.squares.len() {
            if self.board.squares[i] == pc {
                mate = i;
                break;
            }
        }
        if mate == 0 {
            return match 1 - self.board.sd_player {
                0 => Some(Winner::Red),
                1 => Some(Winner::Black),
                _ => Some(Winner::Draw),
            };
        }
        let mut vl_rep = self.board.rep_status(3);
        if vl_rep > 0 {
            vl_rep = self.board.rep_value(vl_rep);
            if -data::WIN_VALUE < vl_rep && vl_rep < data::WIN_VALUE {
                return Some(Winner::Draw);
            }
            return match self.board.sd_player {
                0 => Some(Winner::Red),
                1 => Some(Winner::Black),
                _ => Some(Winner::Draw),
            };
        }
        let mut has_material = false;
        for i in 0..self.board.squares.len() {
            if data::in_broad(i as isize) && self.board.squares[i] & 7 > 2 {
                has_material = true;
                break;
            }
        }
        if !has_material {
            return Some(Winner::Draw);
        }
        None
    }

    pub fn new_state(&mut self, hash: isize) -> MoveState {
        let mut state = MoveState::new(hash);
        if self.board.in_check() {
            state.phase = Status::REST;
            let (all_mvs, _) = self.board.generate_mvs(None);
            for mv in all_mvs {
                if !self.board.make_move(mv) {
                    continue;
                }
                self.board.undo_make_move();
                state.mvs.push(mv);
                if mv == state.hash {
                    state.vls.push(0x7fffffff);
                } else {
                    state.vls.push(self.history[self.board.history_index(mv) as usize])
                };
                shell::sort(&mut state.mvs, &mut state.vls);
                state.signle = state.mvs.len() == 1
            }
            state.hash = hash;
            // 更新杀手启发式表
            state.killer_first = self.killer_table[self.board.distance as usize][0];
            state.killer_second = self.killer_table[self.board.distance as usize][1];

            // 如果当前走法是杀手走法，则提高其优先级
            for i in 0..state.mvs.len() {
                if state.mvs[i] == state.killer_first || state.mvs[i] == state.killer_second {
                    state.vls[i] = 0x7fffffff;
                }
            }
        }
        state
    }

    pub fn next_state(&mut self, state: &mut MoveState) -> isize {
        if state.phase == Status::Hash {
            state.phase = Status::KillerFirst;
            if state.hash > 0 {
                return state.hash;
            }
        };

        if state.phase == Status::KillerFirst {
            state.phase = Status::KillerSecond;
            if state.killer_first != state.hash
                && state.killer_first > 0
                && self.board.legal_move(state.killer_first)
            {
                return state.killer_first;
            }
        };

        if state.phase == Status::KillerSecond {
            state.phase = Status::GenMoves;
            if state.killer_second != state.hash
                && state.killer_second > 0
                && self.board.legal_move(state.killer_second)
            {
                return state.killer_second;
            }
        };

        if state.phase == Status::GenMoves {
            state.phase = Status::REST;

            let (mvs, _) = self.board.generate_mvs(None);
            state.mvs = mvs;
            state.vls = vec![];
            for mv in state.mvs.iter() {
                state.vls.push(self.history[self.board.history_index(*mv) as usize]);
            }
            shell::sort(&mut state.mvs, &mut state.vls);
            state.index = 0;
        };

        while state.index < state.mvs.len() {
            let mv = state.mvs[state.index];
            state.index += 1;
            if mv != state.hash && mv != state.killer_first && mv != state.killer_second {
                return mv;
            }
        }
        0
    }

    pub fn probe_hash(&self, vl_alpha: isize, vl_beta: isize, depth: isize, mvs: &mut [isize]) -> isize {
        let hash_idx = (self.board.zobrist_key & self.mask) as usize;
        let mut hash = self.hash_table[hash_idx];
        if hash.zobrist_lock != self.board.zobrist_key {
            mvs[0] = 0;
            return -data::MATE_VALUE;
        };
        mvs[0] = hash.mv;

        let mut mate = false;

        if hash.vl > data::WIN_VALUE {
            if hash.vl <= data::BAN_VALUE {
                return -data::MATE_VALUE;
            }
            hash.vl -= self.board.distance;
            mate = true;
        } else if hash.vl < -data::WIN_VALUE {
            if hash.vl > -data::BAN_VALUE {
                return -data::MATE_VALUE;
            };
            hash.vl += self.board.distance;
            mate = true;
        } else if hash.vl == self.board.draw_value() {
            return -data::MATE_VALUE;
        };

        if hash.depth < depth && !mate {
            return -data::MATE_VALUE;
        };

        if hash.flag == data::HASH_BETA {
            if hash.vl >= vl_beta {
                return hash.vl;
            };
            return -data::MATE_VALUE;
        };

        if hash.flag == data::HASH_ALPHA {
            if hash.vl <= vl_alpha {
                return hash.vl;
            }
            return -data::MATE_VALUE;
        }
        hash.vl
    }

    pub fn record_hash(&mut self, flag: isize, vl: isize, depth: isize, mv: isize) {
        let hash_idx = self.board.zobrist_key & self.mask;
        let mut hash = self.hash_table[hash_idx as usize];
        if hash.depth > depth {
            return;
        }

        hash.flag = flag;
        hash.depth = depth;
        if vl > data::WIN_VALUE {
            if mv == 0 && vl <= data::BAN_VALUE {
                return;
            };

            hash.vl += self.board.distance;
        } else if vl < -data::WIN_VALUE {
            if mv == 0 && vl <= data::BAN_VALUE {
                return;
            }
            hash.vl -= self.board.distance;
        } else if vl == self.board.draw_value() && mv == 0 {
            return;
        } else {
            hash.vl = vl;
        };
        hash.mv = mv;
        hash.zobrist_lock = self.board.zobrist_lock;
        self.hash_table[hash_idx as usize] = hash;
    }

    pub fn set_best_move(&mut self, mv: isize, depth: isize) {
        let idx = self.board.history_index(mv) as usize;
        self.history[idx] += depth * depth;
        let killer = self.killer_table[self.board.distance as usize];
        if killer[0] != mv {
            self.killer_table[self.board.distance as usize] = [mv, killer[0]];
        };
    }

    pub fn search_pruning(&mut self, mut vl_alpha: isize, vl_beta: isize) -> isize {
        self.all_nodes += 1;

        let mut vl = self.board.mate_value();
        if vl >= vl_beta {
            return vl;
        };

        let vl_rep = self.board.rep_status(1);
        if vl_rep > 0 {
            return self.board.rep_value(vl_rep);
        };

        if self.board.distance == data::LIMIT_DEPTH as isize {
            return self.board.evaluate();
        };

        let mut vl_best = -data::MATE_VALUE;
        let mut mvs;
        let mut vls = vec![];

        if self.board.in_check() {
            (mvs, _) = self.board.generate_mvs(None);
            for mv in mvs.iter_mut() {
                vls.push(self.history[self.board.history_index(*mv) as usize]);
            }
            shell::sort(&mut mvs, &mut vls);
        } else {
            vl = self.board.evaluate();

            if vl > vl_best {
                if vl >= vl_beta {
                    return vl;
                };
                vl_best = vl;
                vl_alpha = vl_alpha.max(vl);
            };

            (mvs, vls) = self.board.generate_mvs(Some(vls));
            shell::sort(&mut mvs, &mut vls);
            for i in 0..mvs.len() {
                if vls[i] < 10
                    || (vls[i] < 20 && data::home_half(util::dst(mvs[i]), self.board.sd_player))
                {
                    mvs = mvs[0..i].to_vec();
                    break;
                }
            }
        };

        for mv in mvs {
            if !self.board.make_move(mv) {
                continue;
            }
            vl = -self.search_pruning(-vl_beta, -vl_alpha);
            self.board.undo_make_move();
            if vl > vl_best {
                if vl >= vl_beta {
                    return vl;
                }
                vl_best = vl;
                vl_alpha = vl_alpha.max(vl);
            }
        }

        if vl_best == -data::MATE_VALUE { self.board.mate_value() } else { vl_best }
    }

    pub fn search_full(
        &mut self, mut vl_alpha: isize, vl_beta: isize, depth: isize, not_null: bool,
    ) -> isize {
        if depth <= 0 {
            return self.search_pruning(vl_alpha, vl_beta);
        };

        self.all_nodes += 1;
        let mut vl = self.board.mate_value();
        if vl > vl_beta {
            return vl;
        };

        let vl_rep = self.board.rep_status(1);
        if vl_rep > 0 {
            return self.board.rep_value(vl_rep);
        };

        let mut mv_hash = vec![0];
        vl = self.probe_hash(vl_alpha, vl_beta, depth, &mut mv_hash);
        if vl > -data::MATE_VALUE {
            return vl;
        };

        if self.board.distance == data::LIMIT_DEPTH as isize {
            return self.board.evaluate();
        };

        if !not_null && !self.board.in_check() && self.board.null_okay() {
            self.board.null_move();
            vl = -self.search_full(-vl_beta, 1 - vl_beta, depth - data::NULL_DEPTH - 1, true);
            self.board.undo_null_move();
            if vl >= vl_beta
                && (self.board.null_safe()
                    || self.search_full(vl_alpha, vl_beta, depth - data::NULL_DEPTH, true) >= vl_beta)
            {
                return vl;
            }
        };

        let mut hash_flag = data::HASH_ALPHA;
        let mut vl_best = -data::MATE_VALUE;
        let mut mv_best = 0;

        let mut state = self.new_state(mv_hash[0]);
        loop {
            let mv = self.next_state(&mut state);
            if mv <= 0 {
                break;
            };
            if !self.board.make_move(mv) {
                continue;
            };

            let new_depth = match self.board.in_check() || state.signle {
                true => depth,
                false => depth - 1,
            };

            if vl_best == -data::MATE_VALUE {
                vl = -self.search_full(-vl_beta, -vl_alpha, new_depth, false);
            } else {
                vl = -self.search_full(-vl_alpha - 1, -vl_alpha, new_depth, false);
                if vl_alpha < vl && vl < vl_beta {
                    vl = -self.search_full(-vl_beta, -vl_alpha, new_depth, false);
                };
            };
            self.board.undo_make_move();
            if vl > vl_best {
                vl_best = vl;
                if vl >= vl_beta {
                    hash_flag = data::HASH_BETA;
                    mv_best = mv;
                    break;
                };
                if vl > vl_alpha {
                    vl_alpha = vl;
                    hash_flag = data::HASH_PV;
                    mv_best = mv;
                }
            };
        }

        if vl_best == -data::MATE_VALUE {
            return self.board.mate_value();
        };

        self.record_hash(hash_flag, vl_best, depth, mv_best);
        if mv_best > 0 {
            self.set_best_move(mv_best, depth);
        };
        vl_best
    }

    pub fn search_root(&mut self, depth: isize) -> isize {
        let mut vl_best: isize = -data::MATE_VALUE;

        let mut state = self.new_state(self.result);
        loop {
            let mv = self.next_state(&mut state);
            if mv <= 0 {
                break;
            };

            if !self.board.make_move(mv) {
                continue;
            };

            let new_depth: isize = match self.board.in_check() {
                true => depth,
                false => depth - 1,
            };

            let mut vl;
            if vl_best == -data::MATE_VALUE {
                vl = -self.search_full(-data::MATE_VALUE, data::MATE_VALUE, new_depth, true);
            } else {
                vl = -self.search_full(-vl_best - 1, -vl_best, new_depth, false);
                if vl > vl_best {
                    vl = -self.search_full(-data::MATE_VALUE, -vl_best, new_depth, false);
                };
            };
            self.board.undo_make_move();
            if vl > vl_best {
                vl_best = vl;
                self.result = mv;
                if vl_best > -data::WIN_VALUE && vl_best < data::WIN_VALUE {
                    vl_best +=
                        (util::randf64(data::RANDOMNESS) - util::randf64(data::RANDOMNESS)) as isize;
                    if vl_best == self.board.draw_value() {
                        vl_best -= 1;
                    }
                }
            }
        }
        self.set_best_move(self.result, depth);
        vl_best
    }

    pub fn search_unique(&mut self, vl_beta: isize, depth: isize) -> bool {
        let mut state = self.new_state(self.result);
        self.next_state(&mut state);

        loop {
            let mv = self.next_state(&mut state);
            if mv <= 0 {
                break;
            };
            if !self.board.make_move(mv) {
                continue;
            }
            let mut new_depth = depth;
            if !self.board.in_check() {
                new_depth -= 1;
            };
            let vl = -self.search_full(-vl_beta, 1 - vl_beta, new_depth, false);
            self.board.undo_make_move();
            if vl >= vl_beta {
                return false;
            }
        }
        true
    }

    pub fn search_main(&mut self, depth: isize, millis: u64) -> isize {
        self.result = self.board.book_move();
        if self.result > 0 {
            self.board.make_move(self.result);
            // 检查将军状态和重复局面
            let rep_status = self.board.rep_status(3);
            if rep_status == 0 {
                self.board.undo_make_move();
                return self.result;
            } else if rep_status & 2 != 0 {
                // 将军状态
                self.board.undo_make_move();
                return self.board.mate_value();
            };
            self.board.undo_make_move();
        };

        self.hash_table = vec![Hash::default(); self.mask as usize + 1];
        self.killer_table = vec![[0, 0]; data::LIMIT_DEPTH];
        self.history = vec![0; data::LIMIT_HISTORY];
        self.result = 0;
        self.all_nodes = 0;
        self.board.distance = 0;

        let start = Instant::now();
        let millis = Duration::from_millis(millis);
        for i in 1..depth + 1 {
            let vl = self.search_root(i);
            if Instant::now() - start >= millis {
                break;
            }
            if !(-data::WIN_VALUE..=data::WIN_VALUE).contains(&vl) {
                break;
            };
            if self.search_unique(1 - data::WIN_VALUE, i) {
                break;
            };
        }
        self.result
    }
}
