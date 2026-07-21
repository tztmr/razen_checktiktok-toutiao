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
