use crate::{Habitats, SeaFlora};

#[test]
fn baked_table_matches_the_file() {
    let text = include_str!("../../../data/sea_flora.toml");
    let parsed = SeaFlora::from_toml(text, &Habitats::shipped()).unwrap();
    assert_eq!(SeaFlora::shipped(), parsed);
}
