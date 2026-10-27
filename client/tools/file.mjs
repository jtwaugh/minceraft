// client/tools/file.mjs: files a play-critic report as one issue, skipping duplicates (Node only).
// `gh(args)` runs the GitHub CLI and returns stdout; tests inject a fake.
import { execFile } from 'node:child_process';
import { writeFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';

export async function realGh(args) {
  const { stdout } = await promisify(execFile)('gh', args, { maxBuffer: 64 * 1024 * 1024 });
  return stdout;
}

// The repro scripts in a report body: the contents of its ```input fences (any fence length).
export function repros(body) {
  const out = [];
  for (const m of String(body).matchAll(/^(`{3,})input\n([\s\S]*?)\n\1\s*$/gm)) out.push(m[2].trim());
  return out;
}

/** The earlier issue with the same title or sharing a repro, or null. */
export function findDuplicate({ title, body }, issues) {
  const mine = repros(body);
  return (
    issues.find((i) => i.title === title || mine.some((r) => r && String(i.body ?? '').includes(r))) ?? null
  );
}

/**
 * Create one issue for the report unless an open or closed `idea` issue already holds it.
 * No `idea` label is added: the maintainer adds it, which starts the reconciler.
 * Returns { filed: true, url } or { filed: false, duplicateOf: { number, title } }.
 */
export async function fileReport({ title, body }, gh = realGh) {
  const listed = await gh([
    'issue', 'list', '--label', 'idea', '--state', 'all', '--limit', '1000',
    '--json', 'number,title,body',
  ]);
  const dup = findDuplicate({ title, body }, JSON.parse(listed || '[]'));
  if (dup) return { filed: false, duplicateOf: { number: dup.number, title: dup.title } };
  const dir = mkdtempSync(join(tmpdir(), 'critic-'));
  try {
    const file = join(dir, 'body.md');
    writeFileSync(file, body);
    const url = (await gh(['issue', 'create', '--title', title, '--body-file', file])).trim();
    return { filed: true, url };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
