import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const appSource = readFileSync(new URL("../src/App.tsx", import.meta.url), "utf8");
const appCss = readFileSync(new URL("../src/App.css", import.meta.url), "utf8");

test("renders the empty state as a viewport overlay instead of centering it in the wide table", () => {
  assert.match(appSource, /className="detector-empty-overlay"/);
  assert.match(appCss, /\.detector-empty-overlay\s*\{[^}]*position:\s*absolute/s);
  assert.doesNotMatch(appSource, /<td[^>]*className="detector-empty-cell"[\s\S]*className="detector-empty-state"/);
});

test("separates Douyin and Toutiao online package allocation", () => {
  assert.match(appSource, /分配抖音在线包/);
  assert.match(appSource, /分配头条在线包/);
  assert.match(appSource, /buildOnlineAllocationPlan\(batchRows, platform\)/);
  assert.match(appSource, /invoke<string\[\]>\("copy_zip_files"/);
  assert.doesNotMatch(appSource, /\? "移动中\.\.\." : "分配在线包"/);
});

test("uses the balanced batch worker policy", () => {
  assert.match(
    appSource,
    /resolveBalancedWorkerCount\(initialRows\.length, navigator\.hardwareConcurrency\)/,
  );
  assert.doesNotMatch(appSource, /Math\.min\(20, Math\.max\(4, cpuCores \* 2\)\)/);
});

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

test("reuses credentials and caps non-current Douyin account work at two", () => {
  const start = appSource.indexOf("async function runBatchDetectionForRow");
  const end = appSource.indexOf("function firstEndpointValue", start);
  const batchFunction = appSource.slice(start, end);

  assert.match(batchFunction, /mapWithConcurrency\(douyinCredentials\.accounts, 2,/);
  assert.match(batchFunction, /const \[accountParamsOutcome, accountTokenOutcome, accountPasswordOutcome\] = await Promise\.all/);
  assert.doesNotMatch(batchFunction, /const creds = await invoke<DouyinAccountCredentialResult>/);
});

test("shows and exports structured step timings", () => {
  assert.match(appSource, /title=\{formatStepTimings\(row\.stepTimings\)\}/);
  assert.match(appSource, /\["分步耗时", formatStepTimings\(selectedBatchRow\.stepTimings\)\]/);
  assert.match(appSource, /"用时\(ms\)", "分步耗时", "来源ZIP"/);
  assert.match(appSource, /row\.durationMs \?\? "",\s*formatStepTimings\(row\.stepTimings\)/s);
});
