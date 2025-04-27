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

const SHELL_STEPS: [usize; 8] = [0, 1, 4, 13, 40, 121, 364, 1093];

pub fn shell_sort(mvs: &mut [isize], vls: &mut [isize]) {
    if mvs.is_empty() {
        return;
    }

    let mut step_level = 1;
    while step_level < SHELL_STEPS.len() && SHELL_STEPS[step_level] < mvs.len() {
        step_level += 1;
    }
    step_level -= 1;

    while step_level > 0 {
        let step = SHELL_STEPS[step_level];
        for i in 0..mvs.len() {
            let mv_best = mvs[i];
            let vl_best = vls[i];
            let mut j = i as isize - step as isize;
            while j >= 0 {
                let j_usize = j as usize;
                if vl_best <= vls[j_usize] {
                    break;
                }
                mvs[j_usize + step] = mvs[j_usize];
                vls[j_usize + step] = vls[j_usize];
                j -= step as isize;
            }
            let insert_pos = (j + step as isize) as usize;
            mvs[insert_pos] = mv_best;
            vls[insert_pos] = vl_best;
        }
        step_level -= 1;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsigned_right_shift() {
        let t = unsigned_right_shift(50343, 30);
        assert_eq!(t, 3);
    }

    #[test]
    fn test_shell_sort() {
        let mut mvs = vec![22599, 34697, 30615, 34713, 46758, 34728, 46760, 13749, 46773];
        let mut vls = vec![29, 36, 26, 39, 28, 39, 29, 26, 26];
        shell_sort(&mut mvs, &mut vls);
        let exp_mvs = [34728, 34713, 34697, 22599, 46760, 46758, 30615, 13749, 46773];
        let exp_vls = [39, 39, 36, 29, 29, 28, 26, 26, 26];
        for i in 0..9 {
            assert_eq!(exp_mvs[i], mvs[i]);
            assert_eq!(exp_vls[i], vls[i]);
        }
    }

    #[test]
    fn test_shell_sort_random() {
        // 生成随机测试数据
        let mut rng = rand::rng();
        let mut mvs: Vec<isize> = (0..1000).map(|_| rng.random_range(0..1000) as isize).collect();
        let mut vls = mvs.clone();

        // 执行排序
        shell_sort(&mut mvs, &mut vls);

        // 验证结果
        for i in 0..vls.len().saturating_sub(1) {
            // 检查降序排列
            assert!(
                vls[i] >= vls[i + 1],
                "Values not in descending order at positions {} and {}: {} vs {}",
                i,
                i + 1,
                vls[i],
                vls[i + 1]
            );

            // 检查元素对应关系
            assert_eq!(
                mvs[i], vls[i],
                "Mismatch between mvs and vls at position {}: {} vs {}",
                i, mvs[i], vls[i]
            );
        }
    }
}
