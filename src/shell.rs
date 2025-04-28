const STEPS: [usize; 8] = [0, 1, 4, 13, 40, 121, 364, 1093];

pub fn sort(mvs: &mut [isize], vls: &mut [isize]) {
    if mvs.is_empty() {
        return;
    }

    let mut step_level = 1;
    while step_level < STEPS.len() && STEPS[step_level] < mvs.len() {
        step_level += 1;
    }
    step_level -= 1;

    while step_level > 0 {
        let step = STEPS[step_level];
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
