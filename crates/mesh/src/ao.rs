//! Vertex ambient occlusion of full-cube and leaf faces, as `faceAO` and
//! `boxFace` in `viewer/mesh.js` do it.
use crate::tables::Face;

/// AO level 0-3 of each corner of face `f` of the block at `p`: the opaque
/// blocks (`solid`) among the corner's two side neighbours and the diagonal,
/// on the face's open side.
pub fn face_ao(f: &Face, [x, y, z]: [i32; 3], solid: impl Fn(i32, i32, i32) -> bool) -> [u8; 4] {
    // the two tangent axes of the face
    let (a, b) = if f.d[0] != 0 {
        (1, 2)
    } else if f.d[1] != 0 {
        (0, 2)
    } else {
        (0, 1)
    };
    let o = [x + f.d[0], y + f.d[1], z + f.d[2]];
    f.c.map(|c| {
        let sa = if c[a] != 0 { 1 } else { -1 };
        let sb = if c[b] != 0 { 1 } else { -1 };
        let q = |da, db| {
            let mut r = o;
            r[a] += da;
            r[b] += db;
            solid(r[0], r[1], r[2]) as u8
        };
        q(sa, 0) + q(0, sb) + q(sa, sb)
    })
}

/// Does AO flip the quad's diagonal? Then its vertices are rotated by one so
/// the shared index pattern splits it the other way.
pub fn flips(ao: [u8; 4]) -> bool {
    ao[0] + ao[2] < ao[1] + ao[3]
}

/// `a` rotated to start at its second element.
pub fn rot<T: Copy>(a: [T; 4]) -> [T; 4] {
    [a[1], a[2], a[3], a[0]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::FACES;

    #[test]
    fn corner_block_gets_all_four_levels() {
        // top face of the block at the origin; solids above it on the -x and -z
        // sides and the (-x, -z) diagonal, plus the +x side
        let solid = |x: i32, y: i32, z: i32| {
            y == 1 && matches!((x, z), (-1, 0) | (0, -1) | (-1, -1) | (1, 0))
        };
        let ao = face_ao(&FACES[0], [0, 0, 0], solid);
        // corners (0,0) (0,1) (1,1) (1,0): 3 around (-,-), 1 at (-,+), 1 at (+,+), 2 at (+,-)
        assert_eq!(ao, [3, 1, 1, 2]);
        let solid = |x: i32, y: i32, z: i32| y == 1 && (x, z) == (1, 1);
        assert_eq!(face_ao(&FACES[0], [0, 0, 0], solid), [0, 0, 1, 0]);
        assert_eq!(face_ao(&FACES[0], [0, 0, 0], |_, _, _| false), [0; 4]);
    }

    #[test]
    fn diagonal_flips_when_the_other_pair_is_darker() {
        assert!(!flips([3, 1, 1, 2]));
        assert!(flips([0, 1, 0, 0]));
        assert!(!flips([1, 1, 1, 1]));
        assert_eq!(rot([0, 1, 2, 3]), [1, 2, 3, 0]);
    }
}
