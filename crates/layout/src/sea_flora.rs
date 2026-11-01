//! Sea flora entries (docs/design/sea-flora-and-shellfish.md sections 3
//! and 4): kelp columns and oyster and scallop clusters, with the habitats,
//! seabed blocks and depths each grows in. They come from
//! `data/sea_flora.toml`, parsed and validated by `build.rs`. Nothing places
//! them yet.

#[cfg(feature = "toml-io")]
use crate::habitat::Habitats;
pub use crate::sea_flora_entry::{SeaFloraEntry, SeaFloraShape};

include!(concat!(env!("OUT_DIR"), "/sea_flora.rs"));

/// The entries, in file order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SeaFlora {
    pub entries: Vec<SeaFloraEntry>,
}

impl SeaFlora {
    /// Parse a sea flora file; habitat names are resolved against `habitats`.
    #[cfg(feature = "toml-io")]
    pub fn from_toml(text: &str, habitats: &Habitats) -> Result<Self, String> {
        let habitat_index = |n: &str| habitats.id_of(n).map(|h| h.0);
        let entries =
            crate::sea_flora_rules::parse_sea_flora(text, &terrain::block_id, &habitat_index)?;
        Ok(SeaFlora { entries })
    }

    /// The shipped `data/sea_flora.toml`, baked in at build time.
    pub fn shipped() -> Self {
        SeaFlora {
            entries: shipped_entries(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&SeaFloraEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}
