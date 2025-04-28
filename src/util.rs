use rand::Rng;

pub fn rank_y(sq: isize) -> isize {
    sq >> 4
}

pub fn file_x(sq: isize) -> isize {
    sq & 15
}

pub fn coord_xy(x: isize, y: isize) -> isize {
    x + (y << 4)
}

pub fn square_fltp(sq: isize) -> usize {
    (254 - sq) as usize
}

pub fn file_fltp(x: isize) -> isize {
    14 - x
}

pub fn mirror_square(sq: isize) -> isize {
    coord_xy(file_fltp(file_x(sq)), rank_y(sq))
}

pub fn square_forward(sq: isize, sd: isize) -> isize {
    sq - 16 + (sd << 5)
}

pub fn side_tag(sd: isize) -> isize {
    8 + (sd << 3)
}

pub fn opp_side_tag(sd: isize) -> isize {
    16 - (sd << 3)
}

pub fn src(mv: isize) -> isize {
    mv & 255
}

pub fn dst(mv: isize) -> isize {
    mv >> 8
}

pub fn merge(src: isize, dst: isize) -> isize {
    src + (dst << 8)
}

pub fn mirror_move(mv: isize) -> isize {
    merge(mirror_square(src(mv)), mirror_square(dst(mv)))
}

pub fn unsigned_right_shift(x: isize, y: isize) -> isize {
    let x = (x as usize) & 0xffffffff;
    (x >> (y & 0xf)) as isize
}

pub fn randf64(value: isize) -> f64 {
    let mut rng = rand::rng();
    let num: f64 = rng.random_range(0.0..1.0);
    (num * (value as f64)).floor()
}

pub fn cord2uint8(cord: &str) -> isize {
    let alphabet = cord.chars().next().unwrap() as isize - 'a' as isize + 3;
    let numeric = '9' as isize - cord.chars().nth(1).unwrap() as isize + 3;
    (numeric << 4) | alphabet
}

pub fn pos2iccs(src_row: usize, src_col: usize, dst_row: usize, dst_col: usize) -> String {
    let mut iccs = String::new();
    iccs.push(char::from(src_col as u8 + b'a'));
    iccs.push(char::from(src_row as u8 + b'0'));
    iccs.push(char::from(dst_col as u8 + b'a'));
    iccs.push(char::from(dst_row as u8 + b'0'));
    iccs
}

pub fn iccs2pos(iccs: &str) -> ((usize, usize), (usize, usize)) {
    let chars = iccs.as_bytes();
    let src_row = (chars[1] - b'a') as usize;
    let src_col = (chars[0] - b'0') as usize;
    let dst_row = (chars[3] - b'a') as usize;
    let dst_col = (chars[2] - b'0') as usize;
    ((src_row, src_col), (dst_row, dst_col))
}

pub fn move2pos(mv: isize) -> ((usize, usize), (usize, usize)) {
    let src = super::util::src(mv);
    let dst = super::util::dst(mv);
    let src_col = file_x(src) as usize - 3;
    let src_row = 12 - rank_y(src) as usize;
    let dst_col = file_x(dst) as usize - 3;
    let dst_row = 12 - rank_y(dst) as usize;
    ((src_row, src_col), (dst_row, dst_col))
}

pub fn iccs2move(iccs: &str) -> isize {
    let iccs = iccs.to_ascii_lowercase();
    let src = cord2uint8(&iccs[..2]);
    let dst = cord2uint8(&iccs[2..]);
    (dst << 8) | src
}

pub fn move2iccs(mv: isize) -> String {
    let src = super::util::src(mv);
    let dst = super::util::dst(mv);
    let mut iccs = String::new();
    iccs.push((b'a' + file_x(src) as u8 - 3) as char);
    iccs.push((b'9' - rank_y(src) as u8 + 3) as char);
    iccs.push((b'a' + file_x(dst) as u8 - 3) as char);
    iccs.push((b'9' - rank_y(dst) as u8 + 3) as char);
    iccs
}
