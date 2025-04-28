use std::time::Duration;
use std::time::Instant;

use self::state::MoveState;
use self::state::Status;

pub mod book;
pub mod position;
pub mod pregen;
pub mod state;
pub mod util;

#[derive(Clone, Copy, Default)]
pub struct Hash {
    pub depth: isize,
    pub flag: isize,
    pub vl: isize,
    pub mv: isize,
    pub zobrist_lock: isize,
}

pub struct Engine {
    pub sd_player: isize,
    pub zobrist_key: isize,
    pub zobrist_lock: isize,
    pub vl_white: isize,
    pub vl_black: isize,
    pub distance: isize,
    pub mv_list: Vec<isize>,
    pub pc_list: Vec<isize>,
    pub key_list: Vec<isize>,
    pub chk_list: Vec<bool>,
    pub squares: [isize; 256],
    pub mask: isize,
    pub hash_table: Vec<Hash>,
    pub history: Vec<isize>,
    pub killer_table: Vec<[isize; 2]>,
    pub result: isize,
    pub all_nodes: isize,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            sd_player: 0,
            zobrist_key: 0,
            zobrist_lock: 0,
            vl_white: 0,
            vl_black: 0,
            distance: 0,
            mv_list: vec![],
            pc_list: vec![],
            key_list: vec![],
            chk_list: vec![],
            squares: [0; 256],
            mask: 65535,
            hash_table: vec![],
            history: vec![],
            killer_table: vec![],
            result: 0,
            all_nodes: 0,
        }
    }

    pub fn from_fen(&mut self, fen: &str) {
        self.clearboard();
        let mut x = pregen::FILE_LEFT;
        let mut y = pregen::RANK_TOP;
        let mut index = 0;

        if fen.len() == index {
            self.set_irrev();
            return;
        }

        let mut chars = fen.chars();
        let mut c = chars.next().unwrap();
        while c != ' ' {
            if c == '/' {
                x = pregen::FILE_LEFT;
                y += 1;
                if y > pregen::RANK_BOTTOM {
                    break;
                }
            } else if ('1'..='9').contains(&c) {
                x += (c as u8 - b'0') as isize;
            } else if c.is_ascii_uppercase() {
                if x <= pregen::FILE_RIGHT {
                    if let Some(pt) = pregen::from_char(c) {
                        self.add_piece(util::coord_xy(x, y), pt + 8, pregen::PieceAction::ADD);
                    };
                    x += 1;
                }
            } else if c.is_ascii_lowercase() && x <= pregen::FILE_RIGHT {
                if let Some(pt) = pregen::from_char((c as u8 + b'A' - b'a') as char) {
                    self.add_piece(util::coord_xy(x, y), pt + 16, pregen::PieceAction::ADD);
                }
                x += 1;
            }
            index += 1;
            if index == fen.len() {
                self.set_irrev();
                return;
            }
            c = chars.next().unwrap();
        }
        index += 1;
        if index == fen.len() {
            self.set_irrev();
            return;
        }
        let player = if fen.chars().nth(index).unwrap() == 'b' {
            0
        } else {
            1
        };
        if self.sd_player == player {
            self.change_side();
        }
        self.set_irrev();
    }

    pub fn to_fen(&self) -> String {
        let mut chars: Vec<String> = Vec::new();
        for y in pregen::RANK_TOP..pregen::RANK_BOTTOM + 1 {
            let mut k = 0;
            let mut row = String::new();
            for x in pregen::FILE_LEFT..pregen::FILE_RIGHT + 1 {
                let pc = self.squares[util::coord_xy(x, y) as usize];
                if pc > 0 {
                    if k > 0 {
                        row.push((k as u8 + b'0') as char);
                        k = 0;
                    }
                    row.push(pregen::FEN_PIECE[pc as usize]);
                } else {
                    k += 1;
                }
            }
            if k > 0 {
                row.push((k as u8 + b'0') as char);
            }
            chars.push(row);
        }
        let mut fen = chars.join("/");
        if self.sd_player == 0 {
            fen.push_str(" w");
        } else {
            fen.push_str(" b");
        }
        fen
    }

    pub fn clearboard(&mut self) {
        self.sd_player = 0;
        self.zobrist_key = 0;
        self.zobrist_lock = 0;
        self.vl_black = 0;
        self.vl_white = 0;
        self.squares = [0; 256];
    }

    pub fn set_irrev(&mut self) {
        self.distance = 0;
        self.mv_list = vec![0];
        self.pc_list = vec![0];
        self.key_list = vec![0];
        self.chk_list = vec![self.checked()];
    }

    pub fn mate_value(&self) -> isize {
        self.distance - pregen::MATE_VALUE
    }

    pub fn ban_value(&self) -> isize {
        self.distance - pregen::BAN_VALUE
    }

    pub fn draw_value(&self) -> isize {
        match self.distance & 1 {
            0 => -pregen::DRAW_VALUE,
            _ => pregen::DRAW_VALUE,
        }
    }

    pub fn evaluate(&self) -> isize {
        let vl = if self.sd_player == 0 {
            (self.vl_white - self.vl_black) + pregen::ADVANCED_VALUE
        } else {
            (self.vl_black - self.vl_white) + pregen::ADVANCED_VALUE
        };
        if vl == self.draw_value() { vl - 1 } else { vl }
    }

    pub fn null_okay(&self) -> bool {
        match self.sd_player {
            0 => self.vl_white > pregen::NULL_OKAY_MARGIN,
            _ => self.vl_black > pregen::NULL_OKAY_MARGIN,
        }
    }

    pub fn null_safe(&self) -> bool {
        match self.sd_player {
            0 => self.vl_white > pregen::NULL_SAFE_MARGIN,
            _ => self.vl_black > pregen::NULL_SAFE_MARGIN,
        }
    }

    pub fn null_move(&mut self) {
        self.mv_list.push(0);
        self.pc_list.push(0);
        self.key_list.push(self.zobrist_key);
        self.change_side();
        self.chk_list.push(false);
        self.distance += 1
    }

    pub fn undo_null_move(&mut self) {
        self.distance -= 1;
        self.chk_list.pop().unwrap();
        self.change_side();
        self.key_list.pop().unwrap();
        self.pc_list.pop().unwrap();
        self.mv_list.pop().unwrap();
    }

    pub fn in_check(&self) -> bool {
        *self.chk_list.last().unwrap()
    }

    pub fn captured(&self) -> bool {
        *self.pc_list.last().unwrap() > 0
    }

    pub fn rep_value(&self, vl_rep: isize) -> isize {
        let mut vl: isize = 0;
        if vl_rep & 2 != 0 {
            vl = self.ban_value();
        };
        if vl_rep & 4 != 0 {
            vl -= self.ban_value();
        };
        match vl {
            0 => self.draw_value(),
            _ => vl,
        }
    }

    pub fn rep_status(&self, mut recur: isize) -> isize {
        let mut status = 0;
        let mut side = false;
        let mut perp_check = true;
        let mut opp_perp_check = true;
        let mut index = self.mv_list.len() - 1;
        while self.mv_list[index] > 0 && self.pc_list[index] == 0 {
            if side {
                perp_check = perp_check && self.chk_list[index];
                if self.key_list[index] == self.zobrist_key {
                    recur -= 1;
                    if recur == 0 {
                        if perp_check {
                            status += 2;
                        }
                        if opp_perp_check {
                            status += 4;
                        }
                        return status + 1;
                    }
                }
            } else {
                opp_perp_check = opp_perp_check && self.chk_list[index];
            }
            side = !side;
            index -= 1;
        }
        status
    }

    pub fn change_side(&mut self) {
        self.sd_player = 1 - self.sd_player;
        self.zobrist_key ^= pregen::PRE_GEN_ZOB_RIST_KEY_PLAYER;
        self.zobrist_lock ^= pregen::PRE_GEN_ZOB_RIST_LOCK_PLAYER;
    }

    pub fn history_index(&self, mv: isize) -> isize {
        ((self.squares[util::src(mv) as usize] - 8) << 8) + util::dst(mv)
    }

    pub fn book_move(&self) -> isize {
        let mut mirror_opt: bool = false;
        let mut lock = util::unsigned_right_shift(self.zobrist_lock, 1);
        let mut index_opt = book::Book::get().search(lock);
        let book = book::Book::get();
        if index_opt.is_none() {
            mirror_opt = true;
            lock = util::unsigned_right_shift(self.mirror().zobrist_lock, 1);
            index_opt = book.search(lock);
        };
        if index_opt.is_none() {
            return 0;
        }
        let mut index = index_opt.unwrap() - 1;
        while index > 0 && book.data[index][0] == lock {
            index -= 1;
        }
        let mut mvs = vec![];
        let mut vls = vec![];
        let mut value = 0;
        index += 1;

        while index < book.data.len() && book.data[index][0] == lock {
            let mut mv = book.data[index][1];
            if mirror_opt {
                mv = util::mirror_move(mv);
            }

            if self.legal_move(mv) {
                mvs.push(mv);
                let vl = book.data[index][2];
                vls.push(vl);
                value += vl;
            }

            index += 1;
        }
        if value == 0 {
            return 0;
        };

        value = util::randf64(value) as isize;
        for (i, vl) in vls.iter().enumerate().take(mvs.len()) {
            value -= vl;
            if value < 0 {
                index = i;
                break;
            }
        }
        mvs[index]
    }

    pub fn legal_move(&self, mv: isize) -> bool {
        let sq_src = util::src(mv);
        let pc_src = self.squares[sq_src as usize];

        let self_side = util::side_tag(self.sd_player);
        if pc_src & self_side == 0 {
            return false;
        }
        let sq_dst = util::dst(mv);
        let pc_dst = self.squares[sq_dst as usize];
        if pc_dst & self_side != 0 {
            return false;
        }

        match pc_src - self_side {
            pregen::PIECE_KING => pregen::in_fort(sq_dst) && pregen::king_span(sq_src, sq_dst),
            pregen::PIECE_ADVISOR => {
                pregen::in_fort(sq_dst) && pregen::advisor_span(sq_src, sq_dst)
            }
            pregen::PIECE_BISHOP => {
                pregen::same_half(sq_src, sq_dst)
                    && pregen::bishop_span(sq_src, sq_dst)
                    && self.squares[pregen::bishop_pin(sq_src, sq_dst)] == 0
            }
            pregen::PIECE_KNIGHT => {
                let pin = pregen::knight_pin(sq_src, sq_dst);
                pin != sq_src && self.squares[pin as usize] == 0
            }
            pregen::PIECE_PAWN => {
                if pregen::away_half(sq_dst, self.sd_player)
                    && (sq_dst == sq_src - 1 || sq_dst == sq_src + 1)
                {
                    true
                } else {
                    sq_dst == util::square_forward(sq_src, self.sd_player)
                }
            }
            pregen::PIECE_ROOK | pregen::PIECE_CANNON => {
                let delta = if pregen::same_rank(sq_src, sq_dst) {
                    if sq_src > sq_dst { -1 } else { 1 }
                } else if pregen::same_file(sq_src, sq_dst) {
                    if sq_src > sq_dst { -16 } else { 16 }
                } else {
                    return false;
                };

                let mut pin = sq_src + delta;
                let mut found_piece = false;

                while pin != sq_dst {
                    if self.squares[pin as usize] != 0 {
                        if found_piece {
                            return false;
                        }
                        found_piece = true;
                    }
                    pin += delta;
                }
                
                if found_piece {
                    (pc_src - self_side == pregen::PIECE_CANNON) && pc_dst != 0
                } else {
                    (pc_src - self_side == pregen::PIECE_ROOK) || pc_dst == 0
                }
            }
            _ => false,
        }
    }

    pub fn mirror(&self) -> Self {
        let mut mirror = Self::new();
        mirror.clearboard();
        for i in 0..mirror.squares.len() {
            let pc = self.squares[i];
            if pc > 0 {
                mirror.add_piece(
                    util::mirror_square(i as isize),
                    pc,
                    pregen::PieceAction::ADD,
                )
            }
        }

        if self.sd_player == 1 {
            mirror.change_side();
        }
        mirror
    }

    pub fn move_piece(&mut self, mv: isize) {
        let sq_src = util::src(mv);
        let sq_dst = util::dst(mv);
        let pc_dst = self.squares[sq_dst as usize];
        self.pc_list.push(pc_dst);
        if pc_dst > 0 {
            self.add_piece(sq_dst, pc_dst, pregen::PieceAction::DEL);
        }
        let pc_src = self.squares[sq_src as usize];

        self.add_piece(sq_src, pc_src, pregen::PieceAction::DEL);
        self.add_piece(sq_dst, pc_src, pregen::PieceAction::ADD);
        self.mv_list.push(mv);
    }

    pub fn make_move(&mut self, mv: isize) -> bool {
        self.move_piece(mv);

        if self.checked() {
            self.undo_move_piece();
            false
        } else {
            self.key_list.push(self.zobrist_key);
            self.change_side();
            self.chk_list.push(self.checked());
            self.distance += 1;
            true
        }
    }

    pub fn undo_make_move(&mut self) {
        self.distance -= 1;
        self.chk_list.pop().unwrap();
        self.change_side();
        self.key_list.pop().unwrap();
        self.undo_move_piece();
    }

    pub fn undo_move_piece(&mut self) {
        let mv = self.mv_list.pop().unwrap();
        let sq_src = util::src(mv);
        let sq_dst = util::dst(mv);
        let pc_dst = self.squares[sq_dst as usize];

        self.add_piece(sq_dst, pc_dst, pregen::PieceAction::DEL);
        self.add_piece(sq_src, pc_dst, pregen::PieceAction::ADD);
        let pc_src = self.pc_list.pop().unwrap();
        if pc_src > 0 {
            self.add_piece(sq_dst, pc_src, pregen::PieceAction::ADD)
        }
    }

    pub fn add_piece(&mut self, sq: isize, pc: isize, action: pregen::PieceAction) {
        self.squares[sq as usize] = match action {
            pregen::PieceAction::DEL => 0,
            pregen::PieceAction::ADD => pc,
        };

        let adjust = if pc < 16 {
            let ad = pc - 8;
            let score = pregen::PIECE_VALUE[ad as usize][sq as usize];
            match action {
                pregen::PieceAction::DEL => self.vl_white -= score,
                pregen::PieceAction::ADD => self.vl_white += score,
            };
            ad
        } else {
            let ad = pc - 16;
            let score = pregen::PIECE_VALUE[ad as usize][util::square_fltp(sq)];
            match action {
                pregen::PieceAction::DEL => self.vl_black -= score,
                pregen::PieceAction::ADD => self.vl_black += score,
            };
            ad + 7
        };
        self.zobrist_key ^= pregen::PRE_GEN_ZOB_RIST_KEY_TABLE[adjust as usize][sq as usize];
        self.zobrist_lock ^= pregen::PRE_GEN_ZOB_RIST_LOCK_TABLE[adjust as usize][sq as usize];
    }

    pub fn checked(&self) -> bool {
        let self_side = util::side_tag(self.sd_player);
        let opp_side = util::opp_side_tag(self.sd_player);

        for sq_src in 0..256 {
            if self.squares[sq_src as usize] != self_side + pregen::PIECE_KING {
                continue;
            }

            let side_pawn = pregen::PIECE_PAWN + opp_side;
            if self.squares[util::square_forward(sq_src, self.sd_player) as usize] == side_pawn {
                return true;
            }

            if self.squares[(sq_src - 1) as usize] == side_pawn {
                return true;
            }
            if self.squares[(sq_src + 1) as usize] == side_pawn {
                return true;
            }

            for i in 0..4usize {
                if self.squares[(sq_src + pregen::ADVISOR_DELTA[i]) as usize] != 0 {
                    continue;
                };

                let side_knight = pregen::PIECE_KNIGHT + opp_side;

                for n in 0..2usize {
                    if self.squares[(sq_src + pregen::KNIGHT_CHECK_DELTA[i][n]) as usize]
                        == side_knight
                    {
                        return true;
                    }
                }
            }

            for i in 0..4usize {
                let delta = pregen::KING_DELTA[i];
                let mut sq_dst = sq_src + delta;
                while pregen::in_broad(sq_dst) {
                    let pc_dst = self.squares[sq_dst as usize];
                    if pc_dst > 0 {
                        if pc_dst == pregen::PIECE_ROOK + opp_side
                            || pc_dst == pregen::PIECE_KING + opp_side
                        {
                            return true;
                        }
                        break;
                    }
                    sq_dst += delta;
                }
                sq_dst += delta;
                while pregen::in_broad(sq_dst) {
                    let pc_dst = self.squares[sq_dst as usize];
                    if pc_dst > 0 {
                        if pc_dst == pregen::PIECE_CANNON + opp_side {
                            return true;
                        }
                        break;
                    }
                    sq_dst += delta;
                }
            }
            return false;
        }
        false
    }

    pub fn generate_mvs(&self, vls_opt: Option<Vec<isize>>) -> (Vec<isize>, Vec<isize>) {
        let self_side = util::side_tag(self.sd_player);
        let opp_side = util::opp_side_tag(self.sd_player);
        let mut mvs = vec![];
        let mut vls = vec![];
        if vls_opt.is_some() {
            vls = vls_opt.clone().unwrap().to_vec();
        }

        for sq_src in 0..self.squares.len() {
            let pc_src = self.squares[sq_src];

            if pc_src & self_side == 0 {
                continue;
            }

            match pc_src - self_side {
                pregen::PIECE_KING => {
                    for i in 0..4usize {
                        let sq_dst = sq_src as isize + pregen::KING_DELTA[i];

                        if !pregen::in_fort(sq_dst) {
                            continue;
                        }
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(pregen::mvv_lva(pc_dst, 5));
                                }
                            }
                            None => {
                                if pc_dst & self_side == 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            }
                        }
                    }
                }
                pregen::PIECE_ADVISOR => {
                    for i in 0..4usize {
                        let sq_dst = sq_src as isize + pregen::ADVISOR_DELTA[i];

                        if !pregen::in_fort(sq_dst) {
                            continue;
                        }
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(pregen::mvv_lva(pc_dst, 1));
                                }
                            }
                            None => {
                                if pc_dst & self_side == 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            }
                        }
                    }
                }
                pregen::PIECE_BISHOP => {
                    for i in 0..4usize {
                        let mut sq_dst = sq_src as isize + pregen::ADVISOR_DELTA[i];

                        if !(pregen::in_broad(sq_dst)
                            && pregen::home_half(sq_dst, self.sd_player)
                            && self.squares[sq_dst as usize] == 0)
                        {
                            continue;
                        }
                        sq_dst += pregen::ADVISOR_DELTA[i];
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(pregen::mvv_lva(pc_dst, 1));
                                }
                            }
                            None => {
                                if pc_dst & self_side == 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            }
                        }
                    }
                }
                pregen::PIECE_KNIGHT => {
                    for i in 0..4usize {
                        let mut sq_dst = sq_src.saturating_add_signed(pregen::KING_DELTA[i]);

                        if self.squares[sq_dst] > 0 {
                            continue;
                        }
                        for j in 0..2usize {
                            sq_dst = sq_src.saturating_add_signed(pregen::KNIGHT_DELTA[i][j]);
                            if !pregen::in_broad(sq_dst as isize) {
                                continue;
                            }
                            let pc_dst = self.squares[sq_dst];
                            match vls_opt {
                                Some(_) => {
                                    if pc_dst & opp_side != 0 {
                                        mvs.push(util::merge(sq_src as isize, sq_dst as isize));
                                        vls.push(pregen::mvv_lva(pc_dst, 1));
                                    }
                                }
                                None => {
                                    if pc_dst & self_side == 0 {
                                        mvs.push(util::merge(sq_src as isize, sq_dst as isize));
                                    }
                                }
                            }
                        }
                    }
                }
                pregen::PIECE_ROOK => {
                    for i in 0..4usize {
                        let delta = pregen::KING_DELTA[i];
                        let mut sq_dst = sq_src as isize + delta;

                        while pregen::in_broad(sq_dst) {
                            let pc_dst = self.squares[sq_dst as usize];
                            if pc_dst == 0 {
                                if vls_opt.is_none() {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            } else {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));

                                    if vls_opt.is_some() {
                                        vls.push(pregen::mvv_lva(pc_dst, 4));
                                    };
                                };
                                break;
                            };
                            sq_dst += delta;
                        }
                    }
                }
                pregen::PIECE_CANNON => {
                    for i in 0..4usize {
                        let delta = pregen::KING_DELTA[i];
                        let mut sq_dst = sq_src as isize + delta;
                        // i=1 delta= -1 sq_dst= 52 sq_src= 53

                        while pregen::in_broad(sq_dst) {
                            let pc_dst = self.squares[sq_dst as usize];
                            if pc_dst == 0 {
                                if vls_opt.is_none() {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            } else {
                                break;
                            };
                            sq_dst += delta;
                        }
                        sq_dst += delta;

                        while pregen::in_broad(sq_dst) {
                            let pc_dst = self.squares[sq_dst as usize];
                            if pc_dst > 0 {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));

                                    if vls_opt.is_some() {
                                        vls.push(pregen::mvv_lva(pc_dst, 4));
                                    };
                                }
                                break;
                            }
                            sq_dst += delta;
                        }
                    }
                }
                pregen::PIECE_PAWN => {
                    let mut sq_dst = util::square_forward(sq_src as isize, self.sd_player);

                    if pregen::in_broad(sq_dst) {
                        let pc_dst = self.squares[sq_dst as usize];

                        if vls_opt.is_none() {
                            if pc_dst & self_side == 0 {
                                mvs.push(util::merge(sq_src as isize, sq_dst));
                            }
                        } else if pc_dst & opp_side != 0 {
                            mvs.push(util::merge(sq_src as isize, sq_dst));
                            vls.push(pregen::mvv_lva(pc_dst, 2));
                        };
                    }

                    if pregen::away_half(sq_src as isize, self.sd_player) {
                        for delta in [-1, 1] {
                            sq_dst = sq_src as isize + delta;
                            if pregen::in_broad(sq_dst) {
                                let pc_dst = self.squares[sq_dst as usize];
                                if vls_opt.is_none() {
                                    if pc_dst & self_side == 0 {
                                        mvs.push(util::merge(sq_src as isize, sq_dst));
                                    }
                                } else if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(pregen::mvv_lva(pc_dst, 2));
                                }
                            }
                        }
                    }
                }
                _ => continue,
            };
        }

        (mvs, vls)
    }

    pub fn has_mate(&mut self) -> bool {
        let (mvs, _) = self.generate_mvs(None);
        for mv in mvs {
            if self.make_move(mv) {
                self.undo_make_move();
                return false;
            }
        }
        true
    }

    pub fn winner(&mut self) -> Option<pregen::Winner> {
        if self.has_mate() {
            return match 1 - self.sd_player {
                0 => Some(pregen::Winner::White),
                1 => Some(pregen::Winner::Black),
                _ => Some(pregen::Winner::Tie),
            };
        };
        let pc = pregen::PIECE_KING + util::side_tag(self.sd_player);
        let mut mate = 0;
        for i in 0..self.squares.len() {
            if self.squares[i] == pc {
                mate = i;
                break;
            }
        }
        if mate == 0 {
            return match 1 - self.sd_player {
                0 => Some(pregen::Winner::White),
                1 => Some(pregen::Winner::Black),
                _ => Some(pregen::Winner::Tie),
            };
        }
        let mut vl_rep = self.rep_status(3);
        if vl_rep > 0 {
            vl_rep = self.rep_value(vl_rep);
            if -pregen::WIN_VALUE < vl_rep && vl_rep < pregen::WIN_VALUE {
                return Some(pregen::Winner::Tie);
            }
            return match self.sd_player {
                0 => Some(pregen::Winner::White),
                1 => Some(pregen::Winner::Black),
                _ => Some(pregen::Winner::Tie),
            };
        }
        let mut has_material = false;
        for i in 0..self.squares.len() {
            if pregen::in_broad(i as isize) && self.squares[i] & 7 > 2 {
                has_material = true;
                break;
            }
        }
        if !has_material {
            return Some(pregen::Winner::Tie);
        }
        None
    }

    pub fn new_state(&mut self, hash: isize) -> MoveState {
        let mut state = MoveState::new(self.history.clone(), hash);
        if self.in_check() {
            state.phase = Status::REST;
            let (all_mvs, _) = self.generate_mvs(None);
            for mv in all_mvs {
                if !self.make_move(mv) {
                    continue;
                }
                self.undo_make_move();
                state.mvs.push(mv);
                if mv == state.hash {
                    state.vls.push(0x7fffffff);
                } else {
                    state
                        .vls
                        .push(self.history[self.history_index(mv) as usize])
                };
                util::shell_sort(&mut state.mvs, &mut state.vls);
                state.signle = state.mvs.len() == 1
            }
            state.hash = hash;
            // 更新杀手启发式表
            state.killer_first = self.killer_table[self.distance as usize][0];
            state.killer_second = self.killer_table[self.distance as usize][1];

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
                && self.legal_move(state.killer_first)
            {
                return state.killer_first;
            }
        };

        if state.phase == Status::KillerSecond {
            state.phase = Status::GenMoves;
            if state.killer_second != state.hash
                && state.killer_second > 0
                && self.legal_move(state.killer_second)
            {
                return state.killer_second;
            }
        };

        if state.phase == Status::GenMoves {
            state.phase = Status::REST;

            let (mvs, _) = self.generate_mvs(None);
            state.mvs = mvs;
            state.vls = vec![];
            for mv in state.mvs.iter() {
                state
                    .vls
                    .push(self.history[self.history_index(*mv) as usize]);
            }
            util::shell_sort(&mut state.mvs, &mut state.vls);
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

    pub fn probe_hash(
        &self,
        vl_alpha: isize,
        vl_beta: isize,
        depth: isize,
        mvs: &mut [isize],
    ) -> isize {
        let hash_idx = (self.zobrist_key & self.mask) as usize;
        let mut hash = self.hash_table[hash_idx]; // todo set hash???
        if hash.zobrist_lock != self.zobrist_key {
            mvs[0] = 0;
            return -pregen::MATE_VALUE;
        };
        mvs[0] = hash.mv;

        let mut mate = false;

        if hash.vl > pregen::WIN_VALUE {
            if hash.vl <= pregen::BAN_VALUE {
                return -pregen::MATE_VALUE;
            }
            hash.vl -= self.distance;
            mate = true;
        } else if hash.vl < -pregen::WIN_VALUE {
            if hash.vl > -pregen::BAN_VALUE {
                return -pregen::MATE_VALUE;
            };
            hash.vl += self.distance;
            mate = true;
        } else if hash.vl == self.draw_value() {
            return -pregen::MATE_VALUE;
        };

        if hash.depth < depth && !mate {
            return -pregen::MATE_VALUE;
        };

        if hash.flag == pregen::HASH_BETA {
            if hash.vl >= vl_beta {
                return hash.vl;
            };
            return -pregen::MATE_VALUE;
        };

        if hash.flag == pregen::HASH_ALPHA {
            if hash.vl <= vl_alpha {
                return hash.vl;
            }
            return -pregen::MATE_VALUE;
        }
        hash.vl
    }

    pub fn record_hash(&mut self, flag: isize, vl: isize, depth: isize, mv: isize) {
        let hash_idx = self.zobrist_key & self.mask;
        let mut hash = self.hash_table[hash_idx as usize];
        if hash.depth > depth {
            return;
        }

        hash.flag = flag;
        hash.depth = depth;
        if vl > pregen::WIN_VALUE {
            if mv == 0 && vl <= pregen::BAN_VALUE {
                return;
            };

            hash.vl += self.distance;
        } else if vl < -pregen::WIN_VALUE {
            if mv == 0 && vl <= pregen::BAN_VALUE {
                return;
            }
            hash.vl -= self.distance;
        } else if vl == self.draw_value() && mv == 0 {
            return;
        } else {
            hash.vl = vl;
        };
        hash.mv = mv;
        hash.zobrist_lock = self.zobrist_lock;
        self.hash_table[hash_idx as usize] = hash;
    }

    pub fn set_best_move(&mut self, mv: isize, depth: isize) {
        let idx = self.history_index(mv) as usize;
        self.history[idx] += depth * depth;
        let killer = self.killer_table[self.distance as usize];
        if killer[0] != mv {
            self.killer_table[self.distance as usize] = [mv, killer[0]];
        };
    }

    pub fn search_pruning(&mut self, mut vl_alpha: isize, vl_beta: isize) -> isize {
        self.all_nodes += 1;

        let mut vl = self.mate_value();
        if vl >= vl_beta {
            return vl;
        };

        let vl_rep = self.rep_status(1);
        if vl_rep > 0 {
            return self.rep_value(vl_rep);
        };

        if self.distance == pregen::LIMIT_DEPTH as isize {
            return self.evaluate();
        };

        let mut vl_best = -pregen::MATE_VALUE;
        let mut mvs;
        let mut vls = vec![];

        if self.in_check() {
            (mvs, _) = self.generate_mvs(None);
            for mv in mvs.iter_mut() {
                vls.push(self.history[self.history_index(*mv) as usize]);
            }
            util::shell_sort(&mut mvs, &mut vls);
        } else {
            vl = self.evaluate();

            if vl > vl_best {
                if vl >= vl_beta {
                    return vl;
                };
                vl_best = vl;
                vl_alpha = vl_alpha.max(vl);
            };

            (mvs, vls) = self.generate_mvs(Some(vls));
            util::shell_sort(&mut mvs, &mut vls);
            for i in 0..mvs.len() {
                if vls[i] < 10
                    || (vls[i] < 20 && pregen::home_half(util::dst(mvs[i]), self.sd_player))
                {
                    mvs = mvs[0..i].to_vec();
                    break;
                }
            }
        };

        for mv in mvs {
            if !self.make_move(mv) {
                continue;
            }
            vl = -self.search_pruning(-vl_beta, -vl_alpha);
            self.undo_make_move();
            if vl > vl_best {
                if vl >= vl_beta {
                    return vl;
                }
                vl_best = vl;
                vl_alpha = vl_alpha.max(vl);
            }
        }

        if vl_best == -pregen::MATE_VALUE {
            self.mate_value()
        } else {
            vl_best
        }
    }

    pub fn search_full(
        &mut self,
        mut vl_alpha: isize,
        vl_beta: isize,
        depth: isize,
        not_null: bool,
    ) -> isize {
        if depth <= 0 {
            return self.search_pruning(vl_alpha, vl_beta);
        };

        self.all_nodes += 1;
        let mut vl = self.mate_value();
        if vl > vl_beta {
            return vl;
        };

        let vl_rep = self.rep_status(1);
        if vl_rep > 0 {
            return self.rep_value(vl_rep);
        };

        let mut mv_hash = vec![0];
        vl = self.probe_hash(vl_alpha, vl_beta, depth, &mut mv_hash);
        if vl > -pregen::MATE_VALUE {
            return vl;
        };

        if self.distance == pregen::LIMIT_DEPTH as isize {
            return self.evaluate();
        };

        if !not_null && !self.in_check() && self.null_okay() {
            self.null_move();
            vl = -self.search_full(-vl_beta, 1 - vl_beta, depth - pregen::NULL_DEPTH - 1, true);
            self.undo_null_move();
            if vl >= vl_beta
                && (self.null_safe()
                    || self.search_full(vl_alpha, vl_beta, depth - pregen::NULL_DEPTH, true)
                        >= vl_beta)
            {
                return vl;
            }
        };

        let mut hash_flag = pregen::HASH_ALPHA;
        let mut vl_best = -pregen::MATE_VALUE;
        let mut mv_best = 0;

        let mut state = self.new_state(mv_hash[0]);
        loop {
            let mv = self.next_state(&mut state);
            if mv <= 0 {
                break;
            };
            if !self.make_move(mv) {
                continue;
            };

            let new_depth = match self.in_check() || state.signle {
                true => depth,
                false => depth - 1,
            };

            if vl_best == -pregen::MATE_VALUE {
                vl = -self.search_full(-vl_beta, -vl_alpha, new_depth, false);
            } else {
                vl = -self.search_full(-vl_alpha - 1, -vl_alpha, new_depth, false);
                if vl_alpha < vl && vl < vl_beta {
                    vl = -self.search_full(-vl_beta, -vl_alpha, new_depth, false);
                };
            };
            self.undo_make_move();
            if vl > vl_best {
                vl_best = vl;
                if vl >= vl_beta {
                    hash_flag = pregen::HASH_BETA;
                    mv_best = mv;
                    break;
                };
                if vl > vl_alpha {
                    vl_alpha = vl;
                    hash_flag = pregen::HASH_PV;
                    mv_best = mv;
                }
            };
        }

        if vl_best == -pregen::MATE_VALUE {
            return self.mate_value();
        };

        self.record_hash(hash_flag, vl_best, depth, mv_best);
        if mv_best > 0 {
            self.set_best_move(mv_best, depth);
        };
        vl_best
    }

    pub fn search_root(&mut self, depth: isize) -> isize {
        let mut vl_best: isize = -pregen::MATE_VALUE;

        let mut state = self.new_state(self.result);
        loop {
            let mv = self.next_state(&mut state);
            if mv <= 0 {
                break;
            };

            if !self.make_move(mv) {
                continue;
            };

            let new_depth: isize = match self.in_check() {
                true => depth,
                false => depth - 1,
            };

            let mut vl;
            if vl_best == -pregen::MATE_VALUE {
                vl = -self.search_full(-pregen::MATE_VALUE, pregen::MATE_VALUE, new_depth, true);
            } else {
                vl = -self.search_full(-vl_best - 1, -vl_best, new_depth, false);
                if vl > vl_best {
                    vl = -self.search_full(-pregen::MATE_VALUE, -vl_best, new_depth, false);
                };
            };
            self.undo_make_move();
            if vl > vl_best {
                vl_best = vl;
                self.result = mv;
                if vl_best > -pregen::WIN_VALUE && vl_best < pregen::WIN_VALUE {
                    vl_best += (util::randf64(pregen::RANDOMNESS)
                        - util::randf64(pregen::RANDOMNESS))
                        as isize;
                    if vl_best == self.draw_value() {
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
            if !self.make_move(mv) {
                continue;
            }
            let mut new_depth = depth;
            if !self.in_check() {
                new_depth -= 1;
            };
            let vl = -self.search_full(-vl_beta, 1 - vl_beta, new_depth, false);
            self.undo_make_move();
            if vl >= vl_beta {
                return false;
            }
        }
        true
    }

    pub fn search_main(&mut self, depth: isize, millis: u64) -> isize {
        self.result = self.book_move();
        if self.result > 0 {
            self.make_move(self.result);
            // 检查将军状态和重复局面
            let rep_status = self.rep_status(3);
            if rep_status == 0 {
                self.undo_make_move();
                return self.result;
            } else if rep_status & 2 != 0 {
                // 将军状态
                self.undo_make_move();
                return self.mate_value();
            };
            self.undo_make_move();
        };

        self.hash_table = vec![Hash::default(); self.mask as usize + 1];
        self.killer_table = vec![[0, 0]; pregen::LIMIT_DEPTH];
        self.history = vec![0; 4096];
        self.result = 0;
        self.all_nodes = 0;
        self.distance = 0;

        let start = Instant::now();
        let millis = Duration::from_millis(millis);
        for i in 1..depth + 1 {
            let vl = self.search_root(i);
            if Instant::now() - start >= millis {
                break;
            }
            if !(-pregen::WIN_VALUE..=pregen::WIN_VALUE).contains(&vl) {
                break;
            };
            if self.search_unique(1 - pregen::WIN_VALUE, i) {
                break;
            };
        }
        self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pregen::*;
    use util::*;

    #[test]
    fn test_fen() {
        let fen = "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        assert_eq!(fen, engine.to_fen());
    }

    #[test]
    fn test_engine_26215() {
        let fen = "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let mv = engine.search_main(64, 1000);
        assert_eq!(mv, 26215);
    }

    #[test]
    fn test_generate_moves() {
        let fen = "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w";
        let mut engine = Engine::new();
        let res_correct = vec![
            13637, 17477, 17221, 18245, 21829, 25925, 30021, 34117, 38213, 42309, 18520, 14424,
            22360, 22872, 23128, 23384, 26712, 30808, 34904, 39000, 22375, 26215, 26727, 25975,
            34167, 26999, 35191, 34439, 34183, 33927, 33671, 34951, 35207, 38791, 42935, 47287,
            51127,
        ];
        engine.from_fen(fen);
        let (mvs, _) = engine.generate_mvs(None);
        assert_eq!(mvs.len(), 37);
        assert_eq!(mvs, res_correct);
    }

    #[test]
    fn test_search_book() {
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let res_correct = vec![
            33683, 34197, 34711, 35225, 35739, 38052, 33956, 29860, 25764, 13476, 41892, 42404,
            42660, 42916, 43172, 43428, 46244, 39594, 35498, 31402, 27306, 15018, 43434, 43178,
            42922, 42666, 42410, 43946, 47786, 46019, 41923, 41924, 42436, 41925, 42949, 47046,
            47047, 47048, 42953, 43977, 43466, 43978, 48075, 43979,
        ];

        for _ in 0..100 {
            assert!(engine.book_move() != 0);
        }

        let (pos_actions, _) = engine.generate_mvs(None);
        assert_eq!(pos_actions, res_correct);
        assert_eq!(engine.zobrist_key, -421837250);
        assert_eq!(engine.zobrist_lock, 86398677);
    }

    #[test]
    fn test_zobrist() {
        // FIXME 为什么测试不通过
        let mut engine = Engine::new();
        engine.from_fen("9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w");
        assert_eq!(engine.zobrist_key, -1362866936);
        assert_eq!(engine.zobrist_lock, -554356577);
    }

    #[test]
    fn test_few_step() {
        let mut engine = Engine::new();
        engine.from_fen("rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1");
        let mv = engine.generate_mvs(None).0[0];
        assert_eq!(mv, 33683);
        assert!(engine.make_move(mv));

        let mv = engine.generate_mvs(None).0[0];
        assert_eq!(mv, 17203);
        assert!(engine.make_move(mv));

        let mv = engine.generate_mvs(None).0[0];
        assert_eq!(mv, 29571);
        assert!(engine.make_move(mv));

        assert_eq!(engine.zobrist_key, -513434690);
        assert_eq!(engine.zobrist_lock, -1428449623);
        assert!(!engine.checked());

        let mv = engine.generate_mvs(None).0[0];
        assert!(engine.legal_move(mv));
        assert!(!engine.legal_move(mv + 20));
    }

    #[test]
    fn test_engine_movable() {
        let fen = "1nbakabnr/r8/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/4K3R/RNBA1ABN1 w - - 0 1";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let mv = engine.search_main(64, 3000);
        assert!(mv != 0)
    }

    #[test]
    fn test_engine_19146() {
        let fen: &str = "RKBAKABR1/9/1C2C1K2/P1P1P3P/6P2/9/p1p1p1p1p/1c4k1c/9/rkbakabr1 b";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let mv = engine.search_main(64, 1000);
        assert_eq!(mv, 19146);
    }

    #[test]
    fn test_engine_22326() {
        let fen: &str = "C1nNk4/9/9/9/9/9/n1pp5/B3C4/9/3A1K3 w - - 0 1";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let mv = engine.search_main(64, 1000);
        assert_eq!(mv, 22326);
    }

    #[test]
    fn test_engine_22985() {
        let fen: &str = "4kab2/4a4/8b/9/9/9/9/9/9/4K1R2 w - - 0 1";
        let mut engine = Engine::new();
        engine.from_fen(fen);
        let mv = engine.search_main(64, 1000);
        assert_eq!(mv, 22985);
    }   

    #[test]
    fn test_puzzle_list() {

        let mut legal = 0;
        let mut gened = 0;
        let mut moved = 0;
        let mut checked = 0;
        let mut merged = 0;
        let mut looped = 0;

        let mut engine = Engine::new();
        let fen_list: [&str; 240] = [
            "9/2Cca4/3k1C3/4P1p2/4N1b2/4R1r2/4c1n2/3p1n3/2rNK4/9 w",
            "4C4/4a4/b2ank2b/9/9/1RNR1crC1/3r1p3/3cKA3/4A4/4n4 w",
            "9/4a4/3k1a3/2R3r2/1N5n1/C7c/1N5n1/2R3r2/3p1p3/4K4 w",
            "9/4P4/2NakaR2/3P1P3/2pP1cb2/3r1c3/1rPNppCn1/3K1A3/2p3n2/9 w",
            "9/9/4Nk3/3c2p2/3r2P2/3p2B2/3p2r2/4KC3/9/9 w",
            "9/9/3k1N3/9/1C5N1/9/1n5r1/9/3p1K3/9 w",
            "9/9/3a1k3/9/1N5N1/4R4/1n5r1/9/3K1p3/9 w",
            "9/3Rak3/3a1n3/1PpP1PPR1/1P5n1/1rBp1pcp1/3C1p3/3Kcr3/9/9 w",
            "9/9/5k1N1/4p1P1p/3P1C1C1/2N1r1r2/9/3ABK3/2ncpp3/1pBAc4 w",
            "1nb1ka3/4a4/4c4/2p1C4/9/3Rcr3/P8/n3C4/4Apr2/4KA3 w",
            "1PP1kab2/1R2a4/4b3R/4C4/1C7/r8/9/2n6/3p1r3/4K4 w",
            "4k4/6P2/3rP2P1/2P6/9/9/9/9/9/4K4 w",
            "3k5/5P3/3a1r3/9/9/9/9/2R6/7p1/4K4 w",
            "9/1P2k4/3a1a3/4P4/8r/9/2R6/3n5/4p4/5K3 w",
            "3aka3/3P5/7R1/4r2C1/6C2/6R2/9/3p1n3/4p4/3K5 w",
            "4ka3/2R1a4/7N1/9/9/9/4p4/2C6/2p1p1r2/1R3K3 w",
            "4k1b2/4CP3/4b4/4p4/4P4/9/4n4/3KB4/4r4/4n1rC1 w",
            "3a1k3/1C7/3a1P3/4N4/9/3n2C2/9/9/1rp1p4/3K5 w",
            "2bakcb2/1n1C1R3/9/4C4/2p1p1p2/9/2N6/6n2/3pAp1r1/4K3c w",
            "4kar2/4a2nn/4bc3/RN1r5/2bC5/9/4p4/9/4p4/3p1K3 w",
            "2bak1P2/4a4/9/6N2/9/9/9/C1nC5/1c1pRp2r/3cK4 w",
            "9/3R5/2C1k4/9/1N2P4/9/9/5n3/1r2p4/3K5 w",
            "2ca1k3/4P1r1R/4ba3/7Cp/8r/7C1/7n1/9/3p1pp2/3nK4 w",
            "9/5k3/3R5/9/2N6/9/6N2/9/1pp1p1c2/CrBK5 w",
            "1r4r2/3ca4/4k4/2pc1P3/9/9/9/9/5K1n1/5R1RC w",
            "3akab2/1C6c/N3b4/9/1N7/9/9/C8/n4p3/rc2K1p2 w",
            "2n6/6N2/3k5/2P6/9/9/2p6/1C6C/4p1r2/5K3 w",
            "4kab2/1N1Pa4/4b4/3N2p2/6Pn1/9/2P6/2n6/2p1Ap3/3AK2p1 w",
            "2ba1kbRC/2N1a4/9/4p4/4c1p2/9/9/1p2B1r2/4r1n2/3K2B2 w",
            "2bak1b1N/9/2n1ca3/3R1C3/9/9/9/C3B4/c3p2p1/1rB2K3 w",
            "2b2a2c/4a1P2/3kb4/2PN5/2nR5/9/4n4/9/4p4/5K1p1 w",
            "3k2r2/2P1a4/9/9/4N4/7r1/9/4B3C/9/4RK3 w",
            "3nk4/2P1a3R/4r4/4P4/2NC5/9/9/9/4p1p2/2r1cK3 w",
            "C3kab1r/2C1a4/b1n5c/1N7/9/9/9/9/2pc1p3/4K4 w",
            "5k2c/3PP3r/5n2b/6N2/9/8C/9/9/2r2p3/4K4 w",
            "5k3/1P7/2PP1a1P1/9/9/4R4/9/5p3/3p1p3/1p2K4 w",
            "5k3/3Cc1P1r/2c2N3/9/9/9/9/9/3p2r2/3CKR3 w",
            "3k1ar2/2P1a4/3P5/9/9/9/9/7C1/2p2p3/4K1C2 w",
            "r1b1ka3/3Pa4/b8/9/9/9/9/1C4p2/9/c3K4 w",
            "4k1P2/4a1P2/3Rb4/6R2/9/N8/9/4p4/2p1pp3/3K2p2 w",
            "9/3Pak3/9/4P4/2b6/9/9/9/9/4K4 w",
            "3k5/4c4/9/9/RR1N5/9/2n6/3p5/4p2r1/3K5 w",
            "6b2/4ak1C1/2N2aR2/4P4/2b6/9/6p1P/4B1p1r/r3Ap3/3AK3c w",
            "3P5/4ak3/3a2R1b/9/5P2N/9/9/9/2rr5/4K4 w",
            "2ba1k3/3Ra4/2N1b4/2R6/2C6/5r3/9/4rA3/5K3/9 w",
            "4kC3/9/5P3/3R5/9/9/9/2p6/4r4/3K5 w",
            "4k4/3Pa1P2/5P3/9/6b2/9/6n2/9/2C1p4/5K3 w",
            "4ka3/4a4/8b/9/4N4/9/9/1R5C1/2pCp4/3K1nnc1 w",
            "2bak1C2/3caP3/4b4/3N1Cn2/9/9/8n/9/4p4/5K3 w",
            "2bak1P1r/4aP3/b5N2/9/9/9/9/9/5r3/2R1K4 w",
            "3ak4/2PPa4/b3b4/2p1C1N2/3c5/1rB6/9/3p5/4p2p1/1CB2K3 w",
            "5ab2/4k1C2/5P3/9/9/6R2/3r2C2/5K3/4r4/3n5 w",
            "3a1a3/3k2P2/9/9/9/3Nr4/9/7C1/4A2c1/1crRKA3 w",
            "3ak1b2/4aPP2/8b/9/9/9/9/2p5r/1n1cA2CC/4K1cn1 w",
            "6b2/2Nkn2P1/2Pab2r1/2R6/2Cn3c1/9/9/3p5/4r4/3K1p3 w",
            "4kabC1/4a2P1/2N1n4/9/2N6/9/9/5n3/2pRp4/3K5 w",
            "6b2/9/3k5/4P1N2/2b6/9/9/9/3p2p2/4K4 w",
            "5ab2/1P1k5/3a4b/9/6p2/9/6P2/9/4K4/5C3 w",
            "9/5R3/3k5/2P6/9/9/6C2/1rn2p3/9/5K3 w",
            "1C1k1a1N1/1P2a4/1n7/2n6/9/9/5R3/4R4/2r2p1r1/4K4 w",
            "4ka3/c1r3n2/6N2/9/2r2N2R/9/9/1p7/1p1KC2n1/2p1cC3 w",
            "3k2P2/9/3P5/4N4/4r4/4C4/9/2n3n2/2c3p2/4K4 w",
            "3aka3/5PP2/2N6/4c4/9/R8/9/3p5/2p1r4/3K5 w",
            "3k5/9/b8/9/9/rpp6/6R2/3ABA3/2r6/3K2R2 w",
            "9/4k1PP1/9/9/9/9/9/3K1p3/4cn1C1/4r3R w",
            "6R2/3P1k3/3a1a3/9/9/6B2/9/B1r6/5p3/4K4 w",
            "4k4/4a4/4P4/9/9/9/9/4B4/9/4K4 w",
            "6R2/5k3/5a3/5R3/6p2/9/9/4rC2B/2n6/5KBr1 w",
            "6nC1/4n4/5k3/6PN1/9/5r2N/9/5r3/2p1p4/3K5 w",
            "3P1k3/9/9/5P3/6c2/3p2C2/9/9/4p4/5K3 w",
            "1n1a1k1P1/6P2/5a2c/6C2/9/9/9/9/1p2pp3/3K5 w",
            "2ba1abR1/9/4k4/7N1/4r4/9/9/1p3p3/6p2/3K5 w",
            "4c4/4ak3/5aP2/9/9/7R1/3r1r1RC/4p1n2/9/4K2c1 w",
            "3ak4/4aPP2/4b4/2r1C2N1/9/3R5/9/9/3p1p3/4KArc1 w",
            "2Rak1b2/4aN3/9/7C1/8C/7r1/9/9/3p1pc2/4K3c w",
            "N1b1kab2/3Pa4/6n2/1R7/9/1C7/6r2/5K3/4r4/3n5 w",
            "9/5k3/5a3/p2N3r1/9/9/P8/7C1/9/5K3 w",
            "2Pc5/r3a1c1R/5k3/6P2/2b3p2/9/9/9/3p1p3/4K4 w",
            "4cknr1/3Ca2R1/2Ca2Rc1/4n4/9/9/9/9/4p4/3K1p3 w",
            "1P7/4c4/5k3/6P2/3P5/1N7/6n2/4C4/2p1p4/3K5 w",
            "9/4a3P/3ak4/9/2P1P4/3C5/P2p1c3/9/4K4/r1p2r3 w",
            "2bak2rr/3Ra4/4b3N/4C4/9/9/9/2R6/3pAp3/4KAn1c w",
            "4k4/1N1Pr1P2/5C3/5n3/9/9/9/9/3p3p1/1Cp2K3 w",
            "9/4c1P2/3kb4/2C6/3P5/9/9/4BA3/4pnppc/2BKnrrpp w",
            "4r4/2P6/3kb1P2/9/1n7/7N1/9/1cr1BK1C1/6p2/4R4 w",
            "2P2R3/4r4/3k5/9/9/9/9/9/5K3/9 w",
            "3r2b2/3kaP3/3rba3/C1n6/9/3RC4/9/9/2p1p4/3K5 w",
            "3a5/4a4/4k4/9/4n3R/9/9/4rC3/1p2A4/3K5 w",
            "4k4/9/9/4P4/4p4/8R/CRp1p4/3K5/1r7/5n3 w",
            "5k2N/3r2P2/9/9/2p3bC1/9/9/9/2pp2p2/4K2p1 w",
            "3a1kb2/4a3C/4bC3/6N2/9/9/9/9/3r1p3/2p1K1p2 w",
            "3ar1b2/4n4/3akN3/9/5P3/9/9/1p2p3R/5p3/4K4 w",
            "cr6R/1c2k4/2Pab4/n8/6b2/4N2N1/9/1R7/C2p1p3/2p1K1n1r w",
            "c8/5P3/3ak4/6r2/8N/9/9/9/pp2pp3/C2K5 w",
            "3P1k1N1/1cC4R1/3nb4/4n4/9/9/9/7r1/3r1p3/4K4 w",
            "4k4/4a4/4b4/3R5/2b6/6P2/1r2Pp3/3K3CR/4r4/2Bc2B1n w",
            "2bk5/r3aR3/n1r1b4/9/9/6R2/9/3n5/4p4/1C3K3 w",
            "2b2a3/4a4/4k4/5PRc1/9/8N/4P4/3K5/2rpp4/9 w",
            "1P1ak1b2/4a4/4b4/6p2/2N4N1/1C2C4/6P2/r2ppp3/9/c3KA3 w",
            "3ak4/4a4/9/9/4R4/9/9/3AKA3/4C4/4r4 w",
            "1r1a1a3/4k4/4bc3/cN1R1C3/2b6/9/9/9/4pp3/2BK2C2 w",
            "2C3Pc1/3kaRC2/5a3/6N2/9/9/9/2r3R2/3p1pp2/4K1B2 w",
            "r2k1a3/1PP4R1/3a5/9/9/7CR/9/5n3/4r4/3K1c3 w",
            "1rb2k3/1R7/2n1b4/4p4/6R2/5CN2/9/9/4p2r1/3K5 w",
            "3k2bP1/4a4/3Pba3/8r/9/7N1/9/4C4/4p2r1/3K5 w",
            "1c3a3/4a4/3k1N3/9/4R4/C8/9/6n2/3r1r3/2p1K4 w",
            "3a1a3/3r1k3/4b4/6P2/2b6/2B6/9/9/5C3/2B1KR1rc w",
            "9/9/3ak4/9/4P4/9/6R1P/r2AB4/2r6/3K4R w",
            "2bak4/4a2r1/4c4/CNrN5/8R/6C2/9/9/2p1pp3/3K5 w",
            "4kc3/4aRN2/9/9/9/9/9/C8/2pr2pr1/4K4 w",
            "N1Paka3/9/9/6N2/7n1/9/9/4C4/r2p1p3/4K4 w",
            "3k5/2P6/2P3N2/P8/9/9/6p2/3n5/4p4/3K5 w",
            "4ck3/4a4/9/4p4/9/9/9/4K4/Cn1rA4/3N1C3 w",
            "4r4/6P2/C2aNk3/9/9/9/9/9/2p1p2r1/3K5 w",
            "2b1k1b2/3P1P3/3a1a3/7C1/9/9/9/9/1p2pp3/3K5 w",
            "3k5/9/9/2p6/9/4N4/9/9/9/4K4 w",
            "3akr3/R4r2C/3a5/9/9/9/9/6n2/C2pA4/1R2K4 w",
            "4k4/4a4/3r1a3/9/4R4/9/9/3A1A3/4K4/4C4 w",
            "2rak4/4aP1P1/9/c3P4/6N2/9/5C3/9/3r5/4K4 w",
            "3ak2P1/4aP3/9/9/9/9/9/9/2r2rC2/c3K4 w",
            "3a1k3/1Nc1a1R2/5rN2/9/9/9/9/9/3p5/4K1Rrc w",
            "5k3/3Pa4/5a3/8N/9/9/7p1/9/4K4/9 w",
            "2baka3/1P7/4b4/7C1/p8/rp7/cp7/cp7/rp3K3/nn6C w",
            "4k4/3P1P1P1/9/9/9/9/9/3p3p1/1C2pn3/3K5 w",
            "2bak4/4a2R1/4b1R2/7C1/6C2/9/9/9/5pr2/4K2cr w",
            "2Ra1k3/3ra4/6P2/9/8p/6R2/2p6/7r1/4K4/c2A5 w",
            "3k5/9/3a5/9/9/9/9/5R3/c2rA2p1/3C1K3 w",
            "3k5/9/4P4/9/9/5R3/9/9/5K3/4r1c2 w",
            "3a1a3/3k3c1/1n2RN3/5r3/8R/9/2n6/9/3r1p3/2c1K4 w",
            "4r2N1/3ka2c1/C3b1P2/4R1N2/2b6/9/4C4/9/4p1r2/5K3 w",
            "1R1a5/2P1k4/1n2b4/4c4/9/9/9/2C5R/4r1r2/5K3 w",
            "2bak1P2/2P1a3R/4b3N/1N7/9/9/9/2C1B1r1n/1p1rC4/2p1K1p2 w",
            "3k5/4P4/5c3/c8/9/9/9/B8/3C5/5KC2 w",
            "9/9/5k3/2p1P1p2/9/5pB2/9/3A5/9/2B1Kc3 w",
            "4ka2c/9/3a5/9/6rr1/1RR6/9/4p4/9/C3K4 w",
            "3a1k3/4a1P2/R3b1P2/4P4/4p4/9/9/5C3/4p1r2/3K5 w",
            "1C7/1CRPak3/4ba3/9/2b6/9/5p3/9/3pr2p1/5K3 w",
            "3k5/1PP1rnr2/9/9/4p4/2p1C4/4N4/9/7pc/2RC1K2c w",
            "3aka3/5P3/2n3n2/C3N4/1N1R5/9/9/9/3p1r3/4KArc1 w",
            "3akar2/9/7R1/5R3/3c5/9/9/9/9/3K5 w",
            "2b6/1C2k4/n3bR3/6N2/9/9/9/1R7/3p2rr1/1C2K3c w",
            "5a2C/3NakCR1/6Nc1/3r2P1p/7n1/9/9/9/3p1p3/4K2cr w",
            "5kb2/1c2R4/n1n2P3/2P6/8C/5r1R1/9/9/2p1Cp3/3AKArc1 w",
            "r8/3ka2R1/3a3C1/1n2pN3/9/R2N1n2c/5c3/2r2A3/3p1p1p1/4K1C2 w",
            "3k5/4a4/3aP3n/9/9/9/6R2/6R2/2rc1pr2/1c2K4 w",
            "5k3/1N1P4R/3aba3/6P2/9/C2c5/4r4/9/4p4/3K1cr2 w",
            "4ka3/6P2/3a5/1R4C2/4C4/9/9/9/pr2pp3/3K5 w",
            "3k2b1R/4P4/4P3b/9/9/5Cn2/9/9/4p4/2p2K1pC w",
            "3ak1b2/4a4/9/6pN1/4r1b2/9/9/2n6/3pA4/2p1KA1RC w",
            "2ba5/4ak3/bc4PN1/2p6/8p/p8/9/9/3p5/4K4 w",
            "n1ck1P3/4P2P1/4baP1b/5cP2/N8/3C1C3/5n3/5p3/R1r1p3r/3R1K3 w",
            "4k3C/2P1aN3/4b3b/7R1/9/6B2/9/2n5B/1r1c1p3/4K1c1r w",
            "2b1ka3/3Pa4/4b1c2/6RC1/3R2c2/8C/9/2n6/3p3r1/4K4 w",
            "3k4C/2N6/9/9/9/9/9/9/3p2p2/4K3n w",
            "1c2k1C2/c4P3/5a3/7R1/9/9/9/9/3rr2C1/5K3 w",
            "3a5/4ak3/2R3C2/4P1N2/9/9/9/6n2/1n3p3/c1rRK4 w",
            "3a5/4a4/5k3/9/9/8C/9/2cc1C3/2rrN4/4K4 w",
            "1Pcak4/9/3a3C1/4p4/9/9/5R3/4pR3/3pp1r2/3C1K3 w",
            "3a1kb2/4a4/4b4/5NN2/9/9/9/8C/3r4c/4K1n2 w",
            "3ak4/1P2a4/4b2R1/9/4r4/9/9/2p1CK2p/6p2/9 w",
            "1rr2ab2/2RRak3/9/1N7/9/9/9/9/2p1p4/3K5 w",
            "C3ka3/4aP1P1/9/2P6/9/9/9/9/2p2n2c/3RK1pr1 w",
            "3ak4/1C2aR3/2n1c4/7N1/9/7R1/9/4C4/4A1rnr/3AK3c w",
            "3a2b2/3k4C/3a4b/6P2/9/9/9/9/9/4K4 w",
            "3c1a3/8r/1NP1k4/5P3/5n3/9/R8/R4p3/C2Np1p2/1c3K3 w",
            "1Cb1P1r2/3k5/c1Pab4/6P2/1N3r3/9/9/4c4/2p1p4/3K5 w",
            "3ak1b2/4a4/4b4/1r7/4R3R/9/9/4B4/4A4/4K4 w",
            "3k2b2/2PN5/4bC3/8C/9/5r3/9/5p3/3nr4/3p1K3 w",
            "4ka3/4a4/4b1r2/2N6/C5pC1/3R5/3R5/4B2n1/1n2p1p2/cr1N1K2c w",
            "3aka3/9/9/9/2R6/2C1P4/9/CR2c3p/3p1p3/2p1K4 w",
            "2ba1a2N/4k4/4b3N/9/4n1n2/9/9/3K1p3/3cp4/7R1 w",
            "9/4a4/3a1k3/4R1PN1/4p4/9/2C2r3/1cpAK4/3rAp3/2n6 w",
            "3k5/9/9/8p/9/2R3B1r/9/4BA3/6r2/2R2K3 w",
            "n1b2ab2/3Pak2C/1N7/4RP3/9/4R4/9/2p1K1p2/4cr3/C3rn3 w",
            "5k3/5c3/9/9/9/4c4/9/9/9/R2K5 w",
            "3a4R/4ak1rn/9/4pC3/4cN2N/5C3/9/9/r2p1p3/4K4 w",
            "2Ra1kb2/R3c4/4b3r/9/9/9/1r5C1/9/4p4/3K5 w",
            "3akc3/2P6/3a5/4p4/4c4/2R1P4/9/9/C1R1p1pn1/2p2K3 w",
            "4ka3/2P2P3/6N2/4n1R2/9/9/9/6pr1/4AK3/2rCcNpC1 w",
            "3P2C1R/4ak3/4b3b/9/2n5r/9/9/3C1p1R1/3p1r3/4K4 w",
            "5a3/9/3ak4/9/4P4/6B1R/9/4B4/3pr2p1/5K3 w",
            "4ka3/4a2n1/3Rc4/9/9/9/2R1r4/4C4/2nr1p3/2p1K4 w",
            "2bk1a1P1/4a3c/r1PcbN3/5CP2/3R2p2/2r6/4p4/5K3/7CR/3n5 w",
            "4kab2/4a4/8b/9/9/9/9/9/9/4K1R2 w",
            "4k4/3Pa4/5a3/9/8N/7p1/9/9/5K3/9 w",
            "1N1a5/4a4/1NC2k3/4p1p2/9/2cp4R/7C1/1r2n3R/3pA4/r1c1K4 w",
            "3a1k3/2NPac3/5PP2/4r1P2/4r4/9/9/2np5/3cnCp2/2pC1K3 w",
            "2Pa5/3kaPn2/9/8R/9/C2C3N1/9/2pp1K1pp/7rr/7n1 w",
            "2Rak1b2/4aPP2/4b4/1np2N3/3n5/1N2c4/1Cc6/3p1K3/4p4/6r2 w",
            "3k1P1n1/4aPP2/NcCPPa2b/7nN/8r/9/9/4r4/3pCp3/4K4 w",
            "5N3/4P4/PP1k5/n2r5/9/7C1/4r4/4p4/1p1Np2R1/3K5 w",
            "5an2/4a4/3k5/2P1P3R/c3p4/7nR/9/3N2rC1/6pr1/3p1KcC1 w",
            "3aN1RcC/4ak3/4b3n/5P3/9/9/9/2n2cC2/3p1p3/4KAr2 w",
            "2b1k4/2PPa4/b1Ra5/9/9/1N7/5C1r1/2N2R3/3p1p3/cr1CKn1nc w",
            "c5b2/c1P1a4/2P1bk3/9/5P3/N8/6rr1/4CR3/4p4/5K3 w",
            "5a1C1/3ka4/4b4/7N1/1Rb6/9/9/5n3/2r1p4/c4K3 w",
            "2Ra5/2n1a4/4k4/5R1N1/NP1np4/9/9/9/6p2/r3cKBr1 w",
            "3ak4/2PPa3N/9/2n6/9/2r6/9/cn1CKRrRc/5p3/7C1 w",
            "C2k5/4P4/r2cb1N1b/8C/6r2/9/9/6p2/4p4/5K1R1 w",
            "1r1akab2/5R3/4b1n2/7N1/9/2B6/2r1P4/B2RC4/3KA4/c8 w",
            "3k1P3/2P6/2Ca3N1/9/2b1p1b2/3n5/C8/9/2r1r4/3K1p3 w",
            "4k2Pn/3Par3/2c1r3N/2p2n3/6N2/2c5C/1C7/R8/4pp3/3K5 w",
            "5ab2/1P1k5/3a4b/2p6/6p2/5C3/6P2/9/9/2B1K4 w",
            "1P3k3/3PP4/3P5/9/9/9/9/9/3ppC2p/5K3 w",
            "3akab2/1r7/4b4/2C4N1/9/5p3/6R2/9/4p3c/3K5 w",
            "2n2k1cr/3P4N/2n1b3b/2CR4R/2r3c1C/9/9/9/4p4/3K1p3 w",
            "2bak3C/4a4/2P1b4/6RR1/9/9/9/9/c1nr1p3/4K4 w",
            "3rk4/1R2a4/4ba3/9/4r4/9/7R1/4C4/3p5/1cBA1K3 w",
            "Ccbaka3/9/4b4/5N1n1/5R1C1/9/9/9/r2p1r3/4KR3 w",
            "C4r3/3ka4/3acr3/1N1P4C/N2P5/3n5/9/3p5/4p4/3K5 w",
            "3k5/9/3a4n/9/6P2/9/9/5R3/c2rA2p1/3C1K3 w",
            "4k3P/3R5/3R5/9/9/9/3n5/5r3/3K5/2r6 w",
            "1R1a1k3/2Cna1P2/b3b1N2/9/9/9/9/4K1R2/3r1r3/5n3 w",
            "N3ka3/2PPa3C/4b1n1b/9/4N1C1R/9/r8/6n2/3p1r3/4K4 w",
            "3k1a3/4a4/4P4/1R4C2/9/9/9/4B1r2/5p3/3AK2n1 w",
            "5k3/9/9/9/9/9/6p2/4K1p2/6p2/C8 w",
            "2ba2b1C/R1n2k3/9/1n3P3/9/8N/9/4R1p2/3p1p3/2p1K4 w",
            "3a1k2r/1R2P4/3a5/2N6/9/9/9/2n1p4/3p1p3/RC2K1Crc w",
            "2baka3/9/b4P3/9/9/4C4/9/9/9/3K5 w",
            "4k4/4C4/5P3/1R7/9/9/4r4/3A5/6pp1/5K3 w",
            "4k4/3P1P3/9/9/9/9/9/4C4/3p1p3/1pB1K4 w",
            "3a1a3/5k3/9/5N3/9/9/7C1/3A3p1/4Ar3/4K1B2 w",
            "4k4/4a3R/4Pa3/7C1/4p4/9/9/2r6/5r3/3K5 w",
            "2b1kcPP1/4a3n/4ba3/4C1R2/5RN2/9/9/3n5/3r2r2/4K4 w",
            "4ka3/2P2P3/3R1a3/4r2C1/9/9/6pC1/7p1/4r4/5K3 w",
            "4ka3/3Pa4/7Cb/8p/9/9/9/9/1p5p1/4K4 w",
            "2r1kab1r/3Ra4/4b4/1N7/3C5/9/9/1R2B4/3p1p3/c3K4 w",
            "4kaN2/2CPa4/9/5P3/6RC1/6B2/7n1/2n6/3pA4/4KABrc w",
            "7c1/3P3r1/r2ak1P2/6P2/9/9/7CR/2p1B1np1/1p2c2CR/4K4 w",
            "9/4ak1P1/5a3/p8/9/9/9/BC1c5/8C/c3K4 w",
            "2Paka1P1/7PC/n3b4/9/3N2b2/9/c1n6/3r5/1cpC4r/2BK3R1 w",
            "2n1P4/c2kaRR2/3a3c1/3P5/2P6/5N3/r8/2nC5/4r4/3p1K3 w",
            "2bk5/4a4/3Pb2R1/9/9/9/9/2p6/3K2p2/2C1rnp2 w",
            "1RC1k1b2/3na2P1/3ab4/9/9/9/6P2/2n6/3p2rr1/2pCKR3 w",
            "4k4/3P5/9/r1p6/4r4/9/2R1p4/4BK2B/1N4p2/3ARA2C w",
            "2ba1k3/4a4/n3N4/9/9/9/9/3RB1r2/2p2r3/3AK2R1 w",
            "r3k4/3P3n1/3cb3b/3N5/CRR3p2/9/9/3r5/4p1p2/5K1C1 w",
            "3a2b2/4a1P2/5k1C1/4P4/2p3b2/8p/9/9/4p4/3K5 w",
            "3ak1b2/4arP2/5P2b/9/6P2/9/9/9/9/4K4 w",
            "3ak1b1r/4a2Pn/4b4/4C4/9/9/cR7/n8/4A1p2/3AKC3 w",
        ];
        for fen in fen_list {
            engine.from_fen(fen);
            for sq_src in 0..=255 {
                if in_broad(sq_src) {
                    for sq_dst in 0..=255 {
                        if in_broad(sq_dst) {
                            looped+=1;
                            let mv = merge(sq_src, sq_dst);
                            merged += mv;
                            if engine.legal_move(mv) {
                                legal+=1;
                            }
                        }
                    }
                }
            }

            let (mvs, _) = engine.generate_mvs(None);
            for mv in &mvs {
                if engine.make_move(*mv) {
                    moved+=1;
                    if engine.checked() {
                        checked+=1;
                    }
                    engine.undo_make_move();
                }
            }
            gened += mvs.len();
        }
        assert_eq!(looped, 1944000);
        assert_eq!(merged, 63450216000);
        assert_eq!(legal, 7809);
        assert_eq!(gened, 7809);
        assert_eq!(moved, 7207);
        assert_eq!(checked, 718);
    }
}