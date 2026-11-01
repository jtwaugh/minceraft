"""Batches 7-12 of the Mince Raft textures.

Most grids are hand-drawn below. The sun, moon phases, digits and squid
mottling are built by small helpers because they are regular or noisy.
Run from the art/ directory: writes batches_7_12.json and checks sizes.
"""
import json
import random

PAL = set("0123456789abcdef.")


def shift_down(rows, n):
    return rows[-n:] + rows[:-n]


# ---------------------------------------------------------------- batch 7
water_flow = [
    "babbabbe",
    "babbabba",
    "bbbbabba",
    "babeabba",
    "babbabba",
    "babbbbba",
    "eabbabba",
    "babbabaa",
]
batch7 = {
    "raft_bottom": ["45445454", "54448844", "66646664", "44448454",
                    "54454454", "48454544", "54454458", "44584454"],
    "planter_top": ["44444444", "46666664", "46440454", "46454054",
                    "46044554", "46440454", "46555554", "44444444"],
    "planter_farmland_top": ["44444444", "46566564", "40000004", "45555554",
                             "46665664", "40000004", "45555554", "44444444"],
    "planter_bottom": ["55545555", "55555545", "44444444", "54555555",
                       "55550455", "44444444", "55545555", "45555554"],
    "butcher_block_bottom": ["55445544", "55445504", "54445544", "55405544",
                             "55445544", "50445544", "55445454", "55445544"],
    "griddle_top": ["11111111", "12222231", "12322221", "12222321",
                    "12232221", "12222231", "12322221", "11111111"],
    "griddle_side": ["33333333", "22222223", "11111112", "10101010",
                     "11111111", "01010101", "11111111", "00000000"],
    "griddle_bottom": ["01111110", "11011011", "11111111", "10111101",
                       "11111111", "11011011", "11111111", "01111110"],
    "crate_top": ["44444444", "46666664", "46555564", "46555564",
                  "46555564", "46555564", "46666664", "44444444"],
    "crate_side": ["44444444", "65656565", "46666664", "55555555",
                   "55555555", "46666664", "56565656", "44444444"],
    "crate_bottom": ["44444444", "45555554", "45444454", "45455454",
                     "45455454", "45444454", "45555554", "44444444"],
    "counter_top": ["66666766", "65666666", "66667666", "66666656",
                    "76666666", "66656666", "66666676", "56666666"],
    "counter_side": ["77777777", "66666666", "44444444", "55545554",
                     "55545554", "55545054", "55545554", "44444444"],
    "plate_rack": ["44444444", "4eeeeee4", "43333334", "4eeeeee4",
                   "43333334", "4eeeeee4", "43333334", "44444444"],
    "chest_top": ["44444444", "46666664", "45656564", "46565654",
                  "45656564", "46565654", "46666664", "44444444"],
    "chest_side": ["44444444", "66666666", "56565656", "65656565",
                   "56565656", "65656565", "55555555", "44444444"],
    "chest_front": ["44444444", "66666666", "44444444", "56535655",
                    "56333655", "65656565", "55555555", "44444444"],
    # Sea-ice floes: ice plates split by deep-water cracks (a, 0), with a
    # light gray pressure ridge (3, e) and off-white broken edges.
    "floe_ice_0": ["bbebba0b", "bbbba0bb", "aa0abbe3", "bbbbb3eb",
                   "be3eabbb", "b3ebbabb", "bbbbba0a", "ebbbbabb"],
    "floe_ice_1": ["bbbabbbb", "ebba0bbe", "3eb0abb3", "bb3eba3e",
                   "bbbbab3b", "a0aabbbb", "bbbbbbeb", "bebbbbbb"],
    "floe_ice_2": ["b0bbbbeb", "ba0bbe3b", "bbaa3ebb", "ebbbab0a",
                   "3ebbbabb", "b3bbbb0b", "bbbeba0b", "bbebba0b"],
}
for i in range(4):
    batch7[f"water_flow_{i}"] = shift_down(water_flow, 2 * i)

# ---------------------------------------------------------------- batch 8
E = "........"
batch8 = {
    "bubble": [E, E, "...eb...", "..e..b..", "..b..b..", "...bb...", E, E],
    "bubble_pop": [E, "..e.....", "......b.", E, E, ".b......", ".....b..", E],
    "splash_0": [E, E, E, E, E, "...b....", "..beb...", "...bb..."],
    "splash_1": [E, E, E, "...e....", ".b...b..", "...b....", "..b.eb..", E],
    "splash_2": [E, "....e...", ".b....b.", "...b....", "b......e", "....b...", E, E],
    "splash_3": [E, E, E, E, ".......b", "b.......", "...e....", "b.....b."],
    "bobber": [E, "...0....", "..ccc...", "..ecc...", "..eee...", "..ee3...", E, E],
    "fishing_wake": [E, E, E, "...ee...", "...eeb..", "....bb..", E, E],
    "puff_0": [".33ee3..", "3eee3e3.", "3ee3ee2.", "e3eee32.",
               "3eee3e2.", "23e3322.", ".2222...", E],
    "puff_1": [E, "..3ee3..", ".3eee32.", ".ee3e32.", ".3eee22.", "..2322..", E, E],
    "puff_2": [E, E, "..3e3...", ".3ee32..", ".e3e32..", ".3e322..", "..222...", E],
    "puff_3": [E, E, "...33...", "..3ee2..", "..3e32..", "...22...", E, E],
    "puff_4": [E, E, E, "...e3...", "...3e2..", "....2...", E, E],
    "puff_5": [E, E, E, "...e3...", "...32...", E, E, E],
    "puff_6": [E, E, E, E, "...e3...", E, E, E],
    "puff_7": [E, E, E, E, "...3....", E, E, E],
    "happy_villager": [E, "...9....", "...9....", ".99e99..", "...9....", "...8....", E, E],
    "angry_villager": [E, "..11....", ".11211..", "1111110.", ".000000.",
                       "....c...", "...c....", "....c..."],
}

# ---------------------------------------------------------------- batch 9
def blank(w, h):
    return [["."] * w for _ in range(h)]


def rows(g):
    return ["".join(r) for r in g]


def sun():
    g = blank(16, 16)
    for y in range(2, 14):
        for x in range(2, 14):
            g[y][x] = "f"
    for y in range(4, 12):
        for x in range(4, 12):
            g[y][x] = "7"
    for y in range(5, 11):
        for x in range(5, 11):
            g[y][x] = "e"
    for i in range(3, 14):
        g[13][i] = "d"
        g[i][13] = "d"
    # ragged halo: hand-picked gaps so it is not regular
    top = "..f.ff.ff.f.ff.."
    bot = "...ff.f.ff.ff.f."
    left = "..f.ff.f.fff.f.."
    right = "...f.ff.ff.f.ff."
    for x in range(16):
        if top[x] == "f": g[1][x] = "f"
        if bot[x] == "f": g[14][x] = "f"
    for y in range(16):
        if left[y] == "f": g[y][1] = "f"
        if right[y] == "f": g[y][14] = "f"
    return rows(g)


CRATERS = [(4, 5), (5, 5), (4, 6), (9, 9), (10, 9), (9, 10), (11, 4), (7, 11)]


def moon(phase):
    g = blank(16, 16)
    for y in range(2, 14):
        for x in range(2, 14):
            g[y][x] = "e"
    for i in range(3, 14):
        g[13][i] = "3"
        g[i][13] = "3"
    for (x, y) in CRATERS:
        g[y][x] = "2"
    if phase == 4:
        for y in range(2, 14):
            for x in range(2, 14):
                g[y][x] = "0" if (x, y) in CRATERS else "1"
        return rows(g)
    # lit columns: waning (1-3) keeps the left part, waxing (5-7) the right
    keep = {0: 12, 1: 9, 2: 6, 3: 3, 5: 3, 6: 6, 7: 9}[phase]
    for y in range(2, 14):
        curve = 1 if y in (2, 3, 12, 13) else 0
        for x in range(2, 14):
            col = x - 2
            if phase in (1, 2, 3):
                lit = col < keep - curve
            elif phase in (5, 6, 7):
                lit = col >= 12 - keep + curve
            else:
                lit = True
            if not lit:
                g[y][x] = "."
    return rows(g)


batch9 = {"sun_sky": sun()}
for p in range(8):
    batch9[f"moon_{p}"] = moon(p)

# ---------------------------------------------------------------- batch 10
GLYPHS = {
    0: ["111", "101", "101", "101", "111"], 1: ["010", "110", "010", "010", "111"],
    2: ["111", "001", "111", "100", "111"], 3: ["111", "001", "011", "001", "111"],
    4: ["101", "101", "111", "001", "001"], 5: ["111", "100", "111", "001", "111"],
    6: ["111", "100", "111", "101", "111"], 7: ["111", "001", "001", "010", "010"],
    8: ["111", "101", "111", "101", "111"], 9: ["111", "101", "111", "001", "111"],
}


def digit(n):
    g = blank(4, 6)
    on = {(x, y) for y, r in enumerate(GLYPHS[n]) for x, c in enumerate(r) if c == "1"}
    for (x, y) in on:
        if (x + 1, y + 1) not in on:
            g[y + 1][x + 1] = "0"
    for (x, y) in on:
        g[y][x] = "e"
    return rows(g)


batch10 = {
    "slot": ["0000000005"] + ["0444444446"] * 8 + ["5666666666"],
    "slot_selected": ["ffffffffff"] + ["f44444444f"] * 8 + ["ffffffffff"],
    "heart_empty": [
        ".00.00..",
        "0440440.",
        "0444440.",
        "0444440.",
        ".04440..",
        "..040...",
        "...0....",
        "........",
    ],
    "panel_tl": [".000", "0777", "0766", "0766"],
    "panel_t": ["0000", "7777", "6666", "6656"],
    "panel_tr": ["000.", "7750", "6650", "6650"],
    "panel_l": ["0766", "0766", "0756", "0766"],
    "panel_c": ["6665", "6666", "6566", "6666"],
    "panel_r": ["6650", "5650", "6650", "6650"],
    "panel_bl": ["0766", "0766", "0755", ".000"],
    "panel_b": ["6666", "6566", "5555", "0000"],
    "panel_br": ["6650", "6650", "5550", "000."],
}
for n in range(10):
    batch10[f"digit_{n}"] = digit(n)

# ---------------------------------------------------------------- batch 11
def mottled(w, h, seed, base="a", spots=(("1", 0.22), ("0", 0.08)), top_hi=False):
    rnd = random.Random(seed)
    g = blank(w, h)
    for y in range(h):
        for x in range(w):
            c = base
            r = rnd.random()
            acc = 0
            for col, p in spots:
                acc += p
                if r < acc:
                    c = col
                    break
            if top_hi and y < 3 and rnd.random() < 0.25:
                c = "b"
            g[y][x] = c
    return g


def squid():
    front = mottled(12, 16, 11, top_hi=True)
    for (ex, ey) in ((2, 10), (8, 10)):
        front[ey][ex], front[ey][ex + 1] = "e", "0"
        front[ey + 1][ex], front[ey + 1][ex + 1] = "0", "0"
    t = mottled(2, 18, 99)
    for y in range(12, 18):  # tips darken
        for x in range(2):
            if t[y][x] == "a" and (x + y) % 3 == 0:
                t[y][x] = "1"
    return {
        "body_front": rows(front),
        "body_back": rows(mottled(12, 16, 12, top_hi=True)),
        "body_left": rows(mottled(12, 16, 13, top_hi=True)),
        "body_right": rows(mottled(12, 16, 14, spots=(("1", 0.3), ("0", 0.1)))),
        "body_top": rows(mottled(12, 12, 15, spots=(("b", 0.15), ("1", 0.15)))),
        "body_bottom": rows(mottled(12, 12, 16, base="1", spots=(("0", 0.25), ("a", 0.1)))),
        "tentacle_side": rows(t),
        "tentacle_end": ["10", "01"],
    }


batch11 = {
    "salmon": {
        "body_left": ["21121112", "d0dddddd", "ddcccdcc", "3e333332"],
        "body_top": ["11", "12", "11", "21", "11", "11", "12", "21"],
        "body_bottom": ["e3", "33", "3e", "33", "33", "e3", "33", "32"],
        "body_front": ["21", "00", "cd", "33"],
        "body_back": ["11", "dd", "cc", "33"],
        "tail": ["..1d", "dd1.", "cdc.", "..cc"],
        "dorsal": ["1..", "112"],
    },
    "tuna": {
        "body_left": ["baaaaaafaf", "a0abaaaaaa", "3333333332", "ee3333e332", "eeeeeeefef"],
        "body_top": ["faf", "aaa", "aba", "aaa", "baa", "aaa", "aab", "aaa", "aaa", "bab"],
        "body_bottom": ["eee", "e3e", "eee", "eee", "3ee", "eee", "ee3", "eee", "fef", "e3e"],
        "body_front": ["aba", "0a0", "333", "3e3", "eee"],
        "body_back": ["afa", "aaa", "333", "333", "efe"],
        "tail": ["...ab", "..aa.", "aaa..", "a11..", "..11.", "...11"],
        "dorsal": [".a..", "aa..", "aaaa"],
    },
    "mackerel": {
        "body_left": ["babaaba", "b0bbabb", "e333332"],
        "body_top": ["ab", "ba", "aa", "ab", "ba", "ab", "bb"],
        "body_bottom": ["33", "3e", "33", "33", "e3", "33", "33"],
        "body_front": ["bb", "00", "33"],
        "body_back": ["ab", "ba", "33"],
        "tail": ["..b", "ba.", "..a"],
        "dorsal": ["a.", "aa"],
    },
    "squid": squid(),
}

# ---------------------------------------------------------------- batch 12
def rev(rs):
    return [r[::-1] for r in rs]


head_left = ["00100010", "10000100", "77000100", "66600100",
             "66665000", "66655100", "66666000", "56665500"]
player = {
    "head_front": ["00100010", "01001000", "07607660", "06666665",
                   "6e0660e5", "66655665", "66644665", "56666655"],
    "head_back": ["00100010", "01000100", "00010001", "10000100",
                  "00100000", "01001010", "00000100", "06666650"],
    "head_left": head_left,
    "head_right": rev(head_left),
    "head_top": ["00100010", "01000100", "00011000", "10010001",
                 "00100100", "01000010", "00010100", "10100001"],
    "head_bottom": ["56666665", "55555555", "55455555", "55555545",
                    "55555555", "54555555", "00555500", "00000000"],
    "body_front": ["bbb66bbb", "3bbbbbba", "b3bbbbba", "bbbbbbba", "bbbabbba", "bbbbbbba",
                   "bbb3bbba", "bbbbbbaa", "bbbbbbba", "abbbbbba", "44454444", "77777776"],
    "body_back": ["bbbbbbba", "3bbbbbba", "bbbbbbba", "bbbbabba", "bbbbbbba", "bbabbbba",
                  "bbbbbbba", "bbbbbbaa", "bbbbbbba", "abbbbbba", "44444544", "77777776"],
    "body_left": ["bbba", "bbba", "bbba", "baba", "bbba", "bbba",
                  "bbaa", "bbba", "bbba", "bbba", "4444", "7776"],
    "body_right": ["abbb", "abbb", "aabb", "abbb", "abbb", "abab",
                   "abbb", "aabb", "abbb", "aabb", "4444", "6777"],
    "body_top": ["bbbbbbbb", "3bbbbbbb", "bbbbbbba", "bbbbbbba"],
    "body_bottom": ["77777777", "76777767", "77677777", "66666666"],
    "arm_front": ["7666", "6666", "6665", "6666", "6665", "7666",
                  "6666", "6665", "8998", "6666", "6665", "5655"],
    "arm_back": ["6665", "6655", "6665", "6655", "6665", "6665",
                 "6655", "6665", "8988", "6655", "6555", "5555"],
    "arm_outer": ["7766", "7666", "6666", "7666", "6666", "6665",
                  "7666", "6666", "9989", "7666", "6665", "5665"],
    "arm_inner": ["6655", "6665", "6655", "6655", "6665", "6655",
                  "6655", "6555", "8898", "6655", "6555", "5555"],
    "arm_top": ["7766", "7666", "6666", "6665"],
    "arm_bottom": ["6655", "6555", "5555", "5554"],
    "leg_front": ["7776", "7776", "7767", "7776", "7776", "6776",
                  "7777", "6666", "5555", "6666", "6665", "6565"],
    "leg_back": ["7766", "7776", "7776", "6776", "7776", "7776",
                 "7777", "6666", "5555", "6665", "6665", "5555"],
    "leg_outer": ["7777", "7777", "7767", "7777", "6777", "7777",
                  "7777", "6666", "5555", "6666", "6665", "6555"],
    "leg_inner": ["6776", "7776", "7776", "7676", "7776", "7766",
                  "7776", "6666", "5555", "6655", "6655", "5555"],
    "leg_top": ["7777", "7776", "7777", "6776"],
    "leg_bottom": ["5555", "5545", "5455", "4555"],
}

v_head_left = ["23222322", "22222222", "66552232", "55555222", "55555522",
               "55545552", "55555555", "55555554", "55555544", "45555444"]
robe_front = ["ee3553e3", "eee33ee3", "eeee3ee3", "eeee3ee3", "eeeee3e3", "eeeee3e3",
              "eeeeeee3", "abaaaaaa", "aaaaaaa1", "eeeee3e3", "eeeeeee3", "eeee3ee3",
              "eeeeeee3", "eeeee3e3", "eeeeeee3", "e3eeeee3", "eeeeee33", "33333332"]
robe_back = ["eeeeeee3", "eeeeeee3", "eeeeeee3", "3eeeeee3", "eeeeeee3", "eeeee3e3",
             "eeeeeee3", "aaaaaaaa", "aaaaaaa1", "eeeeeee3", "eeeeeee3", "eee3eee3",
             "eeeeeee3", "eeeeeee3", "e3eeeee3", "eeeeeee3", "eeeeee33", "33333332"]
robe_left = ["eeeee3", "eeeeee", "eeee3e", "eeeeee", "eeeeee", "e3eeee",
             "eeeeee", "aaaaaa", "aaaaa1", "eeeeee", "eee3ee", "eeeeee",
             "eeeeee", "eeeeee", "eeee3e", "eeeeee", "eeeee3", "333332"]
robe_right = ["3eeee3", "3eeeee", "3eeeee", "3eee3e", "3eeeee", "3eeeee",
              "3eeeee", "1aaaaa", "1aaaaa", "3eeeee", "3eeeee", "3e3eee",
              "3eeeee", "3eeeee", "3eeeee", "3eee3e", "3eeeee", "233333"]
robe_bottom = ["33333333", "32333333", "33333323", "33233333", "33333333", "23333332"]
villager = {
    "head_front": ["23222322", "22232222", "56665555", "65555554", "54444445",
                   "5e9559e5", "55555555", "55555554", "55455554", "45555544"],
    "head_back": ["22322232", "23222222", "22222322", "22232222", "52222225",
                  "55555555", "55555545", "54555555", "55555555", "45555544"],
    "head_left": v_head_left,
    "head_right": rev(v_head_left),
    "head_top": ["22322232", "23222223", "22232322", "32222232",
                 "22322222", "22223232", "23222222", "22232222"],
    "head_bottom": ["55555555", "45555554", "45555554", "44555544",
                    "44555544", "44455444", "44444444", "44444444"],
    "nose_front": ["56", "55", "55", "45"],
    "nose_side": ["65", "55", "55", "54"],
    "nose_bottom": ["44", "45"],
    "body_front": robe_front[:12],
    "body_back": robe_back[:12],
    "body_left": robe_left[:12],
    "body_right": robe_right[:12],
    "body_top": ["eeeeeeee", "eeeeeee3", "eee55ee3", "eee55ee3", "3eeeeee3", "eeeeee33"],
    "body_bottom": robe_bottom,
    "robe_front": robe_front,
    "robe_back": robe_back,
    "robe_left": robe_left,
    "robe_right": robe_right,
    "robe_bottom": robe_bottom,
    "upper_front": ["eeee", "eee3", "eeee", "eee3", "3ee3", "eeee", "eee3", "3333"],
    "upper_back": ["eee3", "eeee", "eee3", "eeee", "eee3", "eeee", "e3e3", "3333"],
    "upper_outer": ["eeee", "eeee", "e3ee", "eeee", "eeee", "eee3", "eeee", "3333"],
    "upper_inner": ["3ee3", "3eee", "3ee3", "3eee", "3eee", "3ee3", "3eee", "3333"],
    "upper_top": ["eeee", "eeee", "eee3", "ee33"],
    "upper_bottom": ["3333", "3233", "3323", "3333"],
    "cross_front": ["ee6655ee", "ee5555e3", "e35555ee", "33444433"],
    "cross_back": ["eeeeeee3", "eeeeeee3", "e3eeeee3", "33333333"],
    "cross_top": ["eeeeeeee", "eeeeeee3", "ee6556e3", "ee5555e3"],
    "cross_bottom": ["33444433", "33333333", "32333323", "33333333"],
    "cross_end": ["eeee", "e33e", "e33e", "3333"],
    "leg_front": ["5655", "5555", "5554", "5555", "5655", "5555",
                  "5554", "5555", "5555", "5455", "4544", "4444"],
    "leg_back": ["5555", "5554", "5555", "4555", "5555", "5554",
                 "5555", "5555", "5455", "5554", "4554", "4444"],
    "leg_outer": ["5655", "5555", "5555", "5565", "5555", "5555",
                  "5554", "5555", "5555", "5555", "4554", "4444"],
    "leg_inner": ["4555", "4555", "4554", "4555", "4555", "4555",
                  "4554", "4555", "4555", "4555", "4444", "4444"],
    "leg_top": ["5555", "5655", "5555", "5545"],
    "leg_bottom": ["4444", "4144", "4441", "4444"],
}
batch12 = {"player": player, "villager": villager}

# ---------------------------------------------------------------- batch 16
batch16 = {
    "sword": ["......00", ".....030", "....0330", "...0330.",
              "0.0330..", ".05560..", "0540....", "00......"],
    "leather": ["..0000..", ".066650.", "06655550", "05565550",
                "05555450", "05554440", ".055440.", "..0000.."],
    "feather": ["......00", ".....0e0", "....0ee0", "...0eee0",
                "..0eee0.", ".0e30...", "030.....", "0......."],
    "raw_beef": ["..0000..", ".0ccec0.", "0cccccc0", "0cecc5c0",
                 "0cccccc0", "0c5ccec0", ".0cccc0.", "..0000.."],
    "raw_pork": ["........", "00000000", "0eeeeee0", "0ddddde0",
                 "0dd5ddd0", "0dddd5d0", "0eeddde0", "00000000"],
    "raw_chicken": ["..000...", ".0ddd0..", "0dd7dd0.", "0ddddd0.",
                    ".0ddd0..", "..0e0...", "...0e00.", "....0ee0"],
}

# ---------------------------------------------------------------- batch 34
batch34 = {
    "thatch_top": ["f7ff6f7f", "7f6ff7f6", "ff7f6ff5", "6ff7ff6f",
                   "f6ff7f6f", "ff5f6ff7", "7ff6f7ff", "f6f7ff6f"],
    "thatch_side": ["7f7ff7f7", "f6ff6f6f", "6f6f6f66", "45545455",
                    "f7ff7f7f", "ff6ff6ff", "6f66f6f6", "54554545"],
    "drying_rack": ["66666665", "44444444", "4.8.98.4", "4.9.89.4",
                    "4.8.98.4", "4.9..8.4", "4.8....4", "4......4"],
    "net": ["6...5...", ".7.7.6.7", "..6...6.", ".7.6.7.7",
            "5...6...", ".7.7.7.6", "..6...5.", ".6.7.7.7"],
    "barrel_top": ["11111111", "17664661", "16654651", "16654651",
                   "16654561", "16654651", "15554551", "11111111"],
    "barrel_side": ["76546654", "13111131", "66546654", "76545654",
                    "66546654", "65546664", "11131111", "76546654"],
}

# ---------------------------------------------------------------- sizes
EXPECT = {
    # batch 9-12 explicit sizes (w, h); everything else defaults to 8x8
    "sun_sky": (16, 16), **{f"moon_{i}": (16, 16) for i in range(8)},
    "slot": (10, 10), "slot_selected": (10, 10), "heart_empty": (8, 8),
    **{f"panel_{s}": (4, 4) for s in ("tl", "t", "tr", "l", "c", "r", "bl", "b", "br")},
    **{f"digit_{i}": (4, 6) for i in range(10)},
}
MODEL_EXPECT = {
    "salmon": {"body_left": (8, 4), "body_top": (2, 8), "body_bottom": (2, 8), "body_front": (2, 4),
               "body_back": (2, 4), "tail": (4, 4), "dorsal": (3, 2)},
    "tuna": {"body_left": (10, 5), "body_top": (3, 10), "body_bottom": (3, 10), "body_front": (3, 5),
             "body_back": (3, 5), "tail": (5, 6), "dorsal": (4, 3)},
    "mackerel": {"body_left": (7, 3), "body_top": (2, 7), "body_bottom": (2, 7), "body_front": (2, 3),
                 "body_back": (2, 3), "tail": (3, 3), "dorsal": (2, 2)},
    "squid": {**{f"body_{s}": (12, 16) for s in ("front", "back", "left", "right")},
              "body_top": (12, 12), "body_bottom": (12, 12), "tentacle_side": (2, 18), "tentacle_end": (2, 2)},
    "player": {**{f"head_{s}": (8, 8) for s in ("front", "back", "left", "right", "top", "bottom")},
               "body_front": (8, 12), "body_back": (8, 12), "body_left": (4, 12), "body_right": (4, 12),
               "body_top": (8, 4), "body_bottom": (8, 4),
               **{f"{p}_{s}": (4, 12) for p in ("arm", "leg") for s in ("front", "back", "outer", "inner")},
               **{f"{p}_{s}": (4, 4) for p in ("arm", "leg") for s in ("top", "bottom")}},
    "villager": {**{f"head_{s}": (8, 10) for s in ("front", "back", "left", "right")},
                 "head_top": (8, 8), "head_bottom": (8, 8),
                 "nose_front": (2, 4), "nose_side": (2, 4), "nose_bottom": (2, 2),
                 "body_front": (8, 12), "body_back": (8, 12), "body_left": (6, 12), "body_right": (6, 12),
                 "body_top": (8, 6), "body_bottom": (8, 6),
                 "robe_front": (8, 18), "robe_back": (8, 18), "robe_left": (6, 18), "robe_right": (6, 18),
                 "robe_bottom": (8, 6),
                 **{f"upper_{s}": (4, 8) for s in ("front", "back", "outer", "inner")},
                 "upper_top": (4, 4), "upper_bottom": (4, 4),
                 **{f"cross_{s}": (8, 4) for s in ("front", "back", "top", "bottom")}, "cross_end": (4, 4),
                 **{f"leg_{s}": (4, 12) for s in ("front", "back", "outer", "inner")},
                 "leg_top": (4, 4), "leg_bottom": (4, 4)},
}


def check(name, rs, w, h):
    errs = []
    if len(rs) != h:
        errs.append(f"{name}: {len(rs)} rows, want {h}")
    for i, r in enumerate(rs):
        if len(r) != w:
            errs.append(f"{name} row {i}: len {len(r)}, want {w}")
        bad = set(r) - PAL
        if bad:
            errs.append(f"{name} row {i}: bad chars {bad}")
    return errs


errs = []
for b in (batch7, batch8, batch9, batch10, batch16, batch34):
    for k, v in b.items():
        errs += check(k, v, *EXPECT.get(k, (8, 8)))
for b in (batch11, batch12):
    for model, faces in b.items():
        want = MODEL_EXPECT[model]
        missing = set(want) - set(faces)
        extra = set(faces) - set(want)
        if missing or extra:
            errs.append(f"{model}: missing {missing} extra {extra}")
        for f, v in faces.items():
            if f in want:
                errs += check(f"{model}.{f}", v, *want[f])
if errs:
    raise SystemExit("\n".join(errs))

out = {
    "Batch 7": {"title": "Remaining block faces", "tiles": True, "textures": batch7},
    "Batch 8": {"title": "Particles", "tiles": False, "textures": batch8},
    "Batch 9": {"title": "Sky", "tiles": False, "background": "night", "textures": batch9,
                "sizes": {k: EXPECT[k] for k in batch9}},
    "Batch 10": {"title": "Inventory", "tiles": False, "textures": batch10,
                 "sizes": {k: EXPECT[k] for k in batch10}},
    "Batch 11": {"title": "Sea creatures", "tiles": False, "models": batch11,
                 "face_sizes": {k: MODEL_EXPECT[k] for k in batch11}},
    "Batch 12": {"title": "People", "tiles": False, "models": batch12,
                 "face_sizes": {k: MODEL_EXPECT[k] for k in batch12}},
    "Batch 16": {"title": "Item icons", "tiles": False, "textures": batch16},
    "Batch 34": {"title": "Village blocks", "tiles": True, "textures": batch34},
}
json.dump(out, open("batches_7_12.json", "w"), indent=1)
print("ok", {k: len(v.get("textures", v.get("models"))) for k, v in out.items()})
