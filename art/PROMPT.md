You are drawing pixel art textures for a small voxel game called
Mince Raft, inspired by early Minecraft Beta. The player lives on a
wooden raft at sea, farms crops in planters, catches fish, and prepares
sashimi and sushi at a butcher block for villagers. Fish, squid and
villagers are 3D box models like Minecraft mobs; fish swim in the sea
the way they do in Minecraft 1.13.

FORMAT
- Unless a batch says otherwise, each texture is exactly 8 rows of
  exactly 8 characters.
- Textures with a stated size W×H are exactly H rows of exactly W
  characters.
- Each character is one palette index from the table below.
- "." means transparent (items, crops, leaves, particles, sky, UI, and
  model faces that say so).
- Output a single JSON object: {"texture_name": ["row1", ..., "rowN"]}.
- For models (batches 11 and 12), nest one level:
  {"model_name": {"face_name": ["row1", ...]}}.
- No commentary, no markdown, only the JSON.

PALETTE (use only these)
0 #1a1a1a  near black (outlines, deep shadow)
1 #4a4a4a  dark gray
2 #7f7f7f  stone gray
3 #b0b0b0  light gray (metal, highlights)
4 #5c3b1e  dark brown
5 #8b5a2b  brown (dirt, bark)
6 #b8864b  tan (planks, cut wood)
7 #e3d39b  sand
8 #2f5e1e  dark green
9 #5c9e31  grass green
a #2b4fa8  deep water
b #4a7fd6  water
c #c0392b  red (tuna, meat)
d #f08a5d  salmon orange
e #f4f1e6  off-white (rice, fish fat, plate)
f #e8c33a  yellow (wheat, highlights)

Skin tones have no dedicated entries: use 6 (tan) with 7 highlights
and 5 shadow for lighter skin, or 5 with 6 highlights and 4 shadow
for darker skin.

STYLE RULES
- No anti-aliasing, no gradients. Hard pixel edges only.
- Light comes from the top left. Highlights go top left, shadows bottom right.
- Terrain block faces must tile seamlessly on all four edges.
- Use 3 to 5 palette colors per block texture, 2 to 6 per item.
- Items and crops have a transparent background and a dark outline
  (0 or 4) on their silhouette, not around the whole tile.
- Avoid perfect symmetry and regular checkerboards on natural materials.
  Scatter noise pixels irregularly.
- Keep it readable at 8x8. One clear silhouette per item.

MODEL FACE CONVENTIONS (batches 11 and 12)
- A box is W wide (left-right), H tall, D deep (front-back). "Front" is
  the side the creature faces: the face for a person, the nose for a fish.
- front and back faces are W×H. left and right faces are D×H.
  top and bottom faces are W×D.
- Every face is drawn as seen from outside the box, top row up.
  On a left face the creature's front is at the left edge of the grid;
  on a right face it is at the right edge. On a top face the front edge
  is the bottom row; on a bottom face the front edge is the top row.
- Faces must line up where they meet: the edge pixels of neighbouring
  faces should continue each other's pattern.
- Model faces are opaque unless the batch says "." is allowed.
- Model shading is light: top faces lightest, bottom faces darkest,
  but keep it subtle because the engine also shades faces.

BATCH 1: TERRAIN BLOCKS
grass_top, grass_side, dirt, sand, gravel, stone, water_0, water_1,
water_2, water_3 (four animation frames, each tileable, with small
shifting highlights), log_side, log_top, planks, leaves

BATCH 2: RAFT AND WORKSTATIONS
raft_planks (rougher than planks, with lashing rope), raft_edge,
butcher_block_top (end-grain checker of tan and brown with a knife
mark), butcher_block_side, planter_side, farmland_dry, farmland_wet

BATCH 3: CROPS (transparent background, growing from the bottom row)
rice_0, rice_1, rice_2, rice_3, wheat_0, wheat_1, wheat_2, wheat_3,
seaweed_0, seaweed_1, seaweed_2, cucumber_0, cucumber_1, cucumber_2

BATCH 4: RAW INGREDIENTS (items)
raw_salmon, raw_tuna, raw_mackerel, raw_squid, rice_grains, nori,
cucumber, wheat, seeds, seaweed

BATCH 5: DISHES (items, on a small white plate where it fits)
salmon_sashimi, tuna_sashimi, mackerel_sashimi, salmon_nigiri,
tuna_nigiri, cucumber_maki, salmon_maki, rice_bowl, bread

BATCH 6: TOOLS AND UI
knife, fishing_rod, hoe, bucket, bucket_water, heart_full, heart_half,
hunger_full, hunger_half, sun, moon, cloud

BATCH 7: REMAINING BLOCK FACES (8x8, opaque, tileable)
raft_bottom (underside of the raft: wet, darker logs with rope, a
little green algae), planter_top (soil framed by a 1-pixel wooden rim
that matches planter_side), planter_bottom (plain dark planks),
butcher_block_bottom (darker side grain, no knife marks),
water_flow_0, water_flow_1, water_flow_2, water_flow_3 (flowing water
for the sides of falling water: vertical streaks of water and deep
water with a few off-white flecks; each frame moves the pattern down by
2 pixels so the four frames loop, and each frame tiles on all edges)

BATCH 8: PARTICLES (8x8, transparent background, no outline required,
small and centred; most should use only 1 to 3 colors)
bubble (a hollow ring, about 4x4), bubble_pop (the same ring broken
into 4 scattered pixels), splash_0, splash_1, splash_2, splash_3
(water droplets thrown up then falling: 0 is a tight cluster low in the
tile, 3 is a few drops spread wide), bobber (a red and off-white fishing
float, about 3x4, with a 1-pixel dark tip; the line is drawn by the
engine), fishing_wake (one 2x2 off-white fleck with a 1-pixel water
shadow, for the trail that moves toward the bobber before a bite),
puff_0, puff_1, puff_2, puff_3, puff_4, puff_5, puff_6, puff_7
(a gray and off-white smoke puff that starts as a large rough 7x7 blob
in puff_0 and shrinks to a single pixel in puff_7), happy_villager (a
small green four-point sparkle), angry_villager (a small dark gray
storm cloud with one red mark)

BATCH 9: SKY (transparent background)
sun_sky (16×16: the sun drawn in the sky, a square sun in yellow with an off-white core and a ragged
1-pixel yellow halo; not round), moon_0, moon_1, moon_2, moon_3, moon_4,
moon_5, moon_6, moon_7 (each 16×16: a square moon in off-white and light
gray with two or three gray craters at the same positions in every
frame; moon_0 is full, moon_4 is new and almost invisible, 1 to 3 wane
from the right, 5 to 7 wax from the left; the dark part is transparent)

BATCH 10: INVENTORY (transparent background where noted)
slot (10×10: a 1-pixel frame around an 8x8 opening for an item; frame is
dark on the top and left edges and light on the bottom and right, so it
looks recessed; the opening is dark brown), slot_selected (10×10: the
same with a bright yellow frame), panel_tl, panel_t, panel_tr, panel_l,
panel_c, panel_r, panel_bl, panel_b, panel_br (each 4×4: a nine-slice
wooden panel like a raft board with a dark outline and a 1-pixel bevel;
panel_c tiles in both directions, panel_t and panel_b tile
horizontally, panel_l and panel_r tile vertically, corners are
transparent outside the outline), digit_0, digit_1, digit_2, digit_3,
digit_4, digit_5, digit_6, digit_7, digit_8, digit_9 (each 4×6:
stack-count numerals in off-white, 3 pixels wide and 5 tall, with a
1-pixel near-black drop shadow to the right and below; transparent
elsewhere)

BATCH 11: SEA CREATURES (models; see MODEL FACE CONVENTIONS)
Draw each creature as an object of named faces. Only the left side of
each body is requested; the engine mirrors it for the right side.
Fins and tails are flat planes: draw them as seen from the creature's
left, with "." allowed around the fin shape.

salmon (body box 2 wide, 4 tall, 8 long; salmon-orange flanks with a
pink-red stripe, darker back, silvery belly):
  body_left 8×4, body_top 2×8, body_bottom 2×8, body_front 2×4 (face with
  an eye on each edge column), body_back 2×4, tail 4×4 (body attaches
  at the left column), dorsal 3×2 (body attaches at the bottom row)

tuna (body box 3 wide, 5 tall, 10 long; deep-blue back, light gray
flanks, off-white belly, a yellow finlet line near the tail):
  body_left 10×5, body_top 3×10, body_bottom 3×10, body_front 3×5,
  body_back 3×5, tail 5×6 (a deep crescent; body attaches at the left
  column), dorsal 4×3 (body attaches at the bottom row)

mackerel (body box 2 wide, 3 tall, 7 long; water-blue back with dark
wavy stripes, light gray belly):
  body_left 7×3, body_top 2×7, body_bottom 2×7, body_front 2×3,
  body_back 2×3, tail 3×3 (body attaches at the left column),
  dorsal 2×2 (body attaches at the bottom row)

squid (Minecraft Beta layout: body box 12 wide, 16 tall, 12 deep, with
eight tentacles hanging from the bottom; deep-water and dark-gray body
with near-black mottling; eyes are near-black with an off-white pixel,
on the front only):
  body_front 12×16, body_back 12×16, body_left 12×16, body_right 12×16,
  body_top 12×12, body_bottom 12×12, tentacle_side 2×18 (used for all
  four sides of every tentacle), tentacle_end 2×2

BATCH 12: PEOPLE (models; see MODEL FACE CONVENTIONS; all opaque)

player (Minecraft Beta layout. A raft dweller: messy dark hair, tan
skin, sleeveless faded water-blue shirt, rolled sand-colored trousers,
bare feet. One arm and one leg texture are drawn and mirrored for the
other side, so arms and legs use outer and inner instead of left and
right):
  head (8 wide, 8 tall, 8 deep): head_front, head_back, head_left,
  head_right, head_top, head_bottom, each 8×8
  body (8 wide, 12 tall, 4 deep): body_front 8×12, body_back 8×12,
  body_left 4×12, body_right 4×12, body_top 8×4, body_bottom 8×4
  arm (4 wide, 12 tall, 4 deep): arm_front, arm_back, arm_outer,
  arm_inner, each 4×12; arm_top 4×4, arm_bottom 4×4 (the hand)
  leg (4 wide, 12 tall, 4 deep): leg_front, leg_back, leg_outer,
  leg_inner, each 4×12; leg_top 4×4, leg_bottom 4×4 (the sole)

villager (Minecraft 1.14 villager shape: tall head, long nose, robe,
arms folded across the chest. A sushi customer: darker skin, short
gray hair, a long off-white robe with a deep-water sash, brown sandals):
  head (8 wide, 10 tall, 8 deep): head_front 8×10 (eyes and a unibrow,
  no mouth; the nose is a separate box), head_back 8×10, head_left 8×10,
  head_right 8×10, head_top 8×8, head_bottom 8×8
  nose (2 wide, 4 tall, 2 deep): nose_front 2×4, nose_side 2×4 (used for
  both sides), nose_bottom 2×2
  body (8 wide, 12 tall, 6 deep): body_front 8×12, body_back 8×12,
  body_left 6×12, body_right 6×12, body_top 8×6, body_bottom 8×6
  robe (drawn just outside the body and reaching down over the legs;
  8 wide, 18 tall, 6 deep): robe_front 8×18, robe_back 8×18,
  robe_left 6×18, robe_right 6×18, robe_bottom 8×6
  folded arms, made of two upper-arm boxes (4 wide, 8 tall, 4 deep,
  sleeves) and one crossbar (8 wide, 4 tall, 4 deep, forearms and hands
  meeting in the middle): upper_front, upper_back, upper_outer,
  upper_inner, each 4×8; upper_top 4×4, upper_bottom 4×4;
  cross_front 8×4, cross_back 8×4, cross_top 8×4, cross_bottom 8×4,
  cross_end 4×4 (used for both ends)
  leg (4 wide, 12 tall, 4 deep; mostly hidden by the robe): leg_front,
  leg_back, leg_outer, leg_inner, each 4×12; leg_top 4×4,
  leg_bottom 4×4

BATCH 13: BIOME BLOCK FACES (8x8, opaque, tileable)
birch_log_side (off-white bark with short near-black horizontal
marks), birch_log_top (rings like log_top in sand and tan, with an
off-white bark rim), spruce_log_side (dark brown bark with vertical
near-black furrows), spruce_log_top (rings like log_top, darker, with a
dark rim), ice (water blue with light diagonal streaks), snow
(off-white with a few light gray specks), cactus_side (green with dark
green vertical ribs and off-white spine dots on the ribs), cactus_top
(green with a dark green rim and a small dark centre), pumpkin_side
(salmon orange with red vertical ribs and yellow highlights),
pumpkin_top (orange with red ribs and a brown stem in the middle)

BATCH 14: WILD PLANTS (transparent background, growing from the bottom
row, drawn on two crossed quads; tall_grass, fern and sugar_cane use
only the greens 8 and 9 so the engine can tint them)
tall_grass, fern, dead_bush (bare brown twigs), dandelion (yellow head
on a green stem), rose (red bloom on a green stem), sugar_cane (two
green stalks with dark nodes; tiles vertically), brown_mushroom,
red_mushroom (red cap with off-white spots)

BATCH 16: ITEM ICONS (8x8, transparent background, 2 to 5 colors)
sword (a straight blade running from the top right to the bottom
left in light gray with a near-black outline, a brown cross-guard and
a brown handle), leather (a flat hide patch in browns with a
near-black outline), feather (a slanted off-white feather with a gray
quill running to the bottom left)

BATCH 15: ANIMALS (models; see MODEL FACE CONVENTIONS; all opaque)
Each animal is one box at 10 pixels per block, matching its `box` in
data/animals.toml (the footprint is square, so width = depth). Faces are
front, back, left, right, top, bottom. Left and right are mirror images
(front at the left edge on left, the right edge on right). Palette only.

cow (0.9 x 1.4 blocks, box 9 wide, 14 tall, 9 deep; brown with off-white
patches, dark brown hooves, pink muzzle on the front):
  front 9×14, back 9×14, left 9×14, right 9×14, top 9×9, bottom 9×9

pig (0.9 x 0.9 blocks, box 9 wide, 9 tall, 9 deep; pink with red-pink
ears and snout, near-black eyes):
  front 9×9, back 9×9, left 9×9, right 9×9, top 9×9, bottom 9×9

chicken (0.4 x 0.7 blocks, box 4 wide, 7 tall, 4 deep; off-white with a
red comb and yellow beak and red wattle on the front face):
  front 4×7, back 4×7, left 4×7, right 4×7, top 4×4, bottom 4×4
