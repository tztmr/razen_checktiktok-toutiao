export type DetectionStep =
  | "localPreparation"
  | "token"
  | "password"
  | "certification"
  | "identity"
  | "multiAccount";

export type DetectionStepTimings = Partial<Record<DetectionStep, number>>;

export type TimedOutcome<T> =
  | { status: "fulfilled"; value: T; durationMs: number }
  | { status: "rejected"; reason: unknown; durationMs: number };

const STEP_LABELS: ReadonlyArray<readonly [DetectionStep, string]> = [
  ["localPreparation", "本地准备"],
  ["token", "Token"],
  ["password", "密码"],
  ["certification", "实名"],
  ["identity", "身份补全"],
  ["multiAccount", "多账号"],
];

export function resolveBalancedWorkerCount(rowCount: number, cpuCores?: number) {
  const normalizedRows = Math.max(0, Math.floor(rowCount));
  if (normalizedRows === 0) return 0;
  if (normalizedRows < 6) return normalizedRows;
  const normalizedCores = Number.isFinite(cpuCores)
    ? Math.floor(cpuCores as number)
    : 6;
  return Math.min(normalizedRows, Math.min(8, Math.max(6, normalizedCores)));
}

export async function mapWithConcurrency<T, R>(
  items: readonly T[],
  limit: number,
  worker: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  if (items.length === 0) return [];
  const workerCount = Math.min(items.length, Math.max(1, Math.floor(limit)));
  const results = new Array<R>(items.length);
  let nextIndex = 0;

  async function runWorker() {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) return;
      results[index] = await worker(items[index], index);
    }
  }

  await Promise.all(Array.from({ length: workerCount }, () => runWorker()));
  return results;
}

export async function measureAsyncStep<T>(task: () => Promise<T>): Promise<TimedOutcome<T>> {
  const startedAt = performance.now();
  try {
    const value = await task();
    return { status: "fulfilled", value, durationMs: Math.round(performance.now() - startedAt) };
  } catch (reason) {
    return { status: "rejected", reason, durationMs: Math.round(performance.now() - startedAt) };
  }
}

export function formatStepTimings(timings: DetectionStepTimings) {
  const values = STEP_LABELS.flatMap(([step, label]) => {
    const durationMs = timings[step];
    return durationMs == null ? [] : [`${label}=${Math.round(durationMs)}ms`];
  });
  return values.length ? values.join("；") : "-";
}
