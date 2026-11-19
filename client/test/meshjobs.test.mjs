import test from 'node:test';
import assert from 'node:assert/strict';
import { createMeshJobs } from '../meshjobs.js';
import { createMeshQueue } from '../meshqueue.js';

// A mesh client whose replies come when the test says: `sent` lists the jobs in the
// order they went out, `reply(i)` resolves the i-th.
function heldClient() {
  const sent = [], resolvers = [];
  return {
    sent,
    submit(rx, rz, blocks) {
      sent.push({ rx, rz, blocks });
      return new Promise((resolve) => resolvers.push(() => resolve({ rx, rz })));
    },
    async reply(i) { resolvers[i](); await new Promise((r) => setImmediate(r)); },
  };
}
const make = (limit) => {
  const client = heldClient();
  const jobs = createMeshJobs({ queue: createMeshQueue({ limit }), client, gather: (rx, rz) => `blocks ${rx},${rz}` });
  return { client, jobs };
};
const centre = { rx: 0, rz: 0 };

test("an edited region's remesh goes out and finishes before queued new regions", async () => {
  const { client, jobs } = make(1);
  jobs.submit({ rx: 1, rz: 0, kind: 'new' });
  jobs.submit({ rx: 0, rz: 1, kind: 'new' });
  jobs.submit({ rx: 5, rz: 5, kind: 'edit' });
  jobs.pump(centre);
  assert.deepEqual(client.sent.map((s) => [s.rx, s.rz]), [[5, 5]]);
  assert.equal(client.sent[0].blocks, 'blocks 5,5', 'gathered when dispatched');
  await client.reply(0);
  assert.deepEqual(jobs.take().map((f) => [f.rx, f.rz, f.kind]), [[5, 5, 'edit']]);
  jobs.pump(centre);
  assert.equal(client.sent.length, 2);
  assert.equal(jobs.busy, true);
});

test('a region freed while its job is in flight is never taken', async () => {
  const { client, jobs } = make(2);
  jobs.submit({ rx: 1, rz: 0, kind: 'new' });
  jobs.submit({ rx: 0, rz: 1, kind: 'new' });
  jobs.pump(centre);
  jobs.cancel(1, 0);
  await client.reply(0);
  await client.reply(1);
  assert.deepEqual(jobs.take().map((f) => [f.rx, f.rz]), [[0, 1]]);
  assert.equal(jobs.busy, false);
});

test('a finished mesh freed before it is taken is dropped, and busy covers every stage', async () => {
  const { client, jobs } = make(1);
  assert.equal(jobs.busy, false);
  jobs.submit({ rx: 0, rz: 0, kind: 'new' });
  assert.equal(jobs.busy, true, 'queued');
  jobs.pump(centre);
  assert.equal(jobs.busy, true, 'in flight');
  await client.reply(0);
  assert.equal(jobs.busy, true, 'finished, not taken');
  jobs.cancel(0, 0);
  assert.deepEqual(jobs.take(), []);
  assert.equal(jobs.busy, false);
});
