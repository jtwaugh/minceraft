use super::*;
use crate::NavMap;

/// Map from rows of text: `.` navigable, `#` blocked.
fn map(rows: &[&str]) -> NavMap {
    NavMap {
        width: rows[0].len() as u32,
        depth: rows.len() as u32,
        navigable: rows
            .iter()
            .flat_map(|r| r.chars().map(|c| c == '.'))
            .collect(),
    }
}

fn length(path: &[[i32; 2]]) -> f64 {
    path.windows(2)
        .map(|p| {
            if p[0][0] != p[1][0] && p[0][1] != p[1][1] {
                std::f64::consts::SQRT_2
            } else {
                1.0
            }
        })
        .sum()
}

fn survivors(nav: &NavMap) -> Vec<[u32; 2]> {
    let mut v = Vec::new();
    for z in 0..nav.depth {
        for x in 0..nav.width {
            if nav.at(x, z) {
                v.push([x, z]);
            }
        }
    }
    v
}

#[test]
fn three_wide_channel_is_gone_at_half_width_two() {
    let nav = map(&["#######", "#.....#", "#.....#", "#.....#", "#######"]);
    assert!(survivors(&erode(&nav, 2)).is_empty());
}

#[test]
fn five_wide_channel_survives_as_one_column() {
    let nav = map(&[
        "#####", "#####", ".....", ".....", ".....", ".....", ".....", "#####", "#####",
    ]);
    // Window edges are unknown, so only the blocked rows erode: row 4 only.
    let out = erode(&nav, 2);
    assert_eq!(
        survivors(&out),
        vec![[0, 4], [1, 4], [2, 4], [3, 4], [4, 4]]
    );
}

#[test]
fn erosion_is_chebyshev() {
    let nav = map(&[".....", ".....", "..#..", ".....", "....."]);
    assert_eq!(survivors(&erode(&nav, 1)).len(), 25 - 9);
    assert_eq!(survivors(&erode(&nav, 0)), survivors(&nav));
}

#[test]
fn straight_and_diagonal_costs() {
    let nav = map(&["....", "....", "....", "...."]);
    let p = find_path(&nav, [0, 0], [3, 3]).unwrap();
    assert_eq!(p.len(), 4);
    assert!((length(&p) - 3.0 * std::f64::consts::SQRT_2).abs() < 1e-9);
    let p = find_path(&nav, [0, 0], [3, 1]).unwrap();
    assert!((length(&p) - (2.0 + std::f64::consts::SQRT_2)).abs() < 1e-9);
}

#[test]
fn goes_around_a_wall() {
    let nav = map(&["..#..", "..#..", "..#..", "....."]);
    let p = find_path(&nav, [0, 0], [4, 0]).unwrap();
    assert_eq!(p.first(), Some(&[0, 0]));
    assert_eq!(p.last(), Some(&[4, 0]));
    assert!(p.iter().all(|c| nav.at(c[0] as u32, c[1] as u32)));
    assert!(length(&p) > 6.0);
}

#[test]
fn blocked_returns_none() {
    let nav = map(&["..#..", "..#..", "..#.."]);
    assert_eq!(find_path(&nav, [0, 0], [4, 0]), None);
    assert_eq!(find_path(&nav, [0, 0], [2, 0]), None);
    assert_eq!(find_path(&nav, [0, 0], [9, 0]), None);
}

#[test]
fn no_cutting_between_diagonal_blocked_columns() {
    let nav = map(&[".#", "#."]);
    assert_eq!(find_path(&nav, [0, 0], [1, 1]), None);
}

#[test]
fn no_cutting_a_corner_with_one_blocked_side() {
    let nav = map(&[".#.", "...", "..."]);
    let p = find_path(&nav, [0, 0], [1, 1]).unwrap();
    assert_eq!(p, vec![[0, 0], [0, 1], [1, 1]]);
}

#[test]
fn start_equals_goal() {
    let nav = map(&["..", ".."]);
    assert_eq!(find_path(&nav, [1, 1], [1, 1]), Some(vec![[1, 1]]));
}

#[test]
fn deterministic_with_ties() {
    let nav = map(&["....", "....", "....", "...."]);
    let a = find_path(&nav, [0, 0], [3, 2]);
    assert_eq!(a, find_path(&nav, [0, 0], [3, 2]));
    assert!(a.is_some());
}

#[test]
fn path_on_eroded_map() {
    let nav = map(&[
        "#########",
        "#.......#",
        "#.......#",
        "#.......#",
        "#.......#",
        "#.......#",
        "#########",
    ]);
    let eroded = erode(&nav, 2);
    assert_eq!(survivors(&eroded), vec![[3, 3], [4, 3], [5, 3]]);
    assert_eq!(
        find_path(&eroded, [3, 3], [5, 3]),
        Some(vec![[3, 3], [4, 3], [5, 3]])
    );
    assert_eq!(find_path(&eroded, [1, 1], [5, 3]), None);
}
