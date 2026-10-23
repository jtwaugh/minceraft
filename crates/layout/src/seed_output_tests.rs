use super::*;

fn args(s: &str) -> Result<Args, String> {
    Args::parse(s.split_whitespace().map(String::from))
}

#[test]
fn parses_all_flags() {
    let a = args("--levels 5 --candidates 100 --top 3 --out x.toml --start 7").unwrap();
    assert_eq!(
        a,
        Args {
            levels: 5,
            candidates: 100,
            top: 3,
            out: "x.toml".into(),
            start: 7
        }
    );
    assert_eq!(a.search_params().k, 4);
    assert_eq!(
        args("--levels 1 --candidates 1 --top 1 --out o")
            .unwrap()
            .start,
        0
    );
}

#[test]
fn bad_arguments_are_errors() {
    for bad in [
        "",
        "--levels 5 --candidates 10 --top 2",
        "--levels 0 --candidates 10 --top 2 --out o",
        "--levels x --candidates 10 --top 2 --out o",
        "--levels 5 --candidates 10 --top 2 --out",
        "--levels 5 --candidates 10 --top 2 --out o --bogus 1",
    ] {
        assert!(args(bad).is_err(), "{bad}");
    }
}

#[test]
fn file_round_trips_validates_and_has_score_comments() {
    let t = Thresholds::shipped();
    let p = SearchParams {
        start: 1,
        candidates: 12,
        top: 1,
        k: 1,
        workers: Some(2),
    };
    let found = search_seeds(
        &p,
        &t,
        &ScoringConfig::shipped(),
        &SearchProgress::default(),
    );
    let best = found.first().expect("a valid seed in 12");
    let text = render(best).unwrap();
    assert!(text.contains(&format!("# score {:.4}", best.score)));
    assert!(text.contains("# spacing_regularity") && text.contains("# extra_islands"));
    assert!(text.contains("# climate_gradient") && text.contains("# biome_variety"));
    assert!(text.contains("[climate]") && text.contains("temp_start"));
    let path = std::env::temp_dir().join(format!("mince-seeds-test-{}.toml", std::process::id()));
    std::fs::write(&path, &text).unwrap();
    let loaded = LevelFile::load(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        LevelFile::from_toml(&loaded.to_toml().unwrap()).unwrap(),
        loaded
    );
    assert_eq!(loaded, level_file(best));
    let (layout, _) = build_route_layout(&loaded.world, &best.route);
    for level in &loaded.levels {
        assert!(validate_level(&layout, level, &t).passed());
    }
}
