// Tile names shared by the viewer (index.html) and the client (client/tiles.js).
// Each page appends what is its own (the viewer's sun_sky, the client's farm
// and planter tiles) and builds its layer order from this list. Variant runs
// are contiguous: a base tile is followed directly by its _1, _2 variants.

/** Tiles every textures.json must have for the viewer to use the tileset at all. */
export const REQUIRED = ['stone', 'dirt', 'grass_top', 'grass_side', 'sand', 'gravel',
  'water_0', 'water_1', 'water_2', 'water_3',
  'water_flow_0', 'water_flow_1', 'water_flow_2', 'water_flow_3',
  'log_side', 'log_top', 'leaves'];

export const NAMES = [
  'stone', 'dirt', 'grass_top', 'grass_top_1', 'grass_top_2', 'grass_side', 'sand', 'gravel',
  'water_0', 'water_1', 'water_2', 'water_3',
  'water_flow_0', 'water_flow_1', 'water_flow_2', 'water_flow_3',
  'log_side', 'log_top', 'leaves',
  'birch_log_side', 'birch_log_top', 'spruce_log_side', 'spruce_log_top',
  'ice', 'snow', 'cactus_side', 'cactus_top', 'pumpkin_side', 'pumpkin_top',
  'tall_grass', 'tall_grass_1', 'tall_grass_2', 'fern', 'fern_1', 'dead_bush',
  'dandelion', 'dandelion_1', 'rose', 'rose_1', 'sugar_cane',
  'brown_mushroom', 'red_mushroom',
  'daisy', 'bluebell', 'tall_grass_double_lower', 'tall_grass_double_upper', 'berry_bush',
  'dry_grass', 'dry_grass_tall_lower', 'dry_grass_tall_upper', 'leaf_litter',
  'fern_double_lower', 'fern_double_upper', 'cattail_lower', 'cattail_upper', 'lily_pad', 'marsh_orchid', 'bush', 'jungle_bush', 'large_fern', 'broadleaf',
  // the leaves tile with its holes filled, for fast leaves
  'leaves_fast'];

/** Tint palette characters (greyscale-and-tint pixels) per climate-tinted tile. */
export const TINTED = { grass_top: '89', grass_top_1: '89', grass_top_2: '89', grass_side: '89',
  leaves: '89', tall_grass: '89', tall_grass_1: '89', tall_grass_2: '89', fern: '89', fern_1: '89',
  tall_grass_double_lower: '89', tall_grass_double_upper: '89', berry_bush: '89',
  dry_grass: '89', dry_grass_tall_lower: '89', dry_grass_tall_upper: '89', leaf_litter: '89',
  fern_double_lower: '89', fern_double_upper: '89', cattail_lower: '89', cattail_upper: '89', lily_pad: '89', bush: '89', jungle_bush: '89', large_fern: '89', broadleaf: '89',
  water_0: 'abe', water_1: 'abe', water_2: 'abe', water_3: 'abe',
  water_flow_0: 'abe', water_flow_1: 'abe', water_flow_2: 'abe', water_flow_3: 'abe' };

/** Base tile name -> { count, weights }: weights sum to 1, base first; variant i is `${base}_${i}`. */
export const VARIANTS = {
  grass_top: { count: 3, weights: [0.6, 0.2, 0.2] },
  tall_grass: { count: 3, weights: [0.6, 0.2, 0.2] },
  fern: { count: 2, weights: [0.7, 0.3] },
  dandelion: { count: 2, weights: [0.7, 0.3] },
  rose: { count: 2, weights: [0.7, 0.3] },
};
