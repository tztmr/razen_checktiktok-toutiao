import assert from "node:assert/strict";
import test from "node:test";
import {
  formatStepTimings,
  mapWithConcurrency,
  measureAsyncStep,
  resolveBalancedWorkerCount,
} from "../src/batch-performance.ts";

const delay = (durationMs: number) => new Promise<void>((resolve) => {
  setTimeout(resolve, durationMs);
});

test("selects zero, small, and balanced worker counts", () => {
  assert.equal(resolveBalancedWorkerCount(0, 8), 0);
  assert.equal(resolveBalancedWorkerCount(3, 12), 3);
  assert.equal(resolveBalancedWorkerCount(6, 4), 6);
  assert.equal(resolveBalancedWorkerCount(20, 7), 7);
  assert.equal(resolveBalancedWorkerCount(200, 12), 8);
  assert.equal(resolveBalancedWorkerCount(200, undefined), 6);
});

test("maps with a fixed concurrency limit while preserving input order", async () => {
  let active = 0;
  let peak = 0;
  const result = await mapWithConcurrency([40, 10, 30, 5], 2, async (durationMs, index) => {
    active += 1;
    peak = Math.max(peak, active);
    await delay(durationMs);
    active -= 1;
    return `${index}:${durationMs}`;
  });

  assert.equal(peak, 2);
  assert.deepEqual(result, ["0:40", "1:10", "2:30", "3:5"]);
});

test("starts independent timed steps together and settles near the longest delay", async () => {
  const startedAt = performance.now();
  const outcomes = await Promise.all([
    measureAsyncStep(async () => { await delay(40); return "short"; }),
    measureAsyncStep(async () => { await delay(70); return "long"; }),
  ]);
  const elapsedMs = performance.now() - startedAt;

  assert.equal(outcomes[0].status, "fulfilled");
  assert.equal(outcomes[1].status, "fulfilled");
  assert.ok(elapsedMs >= 60, `elapsed ${elapsedMs}ms was shorter than the longest step`);
  assert.ok(elapsedMs < 105, `elapsed ${elapsedMs}ms looked serial`);
});

test("keeps a rejected step separate from a successful step", async () => {
  const [success, failure] = await Promise.all([
    measureAsyncStep(async () => "ok"),
    measureAsyncStep(async () => { throw new Error("network down"); }),
  ]);

  assert.deepEqual(success.status === "fulfilled" ? success.value : null, "ok");
  assert.equal(failure.status, "rejected");
  assert.match(failure.status === "rejected" ? String(failure.reason) : "", /network down/);
});

test("formats step timings in stable order and skips missing steps", () => {
  assert.equal(formatStepTimings({}), "-");
  assert.equal(
    formatStepTimings({ certification: 1402, localPreparation: 83, token: 10012 }),
    "本地准备=83ms；Token=10012ms；实名=1402ms",
  );
});
