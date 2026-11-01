// client/tools/finding.mjs: the play critic's finding schema (Node only).
export const KINDS = ['control-flow', 'layout', 'perf', 'render'];
export const SEVERITIES = ['low', 'medium', 'high'];
const FIELDS = ['kind', 'severity', 'goal', 'observation', 'suggestion', 'repro'];

export function validateFinding(obj) {
  if (obj === null || typeof obj !== 'object' || Array.isArray(obj)) {
    return { ok: false, errors: ['finding is not an object'] };
  }
  const errors = [];
  for (const f of FIELDS) {
    if (!(f in obj)) errors.push(`missing field: ${f}`);
    else if (typeof obj[f] !== 'string') errors.push(`field ${f} is not a string`);
  }
  if (typeof obj.kind === 'string' && !KINDS.includes(obj.kind)) {
    errors.push(`unknown kind: ${obj.kind}`);
  }
  if (typeof obj.severity === 'string' && !SEVERITIES.includes(obj.severity)) {
    errors.push(`unknown severity: ${obj.severity}`);
  }
  if (typeof obj.repro === 'string' && obj.repro.trim() === '') errors.push('repro is empty');
  return { ok: errors.length === 0, errors };
}

// Reads a JSON array of findings; never throws.
export function parseFindings(text) {
  let data;
  try {
    data = JSON.parse(text);
  } catch (e) {
    return { valid: [], rejected: [], error: `malformed JSON: ${e.message}` };
  }
  if (!Array.isArray(data)) {
    return { valid: [], rejected: [], error: 'expected a JSON array' };
  }
  const valid = [];
  const rejected = [];
  for (const finding of data) {
    const r = validateFinding(finding);
    if (r.ok) valid.push(finding);
    else rejected.push({ finding, errors: r.errors });
  }
  return { valid, rejected, error: null };
}
