use crate::util::{file_x, rank_y};

pub fn cord2uint8(cord: &str) -> isize {
    let alphabet = cord.chars().nth(0).unwrap() as isize - 'a' as isize + 3;
    let numeric = '9' as isize - cord.chars().nth(1).unwrap() as isize + 3;
    numeric << 4 | alphabet
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
    (dst << 8 | src) as isize
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move2pos() {
        let ((src_row, src_col), (dst_row, dst_col)) = move2pos(34726);
        assert_eq!(src_row, 2);
        assert_eq!(src_col, 3);
        assert_eq!(dst_row, 4);
        assert_eq!(dst_col, 4);
    }

    #[test]
    fn test_pos2iccs() {
        let src_row = 2;
        let src_col = 3;
        let dst_row = 4;
        let dst_col = 4;
        assert_eq!(pos2iccs(src_row, src_col, dst_row, dst_col), "d2e4")
    }

    #[test]
    fn test_move2iccs() {
        let t = move2iccs(22375);
        assert_eq!(t, "e6e7");
    }

    #[test]
    fn test_iccs2move() {
        let t = iccs2move("d2e4");
        assert_eq!(t, 34726)
    }

    #[test]
    fn test_iccs_moves() {
        let mvs = vec![
            "g3g4", "g6g5", "b0c2", "h7h0", "e3e4", "d9e8", "e1e2", "c6c5",
        ];
        for mv in mvs {
            assert_eq!(move2iccs(iccs2move(&mv)), mv)
        }
    }
}
