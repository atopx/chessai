use crate::data::book::Book;
use crate::data::piece;
use crate::data::{self};
use crate::history::Moved;
use crate::util;

#[derive(Debug)]
pub struct Borad {
    pub sd_player: isize,
    pub zobrist_key: isize,
    pub zobrist_lock: isize,
    pub vl_white: isize,
    pub vl_black: isize,
    pub distance: isize,
    pub moves: Vec<Moved>,
    pub squares: [isize; 256],
}

impl Default for Borad {
    fn default() -> Self { Borad::new() }
}

impl Borad {
    pub fn new() -> Self {
        Self {
            sd_player: 0,
            zobrist_key: 0,
            zobrist_lock: 0,
            vl_white: 0,
            vl_black: 0,
            distance: 0,
            moves: vec![],
            squares: [0; 256],
        }
    }

    pub fn from_fen(&mut self, fen: &str) {
        self.clearboard();
        let mut x = data::FILE_LEFT;
        let mut y = data::RANK_TOP;
        let mut index = 0;

        if fen.len() == index {
            self.set_irrev();
            return;
        }

        let mut chars = fen.chars();
        let mut c = chars.next().unwrap();
        while c != ' ' {
            if c == '/' {
                x = data::FILE_LEFT;
                y += 1;
                if y > data::RANK_BOTTOM {
                    break;
                }
            } else if ('1'..='9').contains(&c) {
                x += (c as u8 - b'0') as isize;
            } else if c.is_ascii_uppercase() {
                if x <= data::FILE_RIGHT {
                    if let Some(pt) = piece::from_char(c) {
                        self.add_piece(coord_xy(x, y), pt + 8, piece::Action::ADD);
                    };
                    x += 1;
                }
            } else if c.is_ascii_lowercase() && x <= data::FILE_RIGHT {
                if let Some(pt) = piece::from_char((c as u8 + b'A' - b'a') as char) {
                    self.add_piece(coord_xy(x, y), pt + 16, piece::Action::ADD);
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
        let player = if fen.chars().nth(index).unwrap() == 'b' { 0 } else { 1 };
        if self.sd_player == player {
            self.change_side();
        }
        self.set_irrev();
    }

    pub fn to_fen(&self) -> String {
        let mut chars: Vec<String> = Vec::new();
        for y in data::RANK_TOP..data::RANK_BOTTOM + 1 {
            let mut k = 0;
            let mut row = String::new();
            for x in data::FILE_LEFT..data::FILE_RIGHT + 1 {
                let pc = self.squares[coord_xy(x, y) as usize];
                if pc > 0 {
                    if k > 0 {
                        row.push((k as u8 + b'0') as char);
                        k = 0;
                    }
                    row.push(piece::FEN[pc as usize]);
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
        self.moves = vec![Moved::from_irrev(self.checked())];
    }

    pub fn change_side(&mut self) {
        self.sd_player = 1 - self.sd_player;
        self.zobrist_key ^= data::PRE_GEN_ZOB_RIST_KEY_PLAYER;
        self.zobrist_lock ^= data::PRE_GEN_ZOB_RIST_LOCK_PLAYER;
    }

    pub fn add_piece(&mut self, sq: isize, pc: isize, action: piece::Action) {
        self.squares[sq as usize] = match action {
            piece::Action::DEL => 0,
            piece::Action::ADD => pc,
        };

        let adjust = if pc < 16 {
            let ad = pc - 8;
            let score = piece::VALUES[ad as usize][sq as usize];
            match action {
                piece::Action::DEL => self.vl_white -= score,
                piece::Action::ADD => self.vl_white += score,
            };
            ad
        } else {
            let ad = pc - 16;
            let score = piece::VALUES[ad as usize][util::square_fltp(sq)];
            match action {
                piece::Action::DEL => self.vl_black -= score,
                piece::Action::ADD => self.vl_black += score,
            };
            ad + 7
        };
        self.zobrist_key ^= data::PRE_GEN_ZOB_RIST_KEY_TABLE[adjust as usize][sq as usize];
        self.zobrist_lock ^= data::PRE_GEN_ZOB_RIST_LOCK_TABLE[adjust as usize][sq as usize];
    }

    pub fn checked(&self) -> bool {
        let self_side = util::side_tag(self.sd_player);
        let opp_side = util::opp_side_tag(self.sd_player);

        for sq_src in 0..256 {
            if self.squares[sq_src as usize] != self_side + piece::KING {
                continue;
            }

            let side_pawn = piece::PAWN + opp_side;
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
                if self.squares[(sq_src + data::ADVISOR_DELTA[i]) as usize] != 0 {
                    continue;
                };

                let side_knight = piece::KNIGHT + opp_side;

                for n in 0..2usize {
                    if self.squares[(sq_src + data::KNIGHT_CHECK_DELTA[i][n]) as usize] == side_knight {
                        return true;
                    }
                }
            }

            for i in 0..4usize {
                let delta = data::KING_DELTA[i];
                let mut sq_dst = sq_src + delta;
                while data::in_broad(sq_dst) {
                    let pc_dst = self.squares[sq_dst as usize];
                    if pc_dst > 0 {
                        if pc_dst == piece::ROOK + opp_side || pc_dst == piece::KING + opp_side {
                            return true;
                        }
                        break;
                    }
                    sq_dst += delta;
                }
                sq_dst += delta;
                while data::in_broad(sq_dst) {
                    let pc_dst = self.squares[sq_dst as usize];
                    if pc_dst > 0 {
                        if pc_dst == piece::CANNON + opp_side {
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
            piece::KING => data::in_fort(sq_dst) && data::king_span(sq_src, sq_dst),
            piece::ADVISOR => data::in_fort(sq_dst) && data::advisor_span(sq_src, sq_dst),
            piece::BISHOP => {
                data::same_half(sq_src, sq_dst)
                    && data::bishop_span(sq_src, sq_dst)
                    && self.squares[data::bishop_pin(sq_src, sq_dst)] == 0
            }
            piece::KNIGHT => {
                let pin = data::knight_pin(sq_src, sq_dst);
                pin != sq_src && self.squares[pin as usize] == 0
            }
            piece::PAWN => {
                if data::away_half(sq_dst, self.sd_player)
                    && (sq_dst == sq_src - 1 || sq_dst == sq_src + 1)
                {
                    true
                } else {
                    sq_dst == util::square_forward(sq_src, self.sd_player)
                }
            }
            piece::ROOK | piece::CANNON => {
                let delta = if data::same_rank(sq_src, sq_dst) {
                    if sq_src > sq_dst { -1 } else { 1 }
                } else if data::same_file(sq_src, sq_dst) {
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
                    (pc_src - self_side == piece::CANNON) && pc_dst != 0
                } else {
                    (pc_src - self_side == piece::ROOK) || pc_dst == 0
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
                mirror.add_piece(util::mirror_square(i as isize), pc, piece::Action::ADD)
            }
        }

        if self.sd_player == 1 {
            mirror.change_side();
        }
        mirror
    }

    pub fn history_index(&self, mv: isize) -> isize {
        ((self.squares[util::src(mv) as usize] - 8) << 8) + util::dst(mv)
    }

    pub fn null_move(&mut self) {
        self.moves.push(Moved::from_null(self.zobrist_key));
        self.change_side();
        self.distance += 1
    }

    pub fn undo_null_move(&mut self) {
        self.distance -= 1;
        self.change_side();
        self.moves.pop().unwrap();
    }

    pub fn in_check(&self) -> bool { self.moves.last().unwrap().checked }

    pub fn captured(&self) -> bool { self.moves.last().unwrap().capture_piece > 0 }

    pub fn book_move(&self) -> isize {
        let mut mirror_opt: bool = false;
        let mut lock = util::unsigned_right_shift(self.zobrist_lock, 1);
        let mut index_opt = Book::get().search(lock);
        let book = Book::get();
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

    pub fn move_piece(&mut self, mv: isize) -> Moved {
        let sq_src = util::src(mv);
        let sq_dst = util::dst(mv);
        let pc_dst = self.squares[sq_dst as usize];
        if pc_dst > 0 {
            self.add_piece(sq_dst, pc_dst, piece::Action::DEL);
        }
        let pc_src = self.squares[sq_src as usize];

        self.add_piece(sq_src, pc_src, piece::Action::DEL);
        self.add_piece(sq_dst, pc_src, piece::Action::ADD);

        Moved::new(mv, self.zobrist_key, pc_dst, false)
    }

    pub fn make_move(&mut self, mv: isize) -> bool {
        let mut moved = self.move_piece(mv);

        if self.checked() {
            self.undo_move_piece(&moved);
            false
        } else {
            self.change_side();
            moved.checked = self.checked();
            self.moves.push(moved);
            self.distance += 1;
            true
        }
    }

    pub fn undo_make_move(&mut self) {
        self.distance -= 1;
        let moved = self.moves.pop().unwrap();
        self.change_side();
        self.undo_move_piece(&moved);
    }

    pub fn undo_move_piece(&mut self, moved: &Moved) {
        let sq_src = util::src(moved.mv);
        let sq_dst = util::dst(moved.mv);
        let pc_dst = self.squares[sq_dst as usize];

        self.add_piece(sq_dst, pc_dst, piece::Action::DEL);
        self.add_piece(sq_src, pc_dst, piece::Action::ADD);
        if moved.capture_piece > 0 {
            self.add_piece(sq_dst, moved.capture_piece, piece::Action::ADD)
        }
    }

    pub fn null_okay(&self) -> bool {
        match self.sd_player {
            0 => self.vl_white > data::NULL_OKAY_MARGIN,
            _ => self.vl_black > data::NULL_OKAY_MARGIN,
        }
    }

    pub fn null_safe(&self) -> bool {
        match self.sd_player {
            0 => self.vl_white > data::NULL_SAFE_MARGIN,
            _ => self.vl_black > data::NULL_SAFE_MARGIN,
        }
    }

    pub fn mate_value(&self) -> isize { self.distance - data::MATE_VALUE }

    pub fn ban_value(&self) -> isize { self.distance - data::BAN_VALUE }

    pub fn draw_value(&self) -> isize {
        match self.distance & 1 {
            0 => -data::DRAW_VALUE,
            _ => data::DRAW_VALUE,
        }
    }

    pub fn evaluate(&self) -> isize {
        let vl = if self.sd_player == 0 {
            (self.vl_white - self.vl_black) + data::ADVANCED_VALUE
        } else {
            (self.vl_black - self.vl_white) + data::ADVANCED_VALUE
        };
        if vl == self.draw_value() { vl - 1 } else { vl }
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
                piece::KING => {
                    for i in 0..4usize {
                        let sq_dst = sq_src as isize + data::KING_DELTA[i];

                        if !data::in_fort(sq_dst) {
                            continue;
                        }
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(data::mvv_lva(pc_dst, 5));
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
                piece::ADVISOR => {
                    for i in 0..4usize {
                        let sq_dst = sq_src as isize + data::ADVISOR_DELTA[i];

                        if !data::in_fort(sq_dst) {
                            continue;
                        }
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(data::mvv_lva(pc_dst, 1));
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
                piece::BISHOP => {
                    for i in 0..4usize {
                        let mut sq_dst = sq_src as isize + data::ADVISOR_DELTA[i];

                        if !(data::in_broad(sq_dst)
                            && data::home_half(sq_dst, self.sd_player)
                            && self.squares[sq_dst as usize] == 0)
                        {
                            continue;
                        }
                        sq_dst += data::ADVISOR_DELTA[i];
                        let pc_dst = self.squares[sq_dst as usize];

                        match vls_opt {
                            Some(_) => {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(data::mvv_lva(pc_dst, 1));
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
                piece::KNIGHT => {
                    for i in 0..4usize {
                        let mut sq_dst = sq_src.saturating_add_signed(data::KING_DELTA[i]);

                        if self.squares[sq_dst] > 0 {
                            continue;
                        }
                        for j in 0..2usize {
                            sq_dst = sq_src.saturating_add_signed(data::KNIGHT_DELTA[i][j]);
                            if !data::in_broad(sq_dst as isize) {
                                continue;
                            }
                            let pc_dst = self.squares[sq_dst];
                            match vls_opt {
                                Some(_) => {
                                    if pc_dst & opp_side != 0 {
                                        mvs.push(util::merge(sq_src as isize, sq_dst as isize));
                                        vls.push(data::mvv_lva(pc_dst, 1));
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
                piece::ROOK => {
                    for i in 0..4usize {
                        let delta = data::KING_DELTA[i];
                        let mut sq_dst = sq_src as isize + delta;

                        while data::in_broad(sq_dst) {
                            let pc_dst = self.squares[sq_dst as usize];
                            if pc_dst == 0 {
                                if vls_opt.is_none() {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                }
                            } else {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));

                                    if vls_opt.is_some() {
                                        vls.push(data::mvv_lva(pc_dst, 4));
                                    };
                                };
                                break;
                            };
                            sq_dst += delta;
                        }
                    }
                }
                piece::CANNON => {
                    for i in 0..4usize {
                        let delta = data::KING_DELTA[i];
                        let mut sq_dst = sq_src as isize + delta;
                        // i=1 delta= -1 sq_dst= 52 sq_src= 53

                        while data::in_broad(sq_dst) {
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

                        while data::in_broad(sq_dst) {
                            let pc_dst = self.squares[sq_dst as usize];
                            if pc_dst > 0 {
                                if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));

                                    if vls_opt.is_some() {
                                        vls.push(data::mvv_lva(pc_dst, 4));
                                    };
                                }
                                break;
                            }
                            sq_dst += delta;
                        }
                    }
                }
                piece::PAWN => {
                    let mut sq_dst = util::square_forward(sq_src as isize, self.sd_player);

                    if data::in_broad(sq_dst) {
                        let pc_dst = self.squares[sq_dst as usize];

                        if vls_opt.is_none() {
                            if pc_dst & self_side == 0 {
                                mvs.push(util::merge(sq_src as isize, sq_dst));
                            }
                        } else if pc_dst & opp_side != 0 {
                            mvs.push(util::merge(sq_src as isize, sq_dst));
                            vls.push(data::mvv_lva(pc_dst, 2));
                        };
                    }

                    if data::away_half(sq_src as isize, self.sd_player) {
                        for delta in [-1, 1] {
                            sq_dst = sq_src as isize + delta;
                            if data::in_broad(sq_dst) {
                                let pc_dst = self.squares[sq_dst as usize];
                                if vls_opt.is_none() {
                                    if pc_dst & self_side == 0 {
                                        mvs.push(util::merge(sq_src as isize, sq_dst));
                                    }
                                } else if pc_dst & opp_side != 0 {
                                    mvs.push(util::merge(sq_src as isize, sq_dst));
                                    vls.push(data::mvv_lva(pc_dst, 2));
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
        let mut index = self.moves.len() - 1;
        while self.moves[index].mv > 0 && self.moves[index].capture_piece == 0 {
            if side {
                perp_check = perp_check && self.moves[index].checked;
                if self.moves[index].zobrist_key == self.zobrist_key {
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
                opp_perp_check = opp_perp_check && self.moves[index].checked;
            }
            side = !side;
            index -= 1;
        }
        status
    }
}

fn coord_xy(x: isize, y: isize) -> isize { x + (y << 4) }
