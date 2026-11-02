// One debug-overlay line for the wind at the player (docs/design/wind.md Phase B).
//   formatWind(dx, dz, strength) -> "wind NE 3.2 b/s (gust 0.45)"
//     dx, dz: wind vector in blocks per second, +x east, +z south
//     strength: 0..1; calm (zero vector) prints "wind calm" with no direction
//   formatWind(dx, dz, strength, roughness, multiplier) appends " R 0.62 x0.71"
//     (docs/design/biome-roughness-wind.md Phase A); omitted, the line is unchanged

const POINTS = ['N', 'NE', 'E', 'SE', 'S', 'SW', 'W', 'NW'];

// Compass point the wind blows toward.
function compass(dx, dz) {
  const turns = Math.atan2(dx, -dz) / (2 * Math.PI); // 0 north, clockwise
  return POINTS[Math.round(((turns % 1) + 1) * 8) % 8];
}

const fixed = (v, digits) => (Number.isFinite(v) ? v : 0).toFixed(digits);

export function formatWind(dx, dz, strength, roughness, multiplier) {
  const speed = Math.hypot(dx, dz);
  const gust = fixed(strength, 2);
  const ground = roughness === undefined && multiplier === undefined ? '' : ` R ${fixed(roughness, 2)} x${fixed(multiplier, 2)}`;
  if (!(speed > 0)) return `wind calm 0.0 b/s (gust ${gust})${ground}`;
  return `wind ${compass(dx, dz)} ${speed.toFixed(1)} b/s (gust ${gust})${ground}`;
}
