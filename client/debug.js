// Debug mode gate (docs/design/debug-console.md section 6). Debug mode is on
// when the query string has debug=1, or a test scene is selected with ?scene=.
export function debugEnabled(search) {
  const qs = new URLSearchParams(search);
  return qs.get('debug') === '1' || !!qs.get('scene');
}
