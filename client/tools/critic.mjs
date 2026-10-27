// client/tools/critic.mjs: local play-critic runner (Node only). Run: node client/tools/critic.mjs
// Asks the model for findings, drops any that don't replay, prints the report. It files an issue
// only with --file, and never edits the repo (docs/design/play-critic-agent.md sections 4 and 5).
import { execFile } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { parseFindings } from './finding.mjs';
import { verifyAll } from './verify.mjs';
import { renderReport } from './report.mjs';
import { fileReport } from './file.mjs';

const root = new URL('../../', import.meta.url);
const OUTPUT_RULE =
  '\n\nWhen you are done, reply with the findings as one JSON array of objects with the fields ' +
  'kind, severity, goal, observation, suggestion and repro, and nothing else. Reply [] if none.';

export const criticPrompt = () => readFileSync(new URL('.github/prompts/critic.md', root), 'utf8') + OUTPUT_RULE;

/** The real model call: `claude -p` with the critic prompt. Returns its stdout. */
export async function claudeModel(prompt) {
  const { stdout } = await promisify(execFile)('claude', ['-p', prompt], {
    cwd: fileURLToPath(root),
    maxBuffer: 64 * 1024 * 1024,
  });
  return stdout;
}

// The model may wrap the array in prose or a code fence; try the text, then the outermost [ ... ].
function extractJson(text) {
  const s = String(text ?? '');
  const a = s.indexOf('[');
  const b = s.lastIndexOf(']');
  return parseFindings(s).error && a >= 0 && b > a ? s.slice(a, b + 1) : s;
}

/** Run one critic pass. `model(prompt)` returns text; inject a canned one in tests. */
export async function runCritic({ model = claudeModel, runDate = new Date().toISOString().slice(0, 10) } = {}) {
  const output = await model(criticPrompt());
  const { valid } = parseFindings(extractJson(output));
  const { kept } = verifyAll(valid);
  return renderReport(kept, { runDate });
}

export async function main(out = process.stdout, err = process.stderr, options = {}, argv = []) {
  try {
    const report = await runCritic(options);
    out.write(report ? `${report.title}\n\n${report.body}\n` : 'no findings\n');
    if (report && argv.includes('--file')) {
      const r = await fileReport(report, options.gh);
      out.write(r.filed ? `filed ${r.url}\n` : `duplicate of #${r.duplicateOf.number} (${r.duplicateOf.title}); not filed\n`);
    }
    return 0;
  } catch (e) {
    err.write(`critic: ${e.message}\n`);
    return 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.stdout, process.stderr, {}, process.argv.slice(2));
