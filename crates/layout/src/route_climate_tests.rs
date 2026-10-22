use super::*;
use terrain::Climate;

const SEED: u64 = 12345;

fn mean_t(layout: &Layout, c: [i32; 2]) -> f64 {
    let climate = Climate::new(layout.seed);
    let terms = TemperatureTerms(&layout.climate);
    let tb: [&dyn terrain::TemperatureBias; 1] = [&terms];
    let mut sum = 0.0;
    let mut n = 0.0;
    for dz in (-40..=40).step_by(4) {
        for dx in (-40..=40).step_by(4) {
            sum += climate.at(c[0] + dx, c[1] + dz, &tb, &[]).0;
            n += 1.0;
        }
    }
    sum / n
}

#[test]
fn one_along_path_over_the_route_with_data_file_values() {
    let p = RouteParams::shipped();
    let route = build_route(SEED, 4);
    let (layout, _) = build_route_layout(&WorldConfig { seed: SEED }, &route);
    assert_eq!(layout.climate.len(), 1);
    let ClimateBias::AlongPath {
        points,
        d_temp_start,
        d_temp_end,
        d_humid_start,
        d_humid_end,
        half_width,
        falloff,
    } = &layout.climate[0]
    else {
        panic!("not AlongPath");
    };
    let expect: Vec<_> = route.points.iter().map(|p| (p[0], p[1])).collect();
    assert_eq!(points, &expect);
    assert_eq!((*d_humid_start, *d_humid_end), (0.0, 0.0));
    assert_eq!(
        (*d_temp_start, *d_temp_end, *half_width, *falloff),
        (
            p.temp_start,
            p.temp_end,
            p.climate_half_width,
            p.climate_falloff
        )
    );
}

#[test]
fn first_anchorage_is_colder_than_last() {
    let route = build_route(SEED, 4);
    let (layout, levels) = build_route_layout(&WorldConfig { seed: SEED }, &route);
    let first = mean_t(&layout, levels[0].anchorage);
    let last = mean_t(&layout, levels[4].anchorage);
    assert!(first < last, "first {first} last {last}");
}

#[test]
fn overlapping_regions_agree_with_the_bias() {
    let route = build_route(SEED, 2);
    let (layout, _) = build_route_layout(&WorldConfig { seed: SEED }, &route);
    for decorate in [false, true] {
        let big = layout.generate_region(-60, -60, 120, 120, decorate);
        let small = layout.generate_region(-17, -9, 40, 50, decorate);
        for z in 0..small.depth {
            for x in 0..small.width {
                let i = small.index(x, 0, z);
                let j = big.index(
                    (small.x0 + x as i32 + 60) as u32,
                    0,
                    (small.z0 + z as i32 + 60) as u32,
                );
                assert_eq!(
                    small.blocks[i..i + terrain::HEIGHT],
                    big.blocks[j..j + terrain::HEIGHT],
                    "decorate={decorate} at ({x},{z})"
                );
            }
        }
    }
}
