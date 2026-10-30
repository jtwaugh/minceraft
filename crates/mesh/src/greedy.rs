//! Greedy merging of `meshBlocks` in `viewer/mesh.js`: runs of equal cells in
//! a grid become rectangles, scanned in the same order as the JS so the quads
//! come out the same.

/// Largest merged quad side: uv is u8 in 1/16, so 15 blocks fit.
pub const MERGE_MAX: usize = 15;

/// Merge the `u_n` x `v_n` grid `cells` (row `v`, column `u`, `None` for no
/// face) into rectangles of equal cells at most `MERGE_MAX` a side. For each,
/// `emit(u, v, w, h, cell)` gets its first cell, its corner and its size.
pub fn merge<T: PartialEq>(
    cells: &[Option<T>],
    u_n: usize,
    v_n: usize,
    mut emit: impl FnMut(usize, usize, usize, usize, &T),
) {
    let mut done = alloc::vec![false; u_n * v_n];
    for v in 0..v_n {
        for u in 0..u_n {
            let c = v * u_n + u;
            let Some(cell) = cells[c].as_ref().filter(|_| !done[c]) else {
                continue;
            };
            let same = |done: &[bool], i: usize| !done[i] && cells[i].as_ref() == Some(cell);
            let (mut w, mut h) = (1, 1);
            while w < MERGE_MAX && u + w < u_n && same(&done, c + w) {
                w += 1;
            }
            while h < MERGE_MAX && v + h < v_n && (0..w).all(|i| same(&done, c + h * u_n + i)) {
                h += 1;
            }
            for j in 0..h {
                for i in 0..w {
                    done[c + j * u_n + i] = true;
                }
            }
            emit(u, v, w, h, cell);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn rects<T: PartialEq>(cells: &[Option<T>], n: usize) -> Vec<(usize, usize, usize, usize)> {
        let mut out = Vec::new();
        merge(cells, n, n, |u, v, w, h, _| out.push((u, v, w, h)));
        out
    }

    #[test]
    fn rectangles_split_at_fifteen_blocks() {
        let n = 20;
        let r = rects(&alloc::vec![Some(1); n * n], n);
        assert_eq!(
            r,
            [
                (0, 0, 15, 15),
                (15, 0, 5, 15),
                (0, 15, 15, 5),
                (15, 15, 5, 5)
            ]
        );
    }

    #[test]
    fn unequal_and_empty_cells_stop_a_run() {
        // row 0: a a b, row 1: a a -, row 2: a a a
        let (a, b) = (Some(1), Some(2));
        let cells = [a, a, b, a, a, None, a, a, a];
        assert_eq!(rects(&cells, 3), [(0, 0, 2, 3), (2, 0, 1, 1), (2, 2, 1, 1)]);
    }
}
