# Balanced Batch Detection Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 200 多个抖音/头条包的批量检测改成 6–8 路平衡调度、同包独立检查并行、非敏感 ZIP 路径索引复用，并展示可导出的分步耗时。

**Architecture:** 新建 `src/batch-performance.ts` 承担纯调度、受限并发、异步步骤计时和耗时格式化；`App.tsx` 只编排 Tauri 命令与合并既有业务状态。Rust 在现有 `CacheState` 中缓存按 ZIP 路径、纳秒级修改时间和 APP ID 区分的内部路径索引，备份目录绕过缓存。

**Tech Stack:** React 19、TypeScript 5.8、Node `node:test`、Tauri 2、Rust 2021、`zip` 2、`rusqlite` 0.37。

## Global Constraints

- 包级 worker 数必须为 0、实际包数或 6–8 路，绝不恢复到 20 路。
- 所有现有联网请求继续使用 Rust 端 15 秒超时，不缩短超时时间。
- 只有后端明确返回 Token 失效才产生掉线信号；超时、HTTP、解析和命令异常必须保留为失败信号。
- 抖音当前账号 Token、密码、实名并行；身份补全仍只在 Token 返回后按条件执行。
- 头条 Token 与实名并行；未勾选的功能不发送请求。
- 多账号复用当前行第一次提取的凭据，同时最多处理两个非当前账号；单账号内 Token 与密码并行。
- 跨命令缓存只能保存 ZIP 内部路径等非敏感元数据，不保存 Token、Cookie、证书或 ticket-guard 原值。
- 备份目录继续实时查询 `Manifest.db`，不进入 ZIP 路径索引缓存。
- 执行时已确认当前目录属于 Git 仓库；实现位于 `codex/balanced-batch-performance` 分支，保护 `main` 和用户未跟踪文件。各任务保留测试检查点，本计划不自动提交或推送。

---

## File Structure

- Create `src/batch-performance.ts`: 平衡 worker 计算、有限并发映射、异步步骤计时、分步耗时稳定格式化。
- Create `tests/batch-performance.test.ts`: 纯性能工具的行为和并发测试。
- Modify `src/App.tsx`: 接入 worker 计算、单包并行编排、凭据复用、多账号限流、步骤耗时展示与 CSV 导出。
- Modify `tests/workbench-layout.test.ts`: 用源码契约保护 `App.tsx` 的关键接线，防止重新引入 20 路并发、重复凭据提取或丢失耗时展示。
- Modify `src-tauri/src/lib.rs`: 添加 ZIP APP 路径索引缓存、缓存键和 Rust 单元测试。
- Read only `docs/superpowers/specs/2026-07-22-balanced-batch-detection-performance-design.md`: 每个任务完成后核对已确认规格。

---

### Task 1: Add tested batch-performance primitives

**Files:**
- Create: `src/batch-performance.ts`
- Create: `tests/batch-performance.test.ts`

**Interfaces:**
- Consumes: browser/Node `performance.now()` and caller-supplied async functions.
- Produces: `resolveBalancedWorkerCount(rowCount, cpuCores)`, `mapWithConcurrency(items, limit, worker)`, `measureAsyncStep(task)`, `formatStepTimings(timings)`, `DetectionStepTimings`, and `TimedOutcome<T>`.

- [ ] **Step 1: Write the failing unit tests**

Create `tests/batch-performance.test.ts` with:

```ts
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
```

- [ ] **Step 2: Run the focused test and verify the missing-module failure**

Run: `node --test tests/batch-performance.test.ts`

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `src/batch-performance.ts`.

- [ ] **Step 3: Implement the complete pure helper module**

Create `src/batch-performance.ts` with:

```ts
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
```

- [ ] **Step 4: Run the focused test and type/build checks**

Run: `node --test tests/batch-performance.test.ts`

Expected: 5 tests PASS.

Run: `npm run build`

Expected: TypeScript and Vite build succeed.

- [ ] **Step 5: Record the checkpoint**

Record in the task log: `Task 1 complete: batch-performance primitives pass focused tests and npm build.`

---

### Task 2: Replace package scheduling with the balanced worker policy

**Files:**
- Modify: `src/App.tsx:14-22,202-240,398,1360-1402,2410-2457`
- Modify: `tests/workbench-layout.test.ts:21+`

**Interfaces:**
- Consumes: `resolveBalancedWorkerCount` and `DetectionStepTimings` from Task 1.
- Produces: every `BatchDetectionRow` has `stepTimings`; the batch runner launches only the resolved worker count.

- [ ] **Step 1: Add the failing source-contract test**

Append to `tests/workbench-layout.test.ts`:

```ts
test("uses the balanced batch worker policy", () => {
  assert.match(
    appSource,
    /resolveBalancedWorkerCount\(initialRows\.length, navigator\.hardwareConcurrency\)/,
  );
  assert.doesNotMatch(appSource, /Math\.min\(20, Math\.max\(4, cpuCores \* 2\)\)/);
});
```

- [ ] **Step 2: Run the focused source test and verify it fails**

Run: `node --test tests/workbench-layout.test.ts`

Expected: FAIL because `App.tsx` still contains the 4–20 worker calculation.

- [ ] **Step 3: Add the performance imports and row timing field**

Add this import after `batch-options`:

```ts
import {
  formatStepTimings,
  mapWithConcurrency,
  measureAsyncStep,
  resolveBalancedWorkerCount,
  type DetectionStepTimings,
  type TimedOutcome,
} from "./batch-performance";
```

Add this field immediately after `durationMs` in `BatchDetectionRow`:

```ts
  stepTimings: DetectionStepTimings;
```

Initialize it beside `durationMs: null` in `buildInitialBatchRows`:

```ts
        durationMs: null,
        stepTimings: {},
```

- [ ] **Step 4: Replace the worker-count calculation**

Replace the `cpuCores/maxConcurrency/workerCount` block with:

```ts
    const workerCount = resolveBalancedWorkerCount(
      initialRows.length,
      navigator.hardwareConcurrency,
    );
    setStatus(`批量检测开始，包数 ${initialRows.length}，并发 ${workerCount} 路...`);
```

Do not change stop behavior: workers still stop taking new rows when `batchStopRef.current` becomes true and wait for already-issued commands.

- [ ] **Step 5: Run focused and full frontend tests**

Run: `node --test tests/workbench-layout.test.ts tests/batch-performance.test.ts`

Expected: 8 tests PASS.

Run: `npm test`

Expected: all frontend tests PASS.

- [ ] **Step 6: Record the checkpoint**

Record: `Task 2 complete: App uses 6–8 balanced workers and rows initialize step timings.`

---

### Task 3: Parallelize current-account checks and preserve truthful errors

**Files:**
- Modify: `src/App.tsx:2459-2720`
- Modify: `tests/workbench-layout.test.ts`

**Interfaces:**
- Consumes: `measureAsyncStep`, `TimedOutcome<T>`, and `DetectionStepTimings`.
- Produces: one cached `DouyinAccountCredentialResult | null` per row; timed current-account outcomes whose failures do not cancel sibling checks.

- [ ] **Step 1: Add failing orchestration-contract tests**

Append to `tests/workbench-layout.test.ts`:

```ts
test("runs current-account checks in parallel and extracts Douyin credentials once", () => {
  const start = appSource.indexOf("async function runBatchDetectionForRow");
  const end = appSource.indexOf("function firstEndpointValue", start);
  const batchFunction = appSource.slice(start, end);

  assert.match(batchFunction, /const \[requestParamsOutcome, credentialsOutcome\] = await Promise\.all/);
  assert.match(batchFunction, /const \[tokenOutcome, passwordOutcome, certificationOutcome\] = await Promise\.all/);
  assert.match(batchFunction, /const \[toutiaoTokenOutcome, toutiaoCertificationOutcome\] = await Promise\.all/);
  assert.equal(
    [...batchFunction.matchAll(/"extract_douyin_account_credentials"/g)].length,
    1,
  );
});
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `node --test tests/workbench-layout.test.ts`

Expected: FAIL because the current implementation is serial and extracts credentials twice.

- [ ] **Step 3: Add per-row timing and reusable credentials state**

At the beginning of `runBatchDetectionForRow`, after the error arrays, add:

```ts
  const stepTimings: DetectionStepTimings = {};
  let douyinCredentials: DouyinAccountCredentialResult | null = null;
```

Replace the two serial local extraction blocks with this parallel block:

```ts
    const localPreparationStartedAt = performance.now();
    const [requestParamsOutcome, credentialsOutcome] = await Promise.all([
      measureAsyncStep(() => invoke<DouyinRequestParamsResult>(
        "extract_douyin_request_params",
        { zipPath: row.sourceZip },
      )),
      measureAsyncStep(() => invoke<DouyinAccountCredentialResult>(
        "extract_douyin_account_credentials",
        { zipPath: row.sourceZip },
      )),
    ]);
    stepTimings.localPreparation = Math.round(performance.now() - localPreparationStartedAt);

    if (requestParamsOutcome.status === "fulfilled") {
      const requestParams = requestParamsOutcome.value;
      fullParams = requestParams.headerText || "-";
      secUid = requestParams.secUserId?.trim() || secUid;
    } else {
      fullParams = `提取失败：${String(requestParamsOutcome.reason)}`;
      errors.push(fullParams);
    }

    if (credentialsOutcome.status === "fulfilled") {
      douyinCredentials = credentialsOutcome.value;
      const accounts = douyinCredentials.accounts || [];
      if (accounts.length > 0) {
        const currentLocalAccount = accounts.find((account) => account.isCurrent)
          || (accounts.length === 1 ? accounts[0] : null);
        uid = dedupeText(accounts.map((account) => account.uid || "-")).join(" | ");
        secUid = dedupeText(accounts.map((account) => account.secUid || "-")).join(" | ");
        uniqueId = dedupeText(accounts.map((account) => account.uniqueId || account.shortId || "-")).join(" | ");
        accountName = dedupeText(accounts.map((account) => account.nickname || "未实名")).join(" | ");
        if (currentLocalAccount) {
          uid = currentLocalAccount.uid || uid;
          secUid = currentLocalAccount.secUid || secUid;
          uniqueId = currentLocalAccount.uniqueId || currentLocalAccount.shortId || uniqueId;
          accountName = currentLocalAccount.nickname || accountName;
          phoneNumber = currentLocalAccount.phoneNumber || phoneNumber;
          registerTime = formatRegisterTime(currentLocalAccount.registerTime) || registerTime;
          awemeCount = currentLocalAccount.awemeCount || awemeCount;
          followingCount = currentLocalAccount.followingCount || followingCount;
          likedCount = currentLocalAccount.likedCount || likedCount;
          bindingSummary = currentLocalAccount.bindings.summary || bindingSummary;
          toutiaoBinding = currentLocalAccount.bindings.toutiao || toutiaoBinding;
          toutiaoPlatformScreenName = currentLocalAccount.bindings.toutiaoPlatformScreenName || toutiaoPlatformScreenName;
          qqBinding = currentLocalAccount.bindings.qq || qqBinding;
          qqPlatformScreenName = currentLocalAccount.bindings.qqPlatformScreenName || qqPlatformScreenName;
          googleBinding = currentLocalAccount.bindings.google || googleBinding;
          googlePlatformScreenName = currentLocalAccount.bindings.googlePlatformScreenName || googlePlatformScreenName;
          appleIdBinding = currentLocalAccount.bindings.appleId || appleIdBinding;
          appleIdPlatformScreenName = currentLocalAccount.bindings.appleIdPlatformScreenName || appleIdPlatformScreenName;
          wechatBinding = currentLocalAccount.bindings.wechat || wechatBinding;
          wechatPlatformScreenName = currentLocalAccount.bindings.wechatPlatformScreenName || wechatPlatformScreenName;
          normalFunctions.push(...currentLocalAccount.normalFunctions);
        }
      }
    } else {
      errors.push(`抖音账号凭据提取失败：${String(credentialsOutcome.reason)}`);
    }
```

- [ ] **Step 4: Start the three Douyin current-account network checks together**

Replace the serial Token/password/certification invocations with these typed tasks and the single `Promise.all`:

```ts
    const tokenTask: Promise<TimedOutcome<DouyinTokenStatusResult> | null> = options.token
      ? measureAsyncStep(() => invoke<DouyinTokenStatusResult>("check_douyin_token_status", { zipPath: row.sourceZip }))
      : Promise.resolve(null);
    const passwordTask: Promise<TimedOutcome<DouyinPasswordStatusResult> | null> = shouldFetchDouyinSession
      ? measureAsyncStep(() => invoke<DouyinPasswordStatusResult>("check_douyin_password_status", { zipPath: row.sourceZip }))
      : Promise.resolve(null);
    const certificationTask: Promise<TimedOutcome<DouyinCertificationStatusResult> | null> = options.certification
      ? measureAsyncStep(() => invoke<DouyinCertificationStatusResult>("check_douyin_certification_status", { zipPath: row.sourceZip }))
      : Promise.resolve(null);

    const [tokenOutcome, passwordOutcome, certificationOutcome] = await Promise.all([
      tokenTask,
      passwordTask,
      certificationTask,
    ]);
    if (tokenOutcome) stepTimings.token = tokenOutcome.durationMs;
    if (passwordOutcome) stepTimings.password = passwordOutcome.durationMs;
    if (certificationOutcome) stepTimings.certification = certificationOutcome.durationMs;
```

Merge fulfilled Douyin outcomes with this code before handling rejected outcomes:

```ts
    if (tokenOutcome?.status === "fulfilled") {
      const tokenResult = tokenOutcome.value;
      tokenStatus = formatDouyinTokenLabel(tokenResult);
      const endpointAccount = tokenResult.endpoints.find((endpoint) => endpoint.nickname || endpoint.uid);
      accountName = endpointAccount?.nickname || endpointAccount?.uid || accountName || "未实名";
      secUid = firstEndpointValue(tokenResult.endpoints, "secUid") || secUid;
      uid = firstEndpointValue(tokenResult.endpoints, "uid") || uid;
      phoneNumber = tokenResult.localPhoneNumber
        || firstEndpointValue(tokenResult.endpoints, "phoneNumber")
        || phoneNumber;
      registerTime = formatRegisterTime(firstEndpointValue(tokenResult.endpoints, "registerTime"))
        || registerTime;
      awemeCount = firstEndpointValue(tokenResult.endpoints, "awemeCount") || awemeCount;
      followingCount = firstEndpointValue(tokenResult.endpoints, "followingCount") || followingCount;
      likedCount = firstEndpointValue(tokenResult.endpoints, "likedCount") || likedCount;
      const hasFunctionItems = tokenResult.functions.length > 0;
      for (const fn of tokenResult.functions) {
        if (fn.funcAvailable) normalFunctions.push(fn.funcName);
        else limitedFunctions.push(fn.funcName);
      }
      if (tokenResult.status === "ok") {
        onlineSignal = true;
        childLockStatus = "无";
      } else if (tokenResult.status === "invalid") {
        offlineSignal = true;
        childLockStatus = "未知";
        if (!hasFunctionItems) limitedFunctions.push("Token 失效");
      } else if (tokenResult.status.startsWith("missing_")) {
        childLockStatus = "未知";
        if (!hasFunctionItems) limitedFunctions.push("Token 缺参数");
      } else {
        childLockStatus = "未知";
      }
      if (tokenResult.error) errors.push(tokenResult.error);
    }

    if (passwordOutcome?.status === "fulfilled") {
      const passwordResult = passwordOutcome.value;
      accountName = passwordResult.accountName || accountName || "未实名";
      registerTime = formatRegisterTime(passwordResult.registerTime) || registerTime;
      bindingSummary = passwordResult.bindings.summary || bindingSummary;
      toutiaoBinding = passwordResult.bindings.toutiao || toutiaoBinding;
      toutiaoPlatformScreenName = passwordResult.bindings.toutiaoPlatformScreenName || toutiaoPlatformScreenName;
      qqBinding = passwordResult.bindings.qq || qqBinding;
      qqPlatformScreenName = passwordResult.bindings.qqPlatformScreenName || qqPlatformScreenName;
      googleBinding = passwordResult.bindings.google || googleBinding;
      googlePlatformScreenName = passwordResult.bindings.googlePlatformScreenName || googlePlatformScreenName;
      appleIdBinding = passwordResult.bindings.appleId || appleIdBinding;
      appleIdPlatformScreenName = passwordResult.bindings.appleIdPlatformScreenName || appleIdPlatformScreenName;
      wechatBinding = passwordResult.bindings.wechat || wechatBinding;
      wechatPlatformScreenName = passwordResult.bindings.wechatPlatformScreenName || wechatPlatformScreenName;
      if (options.password) {
        passwordStatus = formatDouyinPasswordLabel(passwordResult);
        if (passwordResult.hasPassword === true) normalFunctions.push("改密功能");
        else if (passwordResult.hasPassword === false) limitedFunctions.push("未设置密码");
        else if (passwordResult.error) errors.push(passwordResult.error);
      }
    }

    if (certificationOutcome?.status === "fulfilled") {
      const certificationResult = certificationOutcome.value;
      certificationStatus = formatDouyinCertificationLabel(certificationResult);
      accountName = certificationResult.accountName || accountName || "未实名";
      if (certificationResult.isVerified === true) normalFunctions.push("实名正常");
      else if (certificationResult.isVerified === false) limitedFunctions.push("未实名");
      else if (certificationResult.error) errors.push(certificationResult.error);
    }
```

For rejected outcomes, use these exact state changes:

```ts
    if (tokenOutcome?.status === "rejected") {
      tokenStatus = "请求失败";
      limitedFunctions.push("Token 请求失败");
      errors.push(`douyin_token_command_failed: ${String(tokenOutcome.reason)}`);
    }
    if (passwordOutcome?.status === "rejected") {
      if (options.password) passwordStatus = "请求失败";
      errors.push(`douyin_password_command_failed: ${String(passwordOutcome.reason)}`);
    }
    if (certificationOutcome?.status === "rejected") {
      certificationStatus = "请求失败";
      errors.push(`douyin_certification_command_failed: ${String(certificationOutcome.reason)}`);
    }
```

For a fulfilled Token outcome, retain all current endpoint/profile/function mappings and the exact status rules: only `status === "invalid"` sets `offlineSignal = true`; `ok` sets `onlineSignal = true`; missing/skipped/error statuses do not set offline.

- [ ] **Step 5: Keep identity resolution after the Token result and time it separately**

Inside the fulfilled Token branch, after `secUid` and `uid` are merged, replace the existing identity `try/catch` with:

```ts
      if (secUid && secUid !== "-" && (!uid || uid === "-" || !uniqueId || uniqueId === "-")) {
        const identityOutcome = await measureAsyncStep(() => invoke<DouyinUniqueIdResult>(
          "resolve_douyin_unique_id",
          { secUid },
        ));
        stepTimings.identity = identityOutcome.durationMs;
        if (identityOutcome.status === "fulfilled") {
          secUid = identityOutcome.value.secUid || secUid;
          uid = identityOutcome.value.uid || uid;
          uniqueId = identityOutcome.value.uniqueId || uniqueId;
        } else {
          errors.push(`抖音身份补全失败：${String(identityOutcome.reason)}`);
        }
      }
```

- [ ] **Step 6: Parallelize the two Toutiao checks**

In the Toutiao branch, create these two tasks:

```ts
    const toutiaoTokenTask: Promise<TimedOutcome<ToutiaoTokenStatusResult> | null> = options.token
      ? measureAsyncStep(() => invoke<ToutiaoTokenStatusResult>("check_toutiao_token_status", { zipPath: row.sourceZip }))
      : Promise.resolve(null);
    const toutiaoCertificationTask: Promise<TimedOutcome<ToutiaoCertificationStatusResult> | null> = options.certification
      ? measureAsyncStep(() => invoke<ToutiaoCertificationStatusResult>("check_toutiao_certification_status", { zipPath: row.sourceZip }))
      : Promise.resolve(null);
    const [toutiaoTokenOutcome, toutiaoCertificationOutcome] = await Promise.all([
      toutiaoTokenTask,
      toutiaoCertificationTask,
    ]);
    if (toutiaoTokenOutcome) stepTimings.token = toutiaoTokenOutcome.durationMs;
    if (toutiaoCertificationOutcome) stepTimings.certification = toutiaoCertificationOutcome.durationMs;
```

Merge fulfilled Toutiao outcomes with:

```ts
    if (toutiaoTokenOutcome?.status === "fulfilled") {
      const tokenResult = toutiaoTokenOutcome.value;
      tokenStatus = formatToutiaoTokenStatus(tokenResult.status);
      accountName = tokenResult.nickname || accountName;
      uid = tokenResult.uid || uid;
      registerTime = formatRegisterTime(tokenResult.registerTime) || registerTime;
      fullParams = [
        "app_name=news_article",
        `device_id=${tokenResult.deviceId || "-"}`,
        "aid=13",
        `iid=${tokenResult.iid || "-"}`,
        "detail=my_tabs_v2",
        "user_app_id=1128",
      ].join("\n");
      if (tokenResult.status === "ok") {
        onlineSignal = true;
        normalFunctions.push("Token 在线", "登录功能");
      } else if (tokenResult.status === "invalid") {
        offlineSignal = true;
        limitedFunctions.push("Token 失效");
      } else {
        limitedFunctions.push(tokenStatus);
      }
      if (tokenResult.error) errors.push(tokenResult.error);
    }

    if (toutiaoCertificationOutcome?.status === "fulfilled") {
      const certificationResult = toutiaoCertificationOutcome.value;
      certificationStatus = formatToutiaoCertificationLabel(certificationResult);
      if (certificationResult.isVerified === true) {
        onlineSignal = true;
        normalFunctions.push("登录功能", "实名正常");
      } else if (certificationResult.isVerified === false) {
        offlineSignal = true;
        limitedFunctions.push("未实名");
      } else if (certificationResult.error) {
        errors.push(certificationResult.error);
      }
    }
```

Use these exact rejected-outcome mappings:

```ts
    if (toutiaoTokenOutcome?.status === "rejected") {
      tokenStatus = "请求失败";
      limitedFunctions.push("Token 请求失败");
      errors.push(`toutiao_token_command_failed: ${String(toutiaoTokenOutcome.reason)}`);
    }
    if (toutiaoCertificationOutcome?.status === "rejected") {
      certificationStatus = "请求失败";
      errors.push(`toutiao_certification_command_failed: ${String(toutiaoCertificationOutcome.reason)}`);
    }
```

Only a fulfilled Token result with `status === "invalid"`, or a fulfilled certification result with `isVerified === false`, may set the existing Toutiao offline signal.

- [ ] **Step 7: Attach timings to the base row and run checks**

Add this field to `baseRow`:

```ts
    stepTimings: { ...stepTimings },
```

Run: `node --test tests/workbench-layout.test.ts tests/batch-performance.test.ts`

Expected: all focused tests PASS.

Run: `npm run build`

Expected: TypeScript and Vite build succeed with no union-narrowing errors.

- [ ] **Step 8: Record the checkpoint**

Record: `Task 3 complete: current Douyin/Toutiao checks run in parallel, errors stay truthful, and credentials are extracted once.`

---

### Task 4: Limit and parallelize non-current Douyin accounts

**Files:**
- Modify: `src/App.tsx:2720-2865`
- Modify: `tests/workbench-layout.test.ts`

**Interfaces:**
- Consumes: `douyinCredentials` from Task 3 and `mapWithConcurrency` from Task 1.
- Produces: ordered multi-account rows, at most two non-current account workers, with Token/password requests started together per account.

- [ ] **Step 1: Add the failing multi-account contract test**

Append to `tests/workbench-layout.test.ts`:

```ts
test("reuses credentials and caps non-current Douyin account work at two", () => {
  const start = appSource.indexOf("async function runBatchDetectionForRow");
  const end = appSource.indexOf("function firstEndpointValue", start);
  const batchFunction = appSource.slice(start, end);

  assert.match(batchFunction, /mapWithConcurrency\(douyinCredentials\.accounts, 2,/);
  assert.match(batchFunction, /const \[accountParamsOutcome, accountTokenOutcome, accountPasswordOutcome\] = await Promise\.all/);
  assert.doesNotMatch(batchFunction, /const creds = await invoke<DouyinAccountCredentialResult>/);
});
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `node --test tests/workbench-layout.test.ts`

Expected: FAIL because multi-account work still uses unbounded `Promise.all` and a second extraction command.

- [ ] **Step 3: Reuse the first credentials result and cap account workers**

Replace the second credential extraction wrapper with:

```ts
  if (row.appType === "douyin" && douyinCredentials?.accounts.length && douyinCredentials.accounts.length > 1) {
    const multiAccountStartedAt = performance.now();
    const activeUid = douyinCredentials.accounts.find((account) => account.isCurrent)?.uid
      || (baseRow.uid !== "-" && !baseRow.uid.includes(" | ") ? baseRow.uid : null);
    const mappedRows = await mapWithConcurrency(douyinCredentials.accounts, 2, async (acc) => {
      const isCurrent = acc.isCurrent || acc.uid === activeUid;
      if (isCurrent) return baseRow;
```

Immediately after the early current-account return, initialize the non-current row with this complete block:

```ts
      const accHasActToken = isActStyleToken(acc.accessToken);
      const shouldFetchSessionDetails = options.password || options.registrationTime;
      let accTokenStatus = options.token ? "未知(非当前)" : "跳过";
      let accFullParams = "-";
      let accPasswordStatus = options.password ? "未知(非当前)" : "跳过";
      let accAccountName = acc.nickname || "未实名";
      let accBindingSummary = acc.bindings.summary || "-";
      let accToutiaoBinding = acc.bindings.toutiao || "-";
      let accToutiaoPlatformScreenName = acc.bindings.toutiaoPlatformScreenName || "-";
      let accQqBinding = acc.bindings.qq || "-";
      let accQqPlatformScreenName = acc.bindings.qqPlatformScreenName || "-";
      let accGoogleBinding = acc.bindings.google || "-";
      let accGooglePlatformScreenName = acc.bindings.googlePlatformScreenName || "-";
      let accAppleIdBinding = acc.bindings.appleId || "-";
      let accAppleIdPlatformScreenName = acc.bindings.appleIdPlatformScreenName || "-";
      let accWechatBinding = acc.bindings.wechat || "-";
      let accWechatPlatformScreenName = acc.bindings.wechatPlatformScreenName || "-";
      let accUid = acc.uid || "-";
      let accSecUid = acc.secUid || "-";
      let accUniqueId = acc.uniqueId || acc.shortId || "-";
      let accPhoneNumber = acc.phoneNumber || "-";
      let accRegisterTime = formatRegisterTime(acc.registerTime) || "-";
      let accAwemeCount = acc.awemeCount || "-";
      let accFollowingCount = acc.followingCount || "-";
      let accLikedCount = acc.likedCount || "-";
      const accNormalFunctions = [...acc.normalFunctions];
      const accLimitedFunctions: string[] = [];

      if (accHasActToken) {
        accFullParams = "跳过(act token)";
        if (options.token) accTokenStatus = "跳过(act token)";
      }
```

Close the worker and timing block with:

```ts
    });
    stepTimings.multiAccount = Math.round(performance.now() - multiAccountStartedAt);
    return mappedRows.map((mappedRow) => ({
      ...mappedRow,
      stepTimings: { ...stepTimings },
    }));
  }
```

This preserves account order because `mapWithConcurrency` writes results by input index.

- [ ] **Step 4: Start each non-current account's independent commands together**

For each non-current account, replace the serial request-params, Token and password invocations with:

```ts
      const accountParamsTask: Promise<TimedOutcome<DouyinRequestParamsResult> | null> = acc.accessToken && !accHasActToken
        ? measureAsyncStep(() => invoke<DouyinRequestParamsResult>("extract_douyin_request_params", {
            zipPath: row.sourceZip,
            tokenOverride: acc.accessToken,
          }))
        : Promise.resolve(null);
      const accountTokenTask: Promise<TimedOutcome<DouyinTokenStatusResult> | null> = options.token && acc.accessToken && !accHasActToken
        ? measureAsyncStep(() => invoke<DouyinTokenStatusResult>("check_douyin_token_status", {
            zipPath: row.sourceZip,
            tokenOverride: acc.accessToken,
          }))
        : Promise.resolve(null);
      const accountPasswordTask: Promise<TimedOutcome<DouyinPasswordStatusResult> | null> = acc.sessionId && shouldFetchSessionDetails
        ? measureAsyncStep(() => invoke<DouyinPasswordStatusResult>("check_douyin_password_status", {
            zipPath: row.sourceZip,
            sessionIdOverride: acc.sessionId,
          }))
        : Promise.resolve(null);
      const [accountParamsOutcome, accountTokenOutcome, accountPasswordOutcome] = await Promise.all([
        accountParamsTask,
        accountTokenTask,
        accountPasswordTask,
      ]);
```

Merge fulfilled outcomes with these exact assignments:

```ts
      if (accountParamsOutcome?.status === "fulfilled") {
        accFullParams = accountParamsOutcome.value.headerText || "-";
        if (accountParamsOutcome.value.secUserId) {
          accSecUid = accountParamsOutcome.value.secUserId.trim();
        }
      }

      if (accountTokenOutcome?.status === "fulfilled") {
        const tokenResult = accountTokenOutcome.value;
        accTokenStatus = formatDouyinTokenLabel(tokenResult);
        accPhoneNumber = tokenResult.localPhoneNumber || accPhoneNumber;
        for (const fn of tokenResult.functions) {
          if (fn.funcAvailable) accNormalFunctions.push(fn.funcName);
          else accLimitedFunctions.push(fn.funcName);
        }
        if (tokenResult.validEndpointCount > 0) {
          const endpoint = tokenResult.endpoints.find((item) => item.status === "ok");
          if (endpoint?.uid) accUid = endpoint.uid;
          if (endpoint?.secUid) accSecUid = endpoint.secUid;
          if (endpoint?.nickname) accAccountName = endpoint.nickname;
          if (endpoint?.phoneNumber) accPhoneNumber = endpoint.phoneNumber;
          if (endpoint?.registerTime) {
            accRegisterTime = formatRegisterTime(endpoint.registerTime) || accRegisterTime;
          }
          if (endpoint?.awemeCount != null) accAwemeCount = String(endpoint.awemeCount);
          if (endpoint?.followingCount != null) accFollowingCount = String(endpoint.followingCount);
          if (endpoint?.likedCount != null) accLikedCount = String(endpoint.likedCount);
        }
        if (tokenResult.status === "invalid" && tokenResult.functions.length === 0) {
          accLimitedFunctions.push("Token 失效");
        } else if (tokenResult.status.startsWith("missing_") && tokenResult.functions.length === 0) {
          accLimitedFunctions.push("Token 缺参数");
        }
      }

      if (accountPasswordOutcome?.status === "fulfilled") {
        const passwordResult = accountPasswordOutcome.value;
        if (options.password) {
          accPasswordStatus = formatDouyinPasswordLabel(passwordResult);
          if (passwordResult.hasPassword === true) accNormalFunctions.push("改密功能");
          else if (passwordResult.hasPassword === false) accLimitedFunctions.push("未设置密码");
        }
        if (passwordResult.accountName) accAccountName = passwordResult.accountName;
        if (passwordResult.registerTime) {
          accRegisterTime = formatRegisterTime(passwordResult.registerTime) || accRegisterTime;
        }
        if (passwordResult.bindings.summary) accBindingSummary = passwordResult.bindings.summary;
        if (passwordResult.bindings.toutiao) accToutiaoBinding = passwordResult.bindings.toutiao;
        if (passwordResult.bindings.toutiaoPlatformScreenName) {
          accToutiaoPlatformScreenName = passwordResult.bindings.toutiaoPlatformScreenName;
        }
        if (passwordResult.bindings.qq) accQqBinding = passwordResult.bindings.qq;
        if (passwordResult.bindings.qqPlatformScreenName) {
          accQqPlatformScreenName = passwordResult.bindings.qqPlatformScreenName;
        }
        if (passwordResult.bindings.google) accGoogleBinding = passwordResult.bindings.google;
        if (passwordResult.bindings.googlePlatformScreenName) {
          accGooglePlatformScreenName = passwordResult.bindings.googlePlatformScreenName;
        }
        if (passwordResult.bindings.appleId) accAppleIdBinding = passwordResult.bindings.appleId;
        if (passwordResult.bindings.appleIdPlatformScreenName) {
          accAppleIdPlatformScreenName = passwordResult.bindings.appleIdPlatformScreenName;
        }
        if (passwordResult.bindings.wechat) accWechatBinding = passwordResult.bindings.wechat;
        if (passwordResult.bindings.wechatPlatformScreenName) {
          accWechatPlatformScreenName = passwordResult.bindings.wechatPlatformScreenName;
        }
      }
```

Map rejections without cancelling the row:

```ts
      if (accountParamsOutcome?.status === "rejected") {
        accFullParams = `提取失败：${String(accountParamsOutcome.reason)}`;
      }
      if (accountTokenOutcome?.status === "rejected") {
        accTokenStatus = "请求失败";
        accLimitedFunctions.push("Token 请求失败");
      }
      if (accountPasswordOutcome?.status === "rejected" && options.password) {
        accPasswordStatus = "请求失败";
        accLimitedFunctions.push("密码请求失败");
      }
```

If `acc.accessToken` is absent, keep `accFullParams = "-"`. If it is an act-style token, keep `accFullParams = "跳过(act token)"` and `accTokenStatus = "跳过(act token)"` when Token detection is selected.

Return the completed non-current row from the account worker with:

```ts
      return {
        ...baseRow,
        fullParams: accFullParams,
        accountName: accAccountName,
        bindingSummary: accBindingSummary,
        toutiaoBinding: accToutiaoBinding,
        toutiaoPlatformScreenName: accToutiaoPlatformScreenName,
        qqBinding: accQqBinding,
        qqPlatformScreenName: accQqPlatformScreenName,
        googleBinding: accGoogleBinding,
        googlePlatformScreenName: accGooglePlatformScreenName,
        appleIdBinding: accAppleIdBinding,
        appleIdPlatformScreenName: accAppleIdPlatformScreenName,
        wechatBinding: accWechatBinding,
        wechatPlatformScreenName: accWechatPlatformScreenName,
        uid: accUid,
        secUid: accSecUid,
        uniqueId: accUniqueId,
        phoneNumber: accPhoneNumber,
        registerTime: accRegisterTime,
        awemeCount: accAwemeCount,
        followingCount: accFollowingCount,
        likedCount: accLikedCount,
        tokenStatus: accTokenStatus,
        passwordStatus: accPasswordStatus,
        certificationStatus: "未知(非当前)",
        normalFunctions: dedupeText(accNormalFunctions).join("｜"),
        limitedFunctions: dedupeText(accLimitedFunctions).join("｜"),
        status: "skipped" as BatchDetectionRow["status"],
      };
```

- [ ] **Step 5: Run focused tests and build**

Run: `node --test tests/workbench-layout.test.ts tests/batch-performance.test.ts`

Expected: all focused tests PASS.

Run: `npm run build`

Expected: build succeeds.

- [ ] **Step 6: Record the checkpoint**

Record: `Task 4 complete: non-current accounts use ordered two-way concurrency and parallel per-account checks.`

---

### Task 5: Show and export step timings

**Files:**
- Modify: `src/App.tsx:326-403,1427-1480,1735-1785`
- Modify: `tests/workbench-layout.test.ts`

**Interfaces:**
- Consumes: `formatStepTimings(row.stepTimings)`.
- Produces: table tooltip, detail card and CSV `分步耗时` column; timing strings contain no request values.

- [ ] **Step 1: Add the failing presentation-contract test**

Append to `tests/workbench-layout.test.ts`:

```ts
test("shows and exports structured step timings", () => {
  assert.match(appSource, /title=\{formatStepTimings\(row\.stepTimings\)\}/);
  assert.match(appSource, /\["分步耗时", formatStepTimings\(selectedBatchRow\.stepTimings\)\]/);
  assert.match(appSource, /"用时\(ms\)", "分步耗时", "来源ZIP"/);
  assert.match(appSource, /row\.durationMs \?\? "",\s*formatStepTimings\(row\.stepTimings\)/s);
});
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `node --test tests/workbench-layout.test.ts`

Expected: FAIL because the timing tooltip/detail/CSV column is absent.

- [ ] **Step 3: Add the table tooltip**

Replace the duration cell with:

```tsx
      <td title={formatStepTimings(row.stepTimings)}>
        {formatDetectionDuration(row.durationMs)}
      </td>
```

- [ ] **Step 4: Add detail and CSV output**

Add this detail card tuple immediately after the total-duration tuple:

```ts
      ["分步耗时", formatStepTimings(selectedBatchRow.stepTimings)],
```

Change the CSV tail headers and values to:

```ts
      "正常功能", "限制功能", "用时(ms)", "分步耗时", "来源ZIP", "错误",
```

```ts
        row.normalFunctions,
        row.limitedFunctions,
        row.durationMs ?? "",
        formatStepTimings(row.stepTimings),
        formatBaseName(row.sourceZip),
        row.error ?? "",
```

- [ ] **Step 5: Run frontend regression tests and build**

Run: `npm test`

Expected: all frontend tests PASS.

Run: `npm run build`

Expected: TypeScript and Vite build succeed.

- [ ] **Step 6: Record the checkpoint**

Record: `Task 5 complete: total and structured step timings appear in table, detail and CSV.`

---

### Task 6: Cache non-sensitive ZIP APP path indexes in Rust

**Files:**
- Modify: `src-tauri/src/lib.rs:1-24,315-320,3251-3318,3923-4000,7255+,8396+`

**Interfaces:**
- Consumes: existing `open_zip`, `split_entry_path`, `load_backup_manifest_context`, `build_backup_virtual_path`, `build_zip_cache_key`, and `CACHE_STATE`.
- Produces: `app_file_path_indexes: BTreeMap<String, Vec<String>>`, `build_app_file_path_index_cache_key`, `get_or_build_app_file_path_index`, and a cached `find_app_file_path` ZIP branch.

- [ ] **Step 1: Add Rust tests and a ZIP fixture helper**

Inside `#[cfg(test)] mod tests`, add imports and helper:

```rust
    use std::cell::Cell;
    use std::io::Write;
    use std::time::Duration;
    use zip::write::SimpleFileOptions;

    fn write_test_zip(path: &Path, entries: &[&str]) {
        let file = File::create(path).expect("create test zip");
        let mut writer = zip::ZipWriter::new(file);
        for entry in entries {
            writer
                .start_file(*entry, SimpleFileOptions::default())
                .expect("start test entry");
            writer.write_all(b"test").expect("write test entry");
        }
        writer.finish().expect("finish test zip");
    }
```

Add these tests:

```rust
    #[test]
    fn reuses_cached_app_file_path_index_without_rebuilding() {
        let temp_dir = tempdir().expect("tempdir");
        let cache_key = format!("test-app-index::{}", temp_dir.path().display());
        let builds = Cell::new(0);
        let first = get_or_build_app_file_path_index(&cache_key, || {
            builds.set(builds.get() + 1);
            Ok(vec!["batch/com.demo.app/Library/demo.plist".to_string()])
        })
        .expect("first index");
        let second = get_or_build_app_file_path_index(&cache_key, || {
            builds.set(builds.get() + 1);
            Ok(Vec::new())
        })
        .expect("cached index");

        assert_eq!(builds.get(), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn app_file_path_cache_key_separates_apps() {
        assert_ne!(
            build_app_file_path_index_cache_key("sample.zip::123", "com.demo.one"),
            build_app_file_path_index_cache_key("sample.zip::123", "com.demo.two"),
        );
    }

    #[test]
    fn zip_cache_key_changes_after_file_modification() {
        let temp_dir = tempdir().expect("tempdir");
        let zip_path = temp_dir.path().join("sample.zip");
        fs::write(&zip_path, b"one").expect("initial file");
        let first = build_zip_cache_key(zip_path.to_string_lossy().as_ref()).expect("first key");
        std::thread::sleep(Duration::from_millis(5));
        fs::write(&zip_path, b"two-two").expect("modified file");
        let second = build_zip_cache_key(zip_path.to_string_lossy().as_ref()).expect("second key");
        assert_ne!(first, second);
    }

    #[test]
    fn finds_multiple_suffixes_from_one_zip_app_index() {
        let temp_dir = tempdir().expect("tempdir");
        let zip_path = temp_dir.path().join("apps.zip");
        write_test_zip(
            &zip_path,
            &[
                "batch/com.demo.app/Library/Preferences/demo.plist",
                "batch/com.demo.app/Library/Cookies/Cookies.binarycookies",
                "batch/com.other.app/Library/Preferences/other.plist",
            ],
        );
        let zip_text = zip_path.to_string_lossy();

        let plist = find_app_file_path(
            zip_text.as_ref(),
            "com.demo.app",
            &["Library/Preferences/demo.plist"],
        )
        .expect("plist lookup");
        let cookies = find_app_file_path(
            zip_text.as_ref(),
            "com.demo.app",
            &["Library/Cookies/Cookies.binarycookies"],
        )
        .expect("cookie lookup");

        assert_eq!(plist.as_deref(), Some("batch/com.demo.app/Library/Preferences/demo.plist"));
        assert_eq!(cookies.as_deref(), Some("batch/com.demo.app/Library/Cookies/Cookies.binarycookies"));
    }
```

Keep the existing `finds_app_file_path_in_backup_directory_manifest` test; it is the regression proof that directory sources still use the live manifest path.

- [ ] **Step 2: Run the focused Rust tests and verify compilation fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_file_path -- --nocapture`

Expected: FAIL because the new cache field and helper functions are not defined.

- [ ] **Step 3: Add the cache field and nanosecond modification key**

Extend `CacheState`:

```rust
#[derive(Debug, Default)]
struct CacheState {
    scan_cache: BTreeMap<String, ZipScanSummary>,
    files_cache: BTreeMap<String, Vec<CandidateFile>>,
    parse_cache: BTreeMap<String, ParseResult>,
    app_file_path_indexes: BTreeMap<String, Vec<String>>,
}
```

Change `build_zip_cache_key` to use the full filesystem timestamp resolution:

```rust
fn build_zip_cache_key(zip_path: &str) -> Result<String, String> {
    let metadata = fs::metadata(zip_path).map_err(|error| format!("zip_stat_failed: {error}"))?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("zip_stat_failed: {error}"))?;
    let modified_nanos = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("zip_stat_failed: {error}"))?
        .as_nanos();
    Ok(format!("{zip_path}::{modified_nanos}"))
}
```

- [ ] **Step 4: Implement cache access with lock-failure fallback**

Add beside the existing cache-key/access helpers:

```rust
fn build_app_file_path_index_cache_key(zip_cache_key: &str, app_id: &str) -> String {
    format!("{zip_cache_key}::app-file-paths::v1::{app_id}")
}

fn cache_get_app_file_path_index(cache_key: &str) -> Result<Option<Vec<String>>, String> {
    let cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    Ok(cache.app_file_path_indexes.get(cache_key).cloned())
}

fn cache_put_app_file_path_index(cache_key: String, paths: Vec<String>) -> Result<(), String> {
    let mut cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    cache.app_file_path_indexes.insert(cache_key, paths);
    Ok(())
}

fn get_or_build_app_file_path_index<F>(
    cache_key: &str,
    build: F,
) -> Result<Vec<String>, String>
where
    F: FnOnce() -> Result<Vec<String>, String>,
{
    if let Ok(Some(cached)) = cache_get_app_file_path_index(cache_key) {
        return Ok(cached);
    }
    let paths = build()?;
    let _ = cache_put_app_file_path_index(cache_key.to_string(), paths.clone());
    Ok(paths)
}
```

The `if let Ok(Some(...))` and ignored cache-put error are intentional: a poisoned cache lock becomes a cache miss and does not block real ZIP scanning.

- [ ] **Step 5: Build the complete APP path index and use it only for ZIPs**

Add this index builder before `find_app_file_path`:

```rust
fn build_app_file_path_index(zip_path: &str, app_id: &str) -> Result<Vec<String>, String> {
    let mut archive = open_zip(zip_path)?;
    let mut paths = Vec::new();

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("zip_entry_read_failed: {error}"))?;
        if file.is_dir() {
            continue;
        }
        let inner_path = normalize_path(file.name());
        let Some((_, entry_app_id, _)) = split_entry_path(&inner_path) else {
            continue;
        };
        if entry_app_id == app_id {
            paths.push(inner_path);
        }
    }

    if let Some(context) = load_backup_manifest_context(&mut archive)? {
        let domain = format!("AppDomain-{app_id}");
        let mut statement = context
            .connection
            .prepare(
                "SELECT relativePath FROM Files \
                 WHERE flags = 1 AND domain = ?1",
            )
            .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        let rows = statement
            .query_map([domain.as_str()], |row| row.get::<_, String>(0))
            .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        for row in rows {
            let relative_path =
                row.map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
            paths.push(build_backup_virtual_path(app_id, &relative_path));
        }
    }

    Ok(paths)
}
```

Replace `find_app_file_path` with:

```rust
fn find_app_file_path(
    zip_path: &str,
    app_id: &str,
    suffixes: &[&str],
) -> Result<Option<String>, String> {
    let suffixes_lower = suffixes
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if is_backup_directory_source(zip_path)? {
        return find_app_file_path_in_backup_directory(zip_path, app_id, &suffixes_lower);
    }

    let zip_cache_key = build_zip_cache_key(zip_path)?;
    let cache_key = build_app_file_path_index_cache_key(&zip_cache_key, app_id);
    let paths = get_or_build_app_file_path_index(&cache_key, || {
        build_app_file_path_index(zip_path, app_id)
    })?;

    Ok(paths.into_iter().find(|path| {
        let path_lower = path.to_ascii_lowercase();
        suffixes_lower
            .iter()
            .any(|suffix| path_lower.ends_with(suffix))
    }))
}
```

- [ ] **Step 6: Format and run Rust tests**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Expected: PASS. If formatting is needed, run `cargo fmt --manifest-path src-tauri/Cargo.toml`, then repeat the check.

Run: `cargo test --manifest-path src-tauri/Cargo.toml app_file_path -- --nocapture`

Expected: new cache tests and existing backup-directory lookup test PASS.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: the complete Rust suite passes with the pre-existing ignored test still ignored.

- [ ] **Step 7: Record the checkpoint**

Record: `Task 6 complete: ZIP APP path indexes are cached by path+mtime+app, secrets are not cached, and backup directories remain live.`

---

### Task 7: Full verification and packaged App handoff

**Files:**
- Verify: `src/batch-performance.ts`
- Verify: `tests/batch-performance.test.ts`
- Verify: `tests/workbench-layout.test.ts`
- Verify: `src/App.tsx`
- Verify: `src-tauri/src/lib.rs`
- Produce: `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`

**Interfaces:**
- Consumes: all deliverables from Tasks 1–6.
- Produces: a tested release `.app` and a truthful list of what automated verification does and does not prove.

- [ ] **Step 1: Run the complete frontend suite**

Run: `npm test`

Expected: every Node test passes, including batch performance, package allocation, options, modal and workbench contracts.

- [ ] **Step 2: Run the production frontend build**

Run: `npm run build`

Expected: `tsc` and Vite succeed and create `dist/`.

- [ ] **Step 3: Verify Rust formatting and tests**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Expected: PASS.

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: all non-ignored tests PASS.

- [ ] **Step 4: Build the macOS application bundle**

Run: `npm run tauri build -- --bundles app`

Expected: Tauri reports a successful app bundle at `src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app`.

- [ ] **Step 5: Inspect the artifact and source boundaries**

Run: `test -d "src-tauri/target/release/bundle/macos/iOS Sandbox ZIP Reader.app"`

Expected: exit code 0.

Run: `rg -n "Math\.min\(20|cpuCores \* 2|extract_douyin_account_credentials" src/App.tsx`

Expected: no old 20-worker formula; exactly one credential extraction inside `runBatchDetectionForRow` (the separate single-file UI action may still contain its own command call).

Run: `rg -n "Token|Cookie|certificate|ticket|secret" src-tauri/src/lib.rs | rg "app_file_path_indexes|cache_put_app_file_path_index"`

Expected: no output, proving the new cache write path stores only `Vec<String>` paths and does not mention sensitive values.

- [ ] **Step 6: Report verification limits and real-data retest procedure**

Report the exact test/build counts and artifact path. State that controlled tests prove the concurrency cap, parallel start, error isolation, cache behavior and packaging, but do not prove third-party endpoint speed. For the user's 200+ ZIPs, rerun the same batch in the packaged app and compare the table/CSV `用时(ms)` and `分步耗时` columns; investigate rows over 45 seconds by their longest recorded step instead of labeling them healthy by default.

- [ ] **Step 7: Record the final checkpoint**

Record: `Task 7 complete: full tests/build/package passed and the real-data timing boundary was reported honestly.`
