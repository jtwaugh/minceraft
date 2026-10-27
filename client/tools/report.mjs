// client/tools/report.mjs: renders one run's verified findings as a grouped idea issue (Node only).
export function renderReport(findings, { runDate } = {}) {
  if (!Array.isArray(findings) || findings.length === 0) return null;
  const title = `Play critic findings, ${runDate}`;
  const lines = [`# Mince Raft: ${title}`, '', 'Status: idea', 'Scope: game', ''];
  findings.forEach((f, i) => {
    lines.push(
      `## ${i + 1}. ${f.goal}`,
      '',
      `- Kind: ${f.kind}`,
      `- Severity: ${f.severity}`,
      `- Goal: ${f.goal}`,
      '',
      `**Observation:** ${f.observation}`,
      '',
      `**Suggestion:** ${f.suggestion}`,
      '',
      '**Repro:**',
      '',
    );
    // A fence longer than any backtick run in the repro keeps it from closing early.
    const longest = Math.max(2, ...(f.repro.match(/`+/g) ?? []).map((s) => s.length));
    const fence = '`'.repeat(longest + 1);
    lines.push(fence + 'input', f.repro.replace(/\n+$/, ''), fence, '');
  });
  return { title, body: lines.join('\n') };
}
